//! The client's handle on `sim`: the server's physics, built a tile at a time from the same files the
//! client streams, with snapshot vehicles as solids, stepping the local stickman exactly as the server
//! steps it. Everything crosses the boundary as numbers and byte slices; `client/src/sim/motor.ts`
//! wraps it in the client's own types.

use shared::protocol::InputFrame;
use shared::world::{Manifest, ObjId, Tile};
use sim::{Character, Obj, Physics};
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

fn error(context: &str, cause: impl std::fmt::Display) -> JsError {
    JsError::new(&format!("{context}: {cause}"))
}

#[wasm_bindgen]
pub struct ClientSim {
    physics: Physics,
    manifest: Manifest,
}

#[wasm_bindgen]
impl ClientSim {
    /// A world from its `world.msgpack`, with no tile in it yet.
    #[wasm_bindgen(constructor)]
    pub fn new(manifest: &[u8]) -> Result<ClientSim, JsError> {
        let manifest = Manifest::decode(manifest).map_err(|e| error("decoding the manifest", e))?;
        Ok(ClientSim { physics: Physics::streamed(&manifest), manifest })
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
    fn a_character_survives_the_trip_through_numbers() {
        let character = Character { position: [1.5, -2.25, 1e-3], yaw: -3.0, vy: 6.5, grounded: true, knock: [0.1, -7.0], ride: 4_000_001, prone: false, chute: true };
        let mut state = [0.0; CHARACTER_LEN];
        write(&character, &mut state);
        assert_eq!(read(&state), character);
    }
}
