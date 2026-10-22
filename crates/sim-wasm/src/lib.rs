//! The client's handle on `sim`: the server's physics, built a tile at a time from the same files the
//! client streams, with snapshot vehicles as solids, stepping the local stickman exactly as the server
//! steps it. Everything crosses the boundary as numbers and byte slices; `client/src/sim/motor.ts`
//! wraps it in the client's own types.

use shared::protocol::InputFrame;
use shared::world::{Manifest, ObjId, Tile};
use sim::{CarCondition, CarControls, CarMotion, Character, Obj, Physics};
use wasm_bindgen::prelude::*;

/// How a character travels in a `Float64Array`: every `f32` and every id fits exactly.
pub const CHARACTER_LEN: usize = 11;
const X: usize = 0;
const Y: usize = 1;
const Z: usize = 2;
const YAW: usize = 3;
const VY: usize = 4;
const GROUNDED: usize = 5;
const KNOCK_X: usize = 6;
const KNOCK_Z: usize = 7;
const RIDE: usize = 8;
const PRONE: usize = 9;
const CHUTE: usize = 10;

fn read(state: &[f64]) -> Character {
    Character {
        position: [state[X] as f32, state[Y] as f32, state[Z] as f32],
        yaw: state[YAW] as f32,
        vy: state[VY] as f32,
        grounded: state[GROUNDED] != 0.0,
        knock: [state[KNOCK_X] as f32, state[KNOCK_Z] as f32],
        ride: state[RIDE] as u32,
        prone: state[PRONE] != 0.0,
        chute: state[CHUTE] != 0.0,
    }
}

fn write(character: &Character, state: &mut [f64]) {
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    state[X] = character.position[0].into();
    state[Y] = character.position[1].into();
    state[Z] = character.position[2].into();
    state[YAW] = character.yaw.into();
    state[VY] = character.vy.into();
    state[GROUNDED] = flag(character.grounded);
    state[KNOCK_X] = character.knock[0].into();
    state[KNOCK_Z] = character.knock[1].into();
    state[RIDE] = character.ride.into();
    state[PRONE] = flag(character.prone);
    state[CHUTE] = flag(character.chute);
}

/// How car controls travel in a `Float32Array`: `[throttle, steer, handbrake, lift, powered, aimed,
/// aim yaw, aim pitch, boost, hop]`, flags as 0 or 1.
pub const CONTROLS_LEN: usize = 10;

fn controls_to(controls: &CarControls) -> Vec<f32> {
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    let [yaw, pitch] = controls.aim.unwrap_or_default();
    vec![controls.throttle, controls.steer, flag(controls.handbrake), controls.lift, flag(controls.powered), flag(controls.aim.is_some()), yaw, pitch, flag(controls.boost), flag(controls.hop)]
}

fn controls_from(c: &[f32]) -> CarControls {
    let at = |i: usize| c.get(i).copied().unwrap_or(0.0);
    CarControls {
        throttle: at(0),
        steer: at(1),
        handbrake: at(2) != 0.0,
        lift: at(3),
        powered: at(4) != 0.0,
        aim: (at(5) != 0.0).then(|| [at(6), at(7)]),
        boost: at(8) != 0.0,
        hop: at(9) != 0.0,
    }
}

/// How a car's state travels: `[x, y, z, qx, qy, qz, qw, vx, vy, vz, wx, wy, wz, steer, spin, tank,
/// firing, steer fraction]`; `tank` is NaN for a kind without one.
pub const CAR_STATE_LEN: usize = 18;

fn error(context: &str, cause: impl std::fmt::Display) -> JsError {
    JsError::new(&format!("{context}: {cause}"))
}

#[wasm_bindgen]
pub struct ClientSim {
    physics: Physics,
    manifest: Manifest,
    /// The kind of each vehicle simulated here (the player's own, predicted), by id.
    cars: std::collections::BTreeMap<u32, u8>,
}

#[wasm_bindgen]
impl ClientSim {
    /// A world from its `world.msgpack`, with no tile in it yet.
    #[wasm_bindgen(constructor)]
    pub fn new(manifest: &[u8]) -> Result<ClientSim, JsError> {
        let manifest = Manifest::decode(manifest).map_err(|e| error("decoding the manifest", e))?;
        Ok(ClientSim { physics: Physics::streamed(&manifest), manifest, cars: Default::default() })
    }

    /// Adds tile `(col, row)` from its file's bytes, replacing it if it is already there.
    #[wasm_bindgen(js_name = addTile)]
    pub fn add_tile(&mut self, col: u16, row: u16, bytes: &[u8]) -> Result<(), JsError> {
        let tile = Tile::decode(bytes).map_err(|e| error("decoding a tile", e))?;
        self.physics.add_tile(&self.manifest, col, row, &tile).map_err(|e| error("building a tile's colliders", e))
    }

    #[wasm_bindgen(js_name = removeTile)]
    pub fn remove_tile(&mut self, col: u16, row: u16) {
        self.physics.remove_tile(col, row);
    }

    /// Fells tree `index` of tile `(col, row)`; takes effect at the next `refresh`.
    #[wasm_bindgen(js_name = fellTree)]
    pub fn fell_tree(&mut self, col: u16, row: u16, index: u32) -> bool {
        self.physics.destroy(Obj::Tree(ObjId { col, row, index }))
    }

    /// Brings building `index` of tile `(col, row)` down; takes effect at the next `refresh`.
    #[wasm_bindgen(js_name = fellBuilding)]
    pub fn fell_building(&mut self, col: u16, row: u16, index: u32) -> bool {
        self.physics.destroy(Obj::Building(ObjId { col, row, index }))
    }

    /// Gives building `index` of tile `(col, row)` its shape without the cells `removed`; takes effect
    /// at the next `refresh`.
    #[wasm_bindgen(js_name = carveBuilding)]
    pub fn carve_building(&mut self, col: u16, row: u16, index: u32, removed: &[u32]) {
        self.physics.carve_building(col, row, index, removed);
    }

    /// Carries out what was queued and brings the scene queries up to date.
    pub fn refresh(&mut self) {
        self.physics.refresh();
    }

    /// Vehicle `id` of `kind` where a snapshot shows it, moving at `(vx, vy, vz)` m/s.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = setObstacle)]
    pub fn set_obstacle(&mut self, id: u32, kind: u8, x: f32, y: f32, z: f32, qx: f32, qy: f32, qz: f32, qw: f32, vx: f32, vy: f32, vz: f32, turret: bool) {
        self.physics.set_obstacle(id, kind, [x, y, z], [qx, qy, qz, qw], [vx, vy, vz], turret);
    }

    #[wasm_bindgen(js_name = removeObstacle)]
    pub fn remove_obstacle(&mut self, id: u32) {
        self.physics.remove_obstacle(id);
    }

    #[wasm_bindgen(js_name = obstacleIds)]
    pub fn obstacle_ids(&self) -> Vec<u32> {
        self.physics.obstacle_ids()
    }

    /// One tick of the stickman in `state` (`CHARACTER_LEN` numbers, rewritten in place) under the
    /// input frame's fields, as the server steps it (`Physics::step_character`). A stickman standing on
    /// vehicle `ride` takes that vehicle's velocity as last placed. Its `ride` stays while it stays on
    /// the ground: which vehicle is under it is the next snapshot's to say.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = stepCharacter)]
    pub fn step_character(&self, state: &mut [f64], seq: u32, strafe: i8, forward: i8, yaw: i16, buttons: u16, pitch: i16, view: u16, slot: u8, dt: f32, speed_scale: f32) {
        let mut character = read(state);
        let input = InputFrame { seq, strafe, forward, yaw, buttons, pitch, view, slot };
        let velocity = if character.ride == 0 { None } else { self.physics.obstacle_velocity(character.ride) };
        self.physics.step_character(&mut character, &input, dt, speed_scale, velocity);
        if !character.grounded {
            character.ride = 0;
        }
        write(&character, state);
    }

    /// Starts simulating vehicle `id` of `kind` here, as a body of its own in place of its stand-in:
    /// the player drives it, and predicts it.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = startCar)]
    pub fn start_car(&mut self, id: u32, kind: u8, x: f32, y: f32, z: f32, yaw: f32) {
        self.stop_car(id);
        self.physics.remove_obstacle(id);
        self.physics.add_car(id, kind, [x, y, z], yaw);
        self.cars.insert(id, kind);
    }

    /// Stops simulating vehicle `id`.
    #[wasm_bindgen(js_name = stopCar)]
    pub fn stop_car(&mut self, id: u32) {
        if self.cars.remove(&id).is_some() {
            self.physics.remove_car(id);
        }
    }

    /// Puts vehicle `id` in the state a snapshot reports: pose, motion (`DriveState`), its tank
    /// (NaN to leave it), and the wear its health and damage mean, turret included.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = setCarState)]
    pub fn set_car_state(&mut self, id: u32, x: f32, y: f32, z: f32, qx: f32, qy: f32, qz: f32, qw: f32, vx: f32, vy: f32, vz: f32, wx: f32, wy: f32, wz: f32, steer: f32, spin: f32, tank: f32, hp: u8, damage: &[u8], turret: bool) {
        let Some(&kind) = self.cars.get(&id) else { return };
        let motion = CarMotion { linvel: [vx, vy, vz], angvel: [wx, wy, wz], steer, spin };
        self.physics.set_car_state(id, [x, y, z], [qx, qy, qz, qw], motion, (!tank.is_nan()).then_some(tank));
        let mut wear = [0; 8];
        for (slot, &value) in wear.iter_mut().zip(damage) {
            *slot = value;
        }
        self.physics.set_car_condition(id, CarCondition::of(kind, hp, wear));
        self.physics.set_turret(id, turret);
    }

    /// The controls (`CONTROLS_LEN` numbers) a driver's input asks of a vehicle of `kind`, as the
    /// server reads them (`CarControls::driven`).
    #[wasm_bindgen(js_name = controlsFor)]
    pub fn controls_for(kind: u8, strafe: i8, forward: i8, yaw: i16, pitch: i16, buttons: u16) -> Vec<f32> {
        let input = InputFrame { strafe, forward, yaw, pitch, buttons, ..Default::default() };
        controls_to(&CarControls::driven(kind, &input))
    }

    /// One tick of vehicle `id` under `controls` (`CONTROLS_LEN` numbers): the server's `drive_car`
    /// then `step_cars`.
    #[wasm_bindgen(js_name = driveCar)]
    pub fn drive_car(&mut self, id: u32, controls: &[f32], dt: f32) {
        if !self.cars.contains_key(&id) {
            return;
        }
        self.physics.drive_car(id, controls_from(controls), dt);
        self.physics.step_cars(dt);
    }

    /// Vehicle `id`'s state (`CAR_STATE_LEN` numbers), or nothing when it is not simulated here.
    #[wasm_bindgen(js_name = carState)]
    pub fn car_state(&self, id: u32) -> Vec<f32> {
        let (Some(pose), Some(motion)) = (self.physics.car_pose(id), self.physics.car_motion(id)) else { return Vec::new() };
        let (tank, firing) = self.physics.car_thrust(id).map_or((f32::NAN, 0), |(tank, firing)| (tank, firing));
        let [x, y, z] = pose.position;
        let [qx, qy, qz, qw] = pose.rotation;
        let [vx, vy, vz] = motion.linvel;
        let [wx, wy, wz] = motion.angvel;
        vec![x, y, z, qx, qy, qz, qw, vx, vy, vz, wx, wy, wz, motion.steer, motion.spin, tank, f32::from(firing), pose.steer]
    }

    /// Whether the standing body fits with its centre at `(x, y, z)`.
    pub fn fits(&self, x: f32, y: f32, z: f32) -> bool {
        self.physics.character_fits([x, y, z])
    }

    /// How far a ray goes before it meets the world or a vehicle within `max`, and the surface normal
    /// there, as `[t, nx, ny, nz]`; empty when it meets nothing.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = castRay)]
    pub fn cast_ray(&self, ox: f32, oy: f32, oz: f32, dx: f32, dy: f32, dz: f32, max: f32) -> Vec<f32> {
        self.physics.cast_ray_normal([ox, oy, oz], [dx, dy, dz], max).map_or_else(Vec::new, |(t, [nx, ny, nz])| vec![t, nx, ny, nz])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_survive_the_trip_through_numbers() {
        let controls = CarControls { throttle: -0.5, steer: 1.0, handbrake: true, lift: -1.0, powered: true, aim: Some([0.25, -1.5]), boost: false, hop: true };
        let numbers = controls_to(&controls);
        assert_eq!(numbers.len(), CONTROLS_LEN);
        assert_eq!(controls_from(&numbers), controls);
        assert_eq!(controls_from(&controls_to(&CarControls::IDLE)), CarControls::IDLE);
    }

    #[test]
    fn a_character_survives_the_trip_through_numbers() {
        let character = Character { position: [1.5, -2.25, 1e-3], yaw: -3.0, vy: 6.5, grounded: true, knock: [0.1, -7.0], ride: 4_000_001, prone: false, chute: true };
        let mut state = [0.0; CHARACTER_LEN];
        write(&character, &mut state);
        assert_eq!(read(&state), character);
    }
}
