//! Ground vehicles: dynamic rigid bodies with ray-cast wheels (Rapier's `DynamicRayCastVehicleController`).
//!
//! The chassis is a box; each wheel is a suspension ray that pushes the chassis up and applies
//! engine, brake and side-grip impulses at its contact point. Body-local axes are +X right,
//! +Y up and -Z forward, so a yaw of 0 faces -Z exactly like a soldier.
//!
//! A car steers its front wheels. A tank has six unsteered wheels and turns on the spot: the
//! physics sets its yaw rate directly, which is how skid steering looks from the outside.
//! A helicopter has no wheels at all: its rotor is a push along the hull's up axis, and the hull is
//! levelled and turned by setting its angular velocity, so it is flown by leaning (see [`fly`]).
//! A plane rolls on wheels like a car, with no engine on them: its propeller pushes the body along the
//! nose, and its wings hold it up once it is fast enough (see [`wing`]).
//! A walker stands on two spring legs and is kept upright by a torque (see [`walk`]); a hen is a walker
//! with wings, which leap, flap and glide (see [`flap`]).
//! A ship floats on open water and sails by the wind, steered by its rudder (see [`sail`]); its guns
//! are vehicles of their own, fixed to its deck and carried there each step (see [`Physics::mount_car`]).
//! "Car" in the names below means any of these vehicles; [`Spec`] says which one.

use shared::vec3;
use super::{Obj, Physics, RAY_FLOOR, TERRAIN_TOP};
use rapier3d::control::{DynamicRayCastVehicleController, WheelTuning};
use rapier3d::prelude::*;
use shared::consts::*;
use shared::armor::{Part, State, unpack};
use shared::protocol::{DAMAGE_PARTS, DAMAGE_SLOTS, DAMAGE_WHEELS, THRUST_AIR, THRUST_BOOST, THRUST_GLIDE, THRUST_LIFT, WHEEL_GONE, is_drone, vehicle_hp};


mod dive;
mod flight;
mod floats;
mod sail;
mod spec;
mod water;

use flight::{LOITER_GLIDE, MISSILE_GLIDE, assist, boost, flap, fly, fly_drone, glide, pull, on_runners, roll_plane, thrust, walk, wing};
use dive::dive;
use floats::float;
use sail::sail;
use water::{wade, water_under};
pub use spec::half_extents;
pub(super) use spec::{Airframe, Buttons, Floats, Lift, Role, Runners, Shape, Solid, Spec, spec};


/// How quickly a tank's yaw rate follows the stick, rad/s².
const TURN_ACCEL: f32 = 3.0;
/// Speed change a blast gives a vehicle at its centre, m/s.
const BLAST_VELOCITY: f32 = 5.0;
/// Impulse each wheel may use for braking per tick, per kilogram of chassis.
const BRAKE_PER_KG: f32 = 0.045;
/// Light brake that stands in for rolling resistance when the driver is off the pedals.
const COAST_BRAKE_PER_KG: f32 = 0.0015;
/// A broken track drags on its side's wheels as this share of a full brake.
const BROKEN_TRACK_DRAG: f32 = 0.5;
/// How fast the front wheels turn towards the steering input, radians per second.
const STEER_RATE: f32 = 7.0;
/// Wheels straighten this much faster than they turn in.
const STEER_RETURN: f32 = 1.5;
/// The engine pulls with full force up to this fraction of the top speed, then fades to nothing at it.
const FULL_POWER: f32 = 0.65;
/// Side grip of a car's rear wheels while the handbrake is held, against 1 normally. Grip is the share
/// of the wheel's sideways slip cancelled each tick, so even 0.3 holds firm; this lets the tail slide out.
const DRIFT_GRIP: f32 = 0.05;
/// A kind that slides (`Spec::slide`) lets its tail go past this share of full steering, and from
/// the first of these speeds (m/s) to all of it at the second: at a crawl it still turns on the spot.
/// From the third it bites again, fully by the fourth, so that a hard turn at full tilt loads it up
/// and, if it rolls (`Spec::roll`), throws it over.
const SLIDE_STEER: f32 = 0.5;
const SLIDE_SPEEDS: [f32; 4] = [5.0, 15.0, 22.0, 28.0];
/// Past the first slip angle (radians) a sliding kind's tail stays loose, wholly so at the second.
const SLIDE_SLIP: [f32; 2] = [0.05, 0.3];
/// Once the driver straightens up, a sliding kind's spin dies away at this rate, per second: the
/// slide is caught instead of carrying on round into a spin.
const SLIDE_CATCH: f32 = 4.0;
/// The share of the true roll leverage Rapier's wheels keep (`WheelTuning`'s private `roll_influence`).
const RAPIER_ROLL_INFLUENCE: f32 = 0.1;
/// Downforce on a car with a wheel down, per kilogram, per (m/s)²: a third of its weight again at 40 m/s.
const DOWNFORCE_PER_KG: f32 = 0.0001;
/// How quickly the throttle pitches and the steering rolls a car with no wheel down, rad/s².
const AIR_PITCH: f32 = 3.0;
const AIR_ROLL: f32 = 3.0;
/// Side grip of the rear wheels and the tyres' friction, as `WheelTuning` sets them.
const SIDE_GRIP: f32 = 1.0;
const FRICTION_SLIP: f32 = 12.0;
/// How high a track's end wheels ride over flat ground, metres: ground rising more than this under the
/// nose or the tail lifts it.
const TRACK_END_CLEAR: f32 = 0.25;
/// How far back from the nose (and forward from the tail) a tracked hull's underside sweeps up, as a
/// share of its half height. Under 1, so the sweep rises more steeply than 45°.
const TRACKED_SWEEP_RUN: f32 = 0.9;
/// Friction of a tracked hull's belly, against 0.4 for anything else's.
const TRACKED_HULL_FRICTION: f32 = 0.1;
/// Gravity, m/s², for the sag of a vehicle's springs under its own weight.
const GRAVITY_SAG: f32 = 9.81;
/// How far a wrecked wheel points off straight, radians: a left wheel turns left, a right one right.
const TOE_MAX: f32 = 0.12;
/// Share of its side grip and friction a wrecked wheel loses.
const WHEEL_GRIP_LOSS: f32 = 0.85;
const WHEEL_SLIP_LOSS: f32 = 0.6;
/// A wheel that has come off: a stub that never reaches the ground.
const LOST_WHEEL_RADIUS: f32 = 0.05;
/// How hard a hitch resists the towed gun swinging about it, as a motor holding their yaw rates
/// together. Without it a gun heavier than what tows it (a Pak behind the snowmobile) sways ever
/// wider at speed until it jackknifes.
const HITCH_YAW_DAMPING: f32 = 200.0;
/// How wide a bow's stem is, as a share of the hull's width.
const STEM: f32 = 0.08;

/// What the driver asks of the car this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CarControls {
    /// -1 (reverse) ..= 1 (forward). A plane's throttle: full ahead, cruising at 0, the blip switch behind.
    pub throttle: f32,
    /// -1 (left) ..= 1 (right). A helicopter turns instead, and a plane rolls.
    pub steer: f32,
    pub handbrake: bool,
    /// -1 (sink) ..= 1 (climb). Only a helicopter has it.
    pub lift: f32,
    /// The rotor or the propeller is turning, i.e. a pilot is aboard. Only a helicopter and a plane have it.
    pub powered: bool,
    /// Where a plane's pilot points the mouse, `[yaw, pitch]` in radians (yaw 0 faces -Z, + turns left):
    /// the plane turns towards it, banking into the turn. `None` flies on in a straight line.
    pub aim: Option<[f32; 2]>,
    /// A Roadster's rear nozzles are held open. Only a Roadster has it.
    pub boost: bool,
    /// A Roadster hops off the ground, then holds itself up in the air. Only a Roadster has it.
    pub hop: bool,
}

/// How worn a vehicle is, which both ends' physics apply: the engine's power, the share of its top
/// speed and turn rate it has left, how much each side (left, right) is driven, and the state of a
/// car's four wheels (front left, front right, rear left, rear right; 0 sound, 255 come off).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarCondition {
    pub power: f32,
    pub speed: f32,
    pub turn: f32,
    pub sides: [f32; 2],
    pub wheels: [u8; 4],
    /// It has no health left: a ship's hull fills, and it sinks.
    pub wrecked: bool,
}

/// A hurt engine's share of its power and of the top speed.
const ENGINE_HURT: f32 = 0.6;
/// A hurt gearbox's share of the top speed.
const GEARBOX_HURT: f32 = 0.7;
/// A hurt track's share of the top speed; with one broken, the share of speed and of the turn left.
const TRACK_HURT: f32 = 0.8;
const TRACK_BROKEN_SPEED: f32 = 0.3;
const TRACK_BROKEN_TURN: f32 = 0.35;

impl CarCondition {
    pub const NEW: CarCondition = CarCondition { power: 1.0, speed: 1.0, turn: 1.0, sides: [1.0; 2], wheels: [0; 4], wrecked: false };

    /// From a vehicle's health and damage as the snapshot carries them. The engine keeps full power
    /// down to half health and loses up to half of it below. A vehicle's parts (its damage slots
    /// `DAMAGE_PARTS`) are hurt or gone on top of that: see `shared::armor::Part`.
    pub fn of(kind: u8, hp: u8, damage: [u8; DAMAGE_SLOTS]) -> CarCondition {
        let health = f32::from(hp) / f32::from(vehicle_hp(kind));
        let def = shared::vehicles::vehicle_def(kind);
        let mut condition = CarCondition { power: (0.5 + health).min(1.0), wrecked: hp == 0, ..CarCondition::NEW };
        // Only a car has wheels to hurt: a tank's first wheel slot says whether its turret is on.
        if def.car_wheels {
            condition.wheels.copy_from_slice(&damage[DAMAGE_WHEELS..DAMAGE_WHEELS + 4]);
        }
        if def.armor.modules.is_empty() {
            return condition;
        }
        let states = unpack([damage[DAMAGE_PARTS], damage[DAMAGE_PARTS + 1]]);
        let state = |part: Part| part.slot().map_or(State::Sound, |slot| states[slot]);
        match state(Part::Engine) {
            State::Hurt => {
                condition.power *= ENGINE_HURT;
                condition.speed *= ENGINE_HURT;
            }
            State::Gone => condition.power = 0.0,
            State::Sound => {}
        }
        match state(Part::Transmission) {
            State::Hurt => condition.speed *= GEARBOX_HURT,
            State::Gone => condition.power = 0.0,
            State::Sound => {}
        }
        for (side, part) in [Part::TrackLeft, Part::TrackRight].into_iter().enumerate() {
            match state(part) {
                State::Hurt => condition.speed *= TRACK_HURT,
                State::Gone => {
                    condition.sides[side] = 0.0;
                    condition.speed *= TRACK_BROKEN_SPEED;
                    condition.turn *= TRACK_BROKEN_TURN;
                }
                State::Sound => {}
            }
        }
        // Nothing turns the tracks, or neither of them goes round: it does not move under its own power.
        if condition.power == 0.0 || condition.sides == [0.0; 2] {
            condition.power = 0.0;
            condition.turn = 0.0;
        }
        condition
    }
}

impl CarControls {
    /// Nobody asks for anything.
    pub const IDLE: CarControls = CarControls { throttle: 0.0, steer: 0.0, handbrake: false, lift: 0.0, powered: false, aim: None, boost: false, hop: false };

    /// What the driver's `input` asks of a vehicle of `kind`. The server drives with it, and the
    /// client predicts its own vehicle with the same.
    pub fn driven(kind: u8, input: &shared::protocol::InputFrame) -> CarControls {
        let held = |button: u16| input.buttons() & button != 0;
        let spec = spec(kind);
        let stick = CarControls {
            steer: shared::quant::from_axis(input.strafe),
            throttle: shared::quant::from_axis(input.forward),
            powered: true,
            // A plane and a drone fly towards where their pilot looks.
            aim: spec.aims.then(|| [shared::quant::from_mrad(input.yaw), shared::quant::from_mrad(input.pitch())]),
            ..CarControls::IDLE
        };
        match spec.buttons {
            Buttons::Drive => CarControls { handbrake: held(BUTTON_JUMP), lift: f32::from(held(BUTTON_JUMP)) - f32::from(held(BUTTON_RUN)), ..stick },
            Buttons::Stunt => CarControls { handbrake: held(BUTTON_CROUCH), boost: held(BUTTON_RUN), hop: held(BUTTON_JUMP), ..stick },
            Buttons::Leap => CarControls { hop: held(BUTTON_JUMP), ..stick },
            Buttons::Sled => CarControls { handbrake: held(BUTTON_JUMP), boost: held(BUTTON_RUN), ..stick },
        }
    }
}

/// A car's pose as the game loop and the wire need it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarPose {
    pub position: [f32; 3],
    /// Unit quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    /// Speed along the forward axis, m/s; negative when reversing.
    pub forward_speed: f32,
    /// Front wheel angle as a fraction of the kind's `steer_lock`, + is right.
    pub steer: f32,
    /// Y component of the chassis' up axis: 1 upright, 0 on its side, -1 upside down.
    pub up_y: f32,
    /// Linear velocity, m/s.
    pub velocity: [f32; 3],
}

/// Where a shot met a vehicle, for armor: see [`Physics::armor_strike`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strike {
    pub distance: f32,
    /// Which of the kind's `spec.solids` it hit (a turret, a casemate, a shield); `None` for the hull.
    pub solid: Option<u8>,
    /// Outward surface normal, in the vehicle's frame (+X right, +Y up, -Z forward).
    pub normal: [f32; 3],
    /// The point, in the vehicle's frame.
    pub point: [f32; 3],
}

/// What a driver needs to resume predicting a car, beyond its pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarMotion {
    /// Linear velocity, m/s.
    pub linvel: [f32; 3],
    /// Angular velocity about the world axes, rad/s.
    pub angvel: [f32; 3],
    /// Front wheel angle, radians, + is right.
    pub steer: f32,
    /// Yaw rate a tank has ramped to, rad/s, + is clockwise.
    pub spin: f32,
}

/// Something a vehicle's solids pushed against during the last step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarImpact {
    pub car: u32,
    /// The vehicle on the other side, if it was one.
    pub other_car: Option<u32>,
    /// The building or tree on the other side, if it was one.
    pub object: Option<Obj>,
    /// Whether the other side was a chunk of placed cubes (`set_construct`).
    pub construct: bool,
    /// Where they touched, in the vehicle's own frame (+X right, +Y up, -Z forward).
    pub local: [f32; 3],
    /// Where they touched, in the world.
    pub point: [f32; 3],
    /// Speed the contact took out of the vehicle this step, m/s: its impulse over the vehicle's mass.
    pub dv: f32,
    /// The part of `dv` along the vehicle's nose: how much the contact stopped it, not lifted or turned it.
    pub stopped: f32,
    /// Whether the other side was the ground.
    pub ground: bool,
}

/// A car touching a character.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarHit {
    pub car: u32,
    pub position: [f32; 3],
    /// Linear velocity of the chassis, m/s.
    pub velocity: [f32; 3],
}

pub(super) struct Car {
    spec: &'static Spec,
    body: RigidBodyHandle,
    collider: ColliderHandle,
    /// A tank's turret, a plane's wings or a walker's legs: more solids on the same body. They have no mass of their own.
    extra: Vec<ColliderHandle>,
    controller: DynamicRayCastVehicleController,
    /// Current front wheel angle, radians, + is right.
    steer: f32,
    /// Yaw rate asked for by a vehicle that turns on the spot, rad/s, + is clockwise (right). Zero leaves it alone.
    turn: f32,
    /// The yaw rate it is actually turning at, rad/s, + is clockwise. It ramps towards `turn`.
    spin: f32,
    /// What a helicopter or a plane was last asked to do, applied in the next [`Physics::step_cars`].
    flight: CarControls,
    /// Throttle and steering of a car, which tilt it while it is in the air.
    air: (f32, f32),
    condition: CarCondition,
    /// A mortar's tow: the car it is hitched to, and the joint that holds it there.
    towed_by: Option<(u32, ImpulseJointHandle)>,
    /// A Roadster's air tank, 0..=`ROADSTER_AIR`, or a hen's or a tupp's stamina, 0..=`HEN_STAMINA`; `None` for anything else.
    tank: Option<f32>,
    /// The `THRUST_*` nozzles that fired in the last step.
    firing: u8,
    /// Where a ship's gun is fixed (`mount_car`): it is carried there after every step.
    mounted: Option<Mounted>,
}

/// A vehicle fixed to another one, `on`, at `at` (its own frame), turned `yaw` there.
#[derive(Clone, Copy, Debug)]
struct Mounted {
    on: u32,
    at: Pose,
}

impl Car {
    /// Whether `collider` is one of this vehicle's solids.
    fn owns(&self, collider: ColliderHandle) -> bool {
        self.collider == collider || self.extra.contains(&collider)
    }
}

fn pose_of(position: [f32; 3], yaw: f32) -> Pose {
    Pose::from_parts(Vector::from(position), Rotation::from_rotation_y(yaw))
}

/// How far below its feet a standing character probes for a car to ride. The character controller
/// rests 2 cm above whatever it stands on, so this reaches into the roof.
const RIDE_PROBE: f32 = 0.06;

impl CarPose {
    /// Where `point`, fixed to the car at `self`, ends up once the car has moved to `to`.
    pub fn carry(&self, to: &CarPose, point: [f32; 3]) -> [f32; 3] {
        let [x, y, z, w] = self.rotation;
        let from = Rotation::from_xyzw(x, y, z, w);
        let [x, y, z, w] = to.rotation;
        let onto = Rotation::from_xyzw(x, y, z, w);
        let local = from.inverse() * (Vector::from(point) - Vector::from(self.position));
        (onto * local + Vector::from(to.position)).into()
    }

    /// Where `local`, a point in the vehicle's own frame, is in the world.
    pub fn world_point(&self, local: [f32; 3]) -> [f32; 3] {
        let [x, y, z, w] = self.rotation;
        (Rotation::from_xyzw(x, y, z, w) * Vector::from(local) + Vector::from(self.position)).into()
    }

    /// The world direction of `local`, a direction in the vehicle's own frame.
    pub fn world_dir(&self, local: [f32; 3]) -> [f32; 3] {
        let [x, y, z, w] = self.rotation;
        (Rotation::from_xyzw(x, y, z, w) * Vector::from(local)).into()
    }

    /// The direction in the vehicle's own frame of `world`, a world direction.
    pub fn local_dir(&self, world: [f32; 3]) -> [f32; 3] {
        let [x, y, z, w] = self.rotation;
        (Rotation::from_xyzw(x, y, z, w).inverse() * Vector::from(world)).into()
    }

    /// Heading of the forward axis in the soldier convention: 0 faces -Z, + turns left (towards -X).
    pub fn yaw(&self) -> f32 {
        let [x, y, z, w] = self.rotation;
        (2.0 * (x * z + w * y)).atan2(1.0 - 2.0 * (x * x + y * y))
    }
}

/// How many wheels a vehicle stands on: two to an axle. A tracked one has a track's end wheels
/// besides, which the engine's pull is not shared out over.
/// Whether a vehicle turns on the spot on tracks rather than steering its wheels.
fn tracked(spec: &Spec) -> bool {
    spec.turn_rate > 0.0 && spec.steer_lock == 0.0 && spec.lift.wheeled()
}

fn road_wheels(spec: &Spec) -> usize {
    2 * spec.axles.len()
}

/// Where a wheeled vehicle's hull centre settles over flat ground, metres: its wheels' reach below the
/// hard points, less the sag under its weight on firm springs. Close enough to hang a track's end wheels from.
fn ride_height(spec: &Spec) -> f32 {
    let sag = GRAVITY_SAG / (road_wheels(spec) as f32 * spec.springs.stiffness);
    -spec.hard_point_y + spec.suspension_rest + spec.wheel_radius - sag
}

/// Whether a vehicle of `kind` is too heavy to be stopped by a tree (a tank, a Strv): it goes through
/// the trunk, felling it.
pub(crate) fn heavy(kind: u8) -> bool {
    shared::vehicles::vehicle_def(kind).body.heavy
}

/// The solids a vehicle of `kind` has besides its hull box, placed in the hull frame: a tank's turret,
/// a Strv's casemate, a plane's wings, a walker's legs. Heavy vehicles' leave the felled-able trees
/// out of what they touch, as the hull does.
pub(crate) fn extra_solids(kind: u8) -> Vec<ColliderBuilder> {
    let solids = spec(kind).solids.iter().map(|solid| ColliderBuilder::new(solid.shape(0.0)).translation(Vector::from(solid.at)));
    if heavy(kind) { solids.map(|collider| collider.collision_groups(super::plows_trees())).collect() } else { solids.collect() }
}

/// Whether a snowmobile under `controls` has its boost open: held, driving forward, with some left in `tank`.
fn boosting(controls: CarControls, tank: Option<f32>) -> bool {
    controls.boost && controls.throttle > 0.0 && tank.is_some_and(|tank| tank > 0.0)
}

/// How far a car's front wheels can turn at `speed` m/s, radians: the lock narrows at speed.
fn lock(spec: &Spec, speed: f32) -> f32 {
    spec.steer_lock / (1.0 + speed.abs() / spec.steer_fade)
}

/// How far a car's front wheels are turned (`steer`, radians) at `speed`, as a share of its lock there.
fn turned(spec: &Spec, steer: f32, speed: f32) -> f32 {
    let lock = lock(spec, speed);
    if lock > 0.0 { steer.abs() / lock } else { 0.0 }
}

/// A vehicle's hull: its box, with the front of the underside swept up if it has a `nose`, or its
/// sides tapered in to a stem if it has a `bow`. A tracked one's underside sweeps up at both ends to
/// half its height at the nose and the tail, as its tracks run up round the sprocket and the idler: it
/// rides up onto a bump or a step rather than ramming it. The sweeps rise more than they run, so a
/// shot into one still meets front or rear armour, not the belly (`Zone::of`).
pub(crate) fn hull(kind: u8) -> ColliderBuilder {
    let spec = spec(kind);
    let [hx, hy, hz] = spec.half_extents;
    if heavy(kind) {
        let run = TRACKED_SWEEP_RUN * hy;
        let mut corners = Vec::new();
        for x in [-hx, hx] {
            corners.extend([[x, hy, -hz], [x, hy, hz], [x, 0.0, -hz], [x, 0.0, hz], [x, -hy, run - hz], [x, -hy, hz - run]].map(Vector::from));
        }
        return ColliderBuilder::convex_hull(&corners).expect("a tracked hull is not flat");
    }
    if spec.bow > 0.0 {
        let stem = hx * STEM;
        let corners: Vec<Vector> = [-hy, hy]
            .into_iter()
            .flat_map(|y| [[-hx, y, hz], [hx, y, hz], [-hx, y, spec.bow - hz], [hx, y, spec.bow - hz], [-stem, y, -hz], [stem, y, -hz]])
            .map(Vector::from)
            .collect();
        return ColliderBuilder::convex_hull(&corners).expect("a hull with a bow is not flat");
    }
    if spec.nose <= 0.0 {
        return ColliderBuilder::cuboid(hx, hy, hz);
    }
    let mut corners = Vec::new();
    for x in [-hx, hx] {
        corners.extend([[x, hy, -hz], [x, hy, hz], [x, -hy, hz], [x, -hy, spec.nose - hz], [x, 0.0, -hz]].map(Vector::from));
    }
    ColliderBuilder::convex_hull(&corners).expect("a hull with a nose is not flat")
}

/// A solid's collision shape.
pub(crate) trait Collide {
    /// Its shape, grown by `margin` on every side.
    fn shape(&self, margin: f32) -> SharedShape;
}

impl Collide for Solid {
    fn shape(&self, margin: f32) -> SharedShape {
        match self.shape {
            Shape::Cylinder { half_height, radius } => SharedShape::cylinder(half_height + margin, radius + margin),
            Shape::Cuboid([x, y, z]) => SharedShape::cuboid(x + margin, y + margin, z + margin),
        }
    }
}

/// The handles of `extra` (a car's or a stand-in's extra solids, made from `spec.solids` in order)
/// whose solid has `role`.
pub(crate) fn with_role<'a>(spec: &'a Spec, extra: &'a [ColliderHandle], role: Role) -> impl Iterator<Item = ColliderHandle> + 'a {
    extra.iter().zip(spec.solids).filter(move |(_, solid)| solid.role == role).map(|(&handle, _)| handle)
}

impl Physics {
    /// Whether a vehicle of `kind` at `position` / `yaw` is clear of buildings and walls.
    pub fn car_fits(&self, kind: u8, position: [f32; 3], yaw: f32) -> bool {
        let [hx, hy, hz] = spec(kind).half_extents;
        // Slightly inflated, so a car never starts touching a wall.
        let shape = SharedShape::cuboid(hx + 0.1, hy + 0.1, hz + 0.1);
        if self.touches_static(shape.as_ref(), pose_of(position, yaw)) {
            return false;
        }
        spec(kind).solids.iter().filter(|solid| solid.role == Role::Wings).all(|wings| {
            let at = pose_of(position, yaw) * Vector::from(wings.at);
            !self.touches_static(wings.shape(0.1).as_ref(), Pose::from_parts(at, Rotation::from_rotation_y(yaw)))
        })
    }

    /// How far a plane parked at `position` facing `yaw` could run straight ahead before a building,
    /// a tree or a wall gets in the way of its fuselage or wingtips, or the ground rises or falls away
    /// more than a little, up to `max` metres. Only the static world counts: cars move.
    pub fn runway(&self, ground: &shared::terrain::Ground, position: [f32; 3], yaw: f32, max: f32) -> f32 {
        let (sin, cos) = yaw.sin_cos();
        let ahead = [-sin, 0.0, -cos];
        let right = [cos, 0.0, -sin];
        let start = ground.height_at(position[0], position[2]);
        let mut clear = max;
        // The ground along the way, every few metres.
        let mut travelled = 0.0;
        while travelled < max {
            travelled += 4.0;
            let height = ground.height_at(position[0] + ahead[0] * travelled, position[2] + ahead[2] * travelled);
            if (height - start).abs() > 1.5 {
                clear = clear.min(travelled - 4.0);
                break;
            }
        }
        // Rays along the middle and out at each wingtip, low and at the top wing.
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, QueryFilter::exclude_dynamic());
        for offset in [0.0, -PLANE_WING_HALF_EXTENTS[0], PLANE_WING_HALF_EXTENTS[0]] {
            for height in [0.6, 2.4] {
                let origin = Vector::new(position[0] + right[0] * offset, start + height, position[2] + right[2] * offset);
                let ray = Ray::new(origin, Vector::from(ahead));
                if let Some((_, distance)) = queries.cast_ray(&ray, max, true) {
                    clear = clear.min(distance);
                }
            }
        }
        clear.max(0.0)
    }

    /// Adds a vehicle of `kind` under `id`; read it back with [`Physics::car_pose`].
    pub fn add_car(&mut self, id: u32, kind: u8, position: [f32; 3], yaw: f32) {
        let spec = spec(kind);
        let [hx, hy, hz] = spec.half_extents;
        let body = RigidBodyBuilder::dynamic()
            .pose(pose_of(position, yaw))
            .linear_damping(spec.linear_damping)
            .angular_damping(0.8)
            // A drone covers more than its own size in a tick, and would fly through a wall without it.
            .ccd_enabled(is_drone(kind))
            .build();
        let body = self.bodies.insert(body);
        // A tracked hull's belly slides over what it rests on rather than dragging on it.
        let mut chassis = hull(kind).friction(if heavy(kind) { TRACKED_HULL_FRICTION } else { 0.4 });
        // A tracked hull keeps the mass and balance of its whole box, swept-up ends and all.
        chassis = if spec.com_drop > 0.0 || heavy(kind) {
            // The box's own mass and inertia, with the centre of mass lowered.
            let mass = spec.density * 8.0 * hx * hy * hz;
            let inertia = Vector::new(hy * hy + hz * hz, hx * hx + hz * hz, hx * hx + hy * hy) * (mass / 3.0);
            chassis.mass_properties(MassProperties::new(Vector::new(0.0, -spec.com_drop, 0.0), mass, inertia))
        } else {
            chassis.density(spec.density)
        };
        if heavy(kind) {
            chassis = chassis.collision_groups(super::plows_trees());
        }
        let collider = self.colliders.insert_with_parent(chassis.build(), body, &mut self.bodies);
        let extra: Vec<ColliderHandle> =
            extra_solids(kind).into_iter().map(|collider| self.colliders.insert_with_parent(collider.density(0.0).friction(0.4).build(), body, &mut self.bodies)).collect();
        // A helicopter has no wheels, so its controller is never updated.
        let mut controller = DynamicRayCastVehicleController::new(body);
        // Our forward is -Z, but the controller only uses this axis to sign its own speed estimate,
        // which we do not read.
        controller.index_forward_axis = 2;
        let tuning = WheelTuning {
            suspension_stiffness: spec.springs.stiffness,
            suspension_compression: spec.springs.compression,
            suspension_damping: spec.springs.damping,
            max_suspension_travel: spec.springs.travel,
            side_friction_stiffness: SIDE_GRIP,
            friction_slip: FRICTION_SLIP,
            max_suspension_force: spec.max_suspension_force,
        };
        for &z in spec.axles {
            for side in [-1.0, 1.0] {
                controller.add_wheel(
                    Vector::new(side * spec.track_half, spec.hard_point_y, z),
                    Vector::new(0.0, -1.0, 0.0),
                    // The axle points right so that `up × axle` (the rolling direction) is -Z.
                    Vector::new(1.0, 0.0, 0.0),
                    spec.suspension_rest,
                    spec.wheel_radius,
                    &tuning,
                );
            }
        }
        // A track's run up round its sprocket and idler: a wheel at each end of each track, hung from
        // the hull's top and riding clear of flat ground, that only bites where the ground rises under
        // the nose or the tail. It lifts that end and pulls it on, over a bump or up a step.
        if heavy(kind) {
            let [_, hy, hz] = spec.half_extents;
            let ride = ride_height(spec);
            let rest = (hy + ride - TRACK_END_CLEAR - spec.wheel_radius).max(0.0);
            let end = WheelTuning { max_suspension_travel: rest, ..tuning };
            for z in [-hz, hz] {
                for side in [-1.0, 1.0] {
                    controller.add_wheel(Vector::new(side * spec.track_half, hy, z), Vector::new(0.0, -1.0, 0.0), Vector::new(1.0, 0.0, 0.0), rest, spec.wheel_radius, &end);
                }
            }
        }
        self.cars.insert(id, Car { spec, body, collider, extra, controller, steer: 0.0, turn: 0.0, spin: 0.0, flight: CarControls::IDLE, air: (0.0, 0.0), condition: CarCondition::NEW, towed_by: None, tank: spec.tank, firing: 0, mounted: None });
    }

    /// Takes vehicle `id` out of the world, with everything hitched to it.
    pub fn remove_car(&mut self, id: u32) {
        self.detach(id);
        let Some(car) = self.cars.remove(&id) else { return };
        self.bodies.remove(car.body, &mut self.islands, &mut self.colliders, &mut self.impulse_joints, &mut self.multibody_joints, &mut self.soft_bodies, true);
    }

    /// Where a car's tow hitch is, in its own frame: just behind its rear face.
    fn tow_anchor(spec: &Spec) -> [f32; 3] {
        let [_, hy, hz] = spec.half_extents;
        [0.0, -hy * 0.5, hz + TOW_BACK]
    }

    /// Where a towed gun's towing eye is, in its own frame: on its centre line at `Spec::tongue`, a
    /// mortar's ahead of it and a Pak's at the end of its trails.
    fn tongue_anchor(spec: &Spec) -> [f32; 3] {
        [0.0, 0.0, spec.tongue]
    }

    /// Hitches mortar `mortar` to car `car`'s tow point, with a spherical joint. Does nothing if the
    /// mortar is already towed. Whether it is in reach is for the caller to check ([`Physics::tow_reach`]).
    pub fn hitch(&mut self, car: u32, mortar: u32) {
        let (Some(towing), Some(towed)) = (self.cars.get(&car), self.cars.get(&mortar)) else { return };
        if towed.towed_by.is_some() {
            return;
        }
        let joint = SphericalJointBuilder::new()
            .local_anchor1(Vector::from(Self::tow_anchor(towing.spec)))
            .local_anchor2(Vector::from(Self::tongue_anchor(towed.spec)))
            .contacts_enabled(false)
            .motor_velocity(JointAxis::AngY, 0.0, HITCH_YAW_DAMPING)
            .build();
        let handle = self.impulse_joints.insert(towing.body, towed.body, joint, true);
        if let Some(towed) = self.cars.get_mut(&mortar) {
            towed.towed_by = Some((car, handle));
        }
    }

    /// Lets mortar `mortar` go, if a car is towing it.
    pub fn unhitch(&mut self, mortar: u32) {
        let Some(towed) = self.cars.get_mut(&mortar) else { return };
        if let Some((_, handle)) = towed.towed_by.take() {
            self.impulse_joints.remove(handle, true);
        }
    }

    /// Takes off whatever is hitched to or from vehicle `id`: its own tow, and any mortar it is towing.
    pub fn detach(&mut self, id: u32) {
        let towed: Vec<u32> = self.cars.iter().filter(|(_, car)| car.towed_by.is_some_and(|(towing, _)| towing == id)).map(|(&mortar, _)| mortar).collect();
        for mortar in towed {
            self.unhitch(mortar);
        }
        self.unhitch(id);
    }

    /// The car that `mortar` is towed by, if any.
    pub fn towed_by(&self, mortar: u32) -> Option<u32> {
        self.cars.get(&mortar)?.towed_by.map(|(car, _)| car)
    }

    /// The mortar that car `car` is towing, if any.
    pub fn towing(&self, car: u32) -> Option<u32> {
        self.cars.iter().find(|(_, towed)| towed.towed_by.is_some_and(|(towing, _)| towing == car)).map(|(&mortar, _)| mortar)
    }

    /// How far apart car `car`'s tow point and mortar `mortar`'s tongue are right now, metres.
    pub fn tow_reach(&self, car: u32, mortar: u32) -> Option<f32> {
        let (towing, towed) = (self.cars.get(&car)?, self.cars.get(&mortar)?);
        let hitch = self.car_pose(car)?.world_point(Self::tow_anchor(towing.spec));
        let tongue = self.car_pose(mortar)?.world_point(Self::tongue_anchor(towed.spec));
        Some(vec3::distance(hitch, tongue))
    }

    /// Puts a car back upright and at rest. Anything hitched to it lets go.
    pub fn reset_car(&mut self, id: u32, position: [f32; 3], yaw: f32) {
        self.detach(id);
        let Some(car) = self.cars.get_mut(&id) else { return };
        car.steer = 0.0;
        car.turn = 0.0;
        car.spin = 0.0;
        car.firing = 0;
        car.tank = car.spec.tank;
        // A turret shot off comes back with the tank.
        for &handle in &car.extra {
            if let Some(collider) = self.colliders.get_mut(handle) {
                collider.set_enabled(true);
            }
        }
        if let Some(body) = self.bodies.get_mut(car.body) {
            body.set_position(pose_of(position, yaw), true);
            body.set_linvel(Vector::ZERO, true);
            body.set_angvel(Vector::ZERO, true);
        }
    }

    /// Puts a tank's turret solid back (`true`) or takes it away (`false`): it comes off when shot off.
    pub fn set_turret(&mut self, id: u32, present: bool) {
        let Some(car) = self.cars.get(&id) else { return };
        for handle in with_role(car.spec, &car.extra, Role::Turret) {
            if let Some(collider) = self.colliders.get_mut(handle) {
                collider.set_enabled(present);
            }
        }
    }

    /// Test helper: rolls a car about its forward axis and moves it.
    #[cfg(any(test, feature = "testing"))]
    pub fn set_car_roll(&mut self, id: u32, roll: f32, position: [f32; 3]) {
        let Some(car) = self.cars.get(&id) else { return };
        if let Some(body) = self.bodies.get_mut(car.body) {
            body.set_position(Pose::from_parts(Vector::from(position), Rotation::from_rotation_z(roll)), true);
            body.set_linvel(Vector::ZERO, true);
            body.set_angvel(Vector::ZERO, true);
        }
    }

    /// Points vehicle `id` `pitch` radians up from level at `yaw`, at `position`, moving along its nose
    /// at `speed`: a missile leaving its tube, or a test's dive.
    pub fn set_car_dive(&mut self, id: u32, position: [f32; 3], yaw: f32, pitch: f32, speed: f32) {
        let Some(car) = self.cars.get(&id) else { return };
        if let Some(body) = self.bodies.get_mut(car.body) {
            let rotation = Rotation::from_rotation_y(yaw) * Rotation::from_rotation_x(pitch);
            body.set_position(Pose::from_parts(Vector::from(position), rotation), true);
            body.set_linvel(rotation * Vector::new(0.0, 0.0, -speed), true);
            body.set_angvel(Vector::ZERO, true);
        }
    }

    /// Sets vehicle `id`'s linear velocity (m/s), leaving its spin alone.
    pub fn set_car_velocity(&mut self, id: u32, velocity: [f32; 3]) {
        let Some(car) = self.cars.get(&id) else { return };
        if let Some(body) = self.bodies.get_mut(car.body) {
            body.set_linvel(Vector::from(velocity), true);
        }
    }

    /// How worn vehicle `id` is from now on; applied by [`Physics::drive_car`].
    /// Puts vehicle `id` in the state a snapshot reports for it: its pose, its motion and, for a kind
    /// that has one, its tank (`None` leaves the tank as it is). The client predicts its own vehicle
    /// from there.
    pub fn set_car_state(&mut self, id: u32, position: [f32; 3], rotation: [f32; 4], motion: CarMotion, tank: Option<f32>) {
        let Some(car) = self.cars.get_mut(&id) else { return };
        car.steer = motion.steer;
        car.spin = motion.spin;
        if let (Some(tank), Some(slot)) = (tank, car.tank.as_mut()) {
            *slot = tank;
        }
        if let Some(body) = self.bodies.get_mut(car.body) {
            let rotation = Rotation::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]).normalize();
            body.set_position(Pose::from_parts(Vector::from(position), rotation), true);
            body.set_linvel(Vector::from(motion.linvel), true);
            body.set_angvel(Vector::from(motion.angvel), true);
        }
    }

    pub fn set_car_condition(&mut self, id: u32, condition: CarCondition) {
        if let Some(car) = self.cars.get_mut(&id) {
            car.condition = condition;
        }
    }

    /// Applies the driver's controls to the wheels for the next [`Physics::step_cars`].
    pub fn drive_car(&mut self, id: u32, controls: CarControls, dt: f32) {
        let Some(car) = self.cars.get_mut(&id) else { return };
        if matches!(car.spec.lift, Lift::Rotor | Lift::Walker | Lift::Drone | Lift::Loiter | Lift::Missile | Lift::Sail | Lift::Dive | Lift::Mounted) {
            car.flight = controls;
            return;
        }
        let Some(body) = self.bodies.get(car.body) else { return };
        let mass = body.mass();
        let forward = body.rotation() * Vector::new(0.0, 0.0, -1.0);
        let speed = forward.dot(body.linvel());
        if car.spec.lift == Lift::Wing {
            car.flight = controls;
            roll_plane(car, controls, body, speed, mass, dt);
            return;
        }

        // Steering eases towards its target, and the lock narrows at speed.
        let lock = lock(car.spec, speed);
        let target = controls.steer.clamp(-1.0, 1.0) * lock;
        let returning = target.abs() < car.steer.abs() || target * car.steer < 0.0;
        // A kind with a `steer_time` swings a share of its lock each second rather than a set angle.
        let rate = if car.spec.steer_time > 0.0 { lock / car.spec.steer_time } else { STEER_RATE };
        let step = rate * if returning { STEER_RETURN } else { 1.0 } * dt;
        car.steer += (target - car.steer).clamp(-step, step);
        car.turn = controls.steer.clamp(-1.0, 1.0) * car.spec.turn_rate * car.condition.turn;
        car.air = (controls.throttle.clamp(-1.0, 1.0), controls.steer.clamp(-1.0, 1.0));
        car.flight = controls;

        // Pushing against the direction of travel brakes instead of reversing the engine.
        let against = controls.throttle * speed < -1.0;
        let (engine, brake) = if controls.throttle == 0.0 {
            // A sled's runners have no rolling to resist: its runners' friction slows it instead (`on_runners`).
            (0.0, if car.spec.runners.is_some() { 0.0 } else { COAST_BRAKE_PER_KG * mass })
        } else if against {
            (0.0, BRAKE_PER_KG * mass)
        } else {
            // A snowmobile's boost opens the engine right up while there is any left.
            let boosted = car.spec.buttons == Buttons::Sled && boosting(controls, car.tank);
            let (power, top) = if boosted { (SNOWMOBILE_BOOST_POWER, SNOWMOBILE_BOOST_SPEED) } else { (1.0, 1.0) };
            let headroom = if controls.throttle > 0.0 { pull(speed, car.spec.max_speed * top * car.condition.speed) } else { 0.5 };
            (controls.throttle * power * car.spec.engine_force / road_wheels(car.spec) as f32 * headroom * car.condition.power, 0.0)
        };
        let handbrake = if controls.handbrake { BRAKE_PER_KG * mass } else { 0.0 };

        // Only a car's first axle steers and is spared the handbrake, which also lets its tail slide.
        let front_wheels = if car.spec.steer_lock > 0.0 { 2 } else { 0 };
        let drift = controls.handbrake && front_wheels > 0;
        // A kind that slides lets its rear go in a hard turn at speed, the more so under power: grip
        // falls away towards the handbrake's, geometrically, since even a third of it holds firm.
        // How hard it is turned is how far the wheels have swung, not how far the stick is pushed.
        let turned = turned(car.spec, car.steer, speed);
        let hard = ((turned - SLIDE_STEER) / (1.0 - SLIDE_STEER)).clamp(0.0, 1.0);
        let fast = ((speed.abs() - SLIDE_SPEEDS[0]) / (SLIDE_SPEEDS[1] - SLIDE_SPEEDS[0])).clamp(0.0, 1.0)
            * (1.0 - ((speed.abs() - SLIDE_SPEEDS[2]) / (SLIDE_SPEEDS[3] - SLIDE_SPEEDS[2])).clamp(0.0, 1.0));
        // And while it is still sliding it only finds its grip again as the slide dies, rather than
        // catching all at once and throwing itself over.
        let sideways = (body.rotation() * Vector::new(1.0, 0.0, 0.0)).dot(body.linvel());
        let slip = sideways.abs().atan2(speed.abs());
        let sliding = ((slip - SLIDE_SLIP[0]) / (SLIDE_SLIP[1] - SLIDE_SLIP[0])).clamp(0.0, 1.0);
        let carve = car.spec.slide * (hard * fast * (0.6 + 0.4 * controls.throttle.max(0.0))).max(sliding);
        let rear_grip = SIDE_GRIP * (DRIFT_GRIP / SIDE_GRIP).powf(carve);
        let (spec, condition, steer) = (car.spec, car.condition, car.steer);
        for (index, wheel) in car.controller.wheels_mut().iter_mut().enumerate() {
            let front = index < front_wheels;
            // Only a car's four wheels take damage. A wrecked one points off to its own side and grips less.
            let hurt = if front_wheels > 0 { condition.wheels.get(index).copied().unwrap_or(0) } else { 0 };
            let wear = f32::from(hurt) / f32::from(WHEEL_GONE);
            let gone = hurt == WHEEL_GONE;
            let toe = if index % 2 == 0 { TOE_MAX } else { -TOE_MAX } * wear;
            // Wheels are added front pair first, left before right. The steering angle is negated:
            // +angle turns the wheel about the controller's up axis, which turns the car left.
            wheel.steering = if front { -steer } else { 0.0 } + toe;
            // Wheels are added left before right: a broken track's side is not driven, and drags.
            let side = condition.sides[index % 2];
            wheel.engine_force = if gone { 0.0 } else { engine * side };
            let drag = (1.0 - side) * BROKEN_TRACK_DRAG * BRAKE_PER_KG * mass;
            wheel.brake = if gone { 0.0 } else if front { brake } else { brake.max(handbrake).max(drag) };
            wheel.side_friction_stiffness = if front { SIDE_GRIP } else if drift { DRIFT_GRIP } else { rear_grip } * (1.0 - WHEEL_GRIP_LOSS * wear);
            wheel.friction_slip = FRICTION_SLIP * (1.0 - WHEEL_SLIP_LOSS * wear);
            // A track's end wheels keep the reach they were hung with.
            if index < road_wheels(spec) {
                wheel.radius = if gone { LOST_WHEEL_RADIUS } else { spec.wheel_radius };
                wheel.suspension_rest_length = if gone { 0.0 } else { spec.suspension_rest };
            }
        }
    }

    /// Takes destroyed buildings and trees out of the collider set. With no car the pipeline does
    /// not run otherwise, and the broad-phase would keep pointing at colliders that are gone.
    pub(crate) fn apply_removals(&mut self) {
        if self.removals.is_empty() && self.replacements.is_empty() && self.construct_shapes.is_empty() && self.terrain_shapes.is_empty() {
            return;
        }
        for (block, collider) in std::mem::take(&mut self.terrain_shapes) {
            if let Some(old) = self.terrain.remove(&block) {
                self.colliders.remove(old, &mut self.islands, &mut self.bodies, &mut self.soft_bodies, false);
            }
            let handle = self.colliders.insert(collider);
            self.terrain.insert(block, handle);
        }
        for (chunk, shape) in std::mem::take(&mut self.construct_shapes) {
            if let Some(old) = self.constructs.remove(&chunk) {
                self.mark_changed(old);
                self.construct_chunks.remove(&old);
                self.colliders.remove(old, &mut self.islands, &mut self.bodies, &mut self.soft_bodies, false);
            }
            if let Some(shape) = shape {
                let handle = self.colliders.insert(ColliderBuilder::new(shape).build());
                self.mark_changed(handle);
                self.constructs.insert(chunk, handle);
                self.construct_chunks.insert(handle, chunk);
            }
        }
        // Bitten buildings get their new shape first; one that is also coming down just goes.
        for (object, shape) in std::mem::take(&mut self.replacements) {
            if self.removals.contains(&object) {
                continue;
            }
            let Some(&old) = self.handles.get(&object) else { continue };
            // A building with nothing left is not a collider at all.
            let Some(shape) = shape else {
                self.removals.push(object);
                continue;
            };
            self.mark_changed(old);
            self.objects.remove(&old);
            self.colliders.remove(old, &mut self.islands, &mut self.bodies, &mut self.soft_bodies, false);
            let handle = self.colliders.insert(ColliderBuilder::new(shape).build());
            self.objects.insert(handle, object);
            self.handles.insert(object, handle);
        }
        for object in std::mem::take(&mut self.removals) {
            if let Some(handle) = self.handles.remove(&object) {
                self.mark_changed(handle);
                self.objects.remove(&handle);
                self.colliders.remove(handle, &mut self.islands, &mut self.bodies, &mut self.soft_bodies, false);
            }
        }
        if self.cars.is_empty() {
            self.step_pipeline();
        }
    }

    /// Advances every car and the rigid-body world by one tick.
    pub fn step_cars(&mut self, dt: f32) {
        self.apply_removals();
        let waves = shared::waves::time(self.tick);
        let wind = shared::wind::wind(self.tick);
        self.tick = self.tick.wrapping_add(1);
        if self.cars.is_empty() {
            return;
        }
        // The ground under each flier and plane, which their ceilings are measured from.
        let floors: std::collections::HashMap<u32, f32> = self
            .cars
            .iter()
            .filter(|(_, car)| matches!(car.spec.lift, Lift::Rotor | Lift::Drone | Lift::Loiter | Lift::Missile | Lift::Wing) || car.tank.is_some())
            .filter_map(|(&id, car)| Some((id, self.terrain_below(self.bodies.get(car.body)?.translation()))))
            .collect();
        let floor = |id: &u32| floors.get(id).copied().unwrap_or(0.0);
        for (id, car) in self.cars.iter().filter(|(_, car)| matches!(car.spec.lift, Lift::Rotor | Lift::Drone | Lift::Loiter | Lift::Missile)) {
            if let Some(body) = self.bodies.get_mut(car.body) {
                // Forces persist from step to step until reset.
                body.reset_forces(false);
                // Unpowered, or with its engine shot dead, the rotor stops and it falls.
                if !car.flight.powered || car.condition.power == 0.0 {
                    continue;
                }
                if car.spec.lift == Lift::Drone {
                    fly_drone(body, car.flight, floor(id), dt);
                } else if car.spec.lift == Lift::Loiter {
                    glide(body, LOITER_GLIDE, car.flight, floor(id), dt);
                } else if car.spec.lift == Lift::Missile {
                    glide(body, MISSILE_GLIDE, car.flight, floor(id), dt);
                } else {
                    fly(body, car.flight, floor(id), dt);
                }
            }
        }
        // A walker's feet: how far below each hip the ground is.
        let mut stances = Vec::new();
        for (&id, car) in self.cars.iter().filter(|(_, car)| car.spec.lift == Lift::Walker) {
            let Some(body) = self.bodies.get(car.body) else { continue };
            let [_, hy, _] = car.spec.half_extents;
            let filter = QueryFilter::default().exclude_rigid_body(car.body);
            let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, filter);
            let feet = [-1.0, 1.0].map(|side| {
                let hip = body.translation() + *body.rotation() * Vector::new(side * WALKER_FOOT_HALF, -hy, 0.0);
                queries.cast_ray(&Ray::new(hip, Vector::new(0.0, -1.0, 0.0)), WALKER_LEG_LENGTH, true).map(|(_, distance)| distance)
            });
            stances.push((id, feet));
        }
        for (id, feet) in stances {
            let Some(car) = self.cars.get_mut(&id) else { continue };
            let Some(body) = self.bodies.get_mut(car.body) else { continue };
            // A wreck (no health, which is exactly half power) has let go.
            let standing = car.condition.power > 0.5;
            walk(body, car.flight, standing, feet);
            // A hen's wings, after the legs, which reset the forces.
            if let Some(stamina) = &mut car.tank {
                let grounded = feet.iter().any(|reach| reach.is_some_and(|distance| distance < WALKER_LEG_LENGTH));
                car.firing = flap(body, car.flight, stamina, standing, grounded, dt);
            }
        }
        // A ship floats on whatever water it is on, and sails if there is enough of it under her keel.
        for car in self.cars.values().filter(|car| car.spec.lift == Lift::Sail) {
            let Some(body) = self.bodies.get_mut(car.body) else { continue };
            let water = water_under(&self.waters, waves, body, car.spec);
            sail(body, car.spec, car.flight, car.condition, water.as_ref(), wind, dt);
        }
        // A submarine floats on the surface or holds whatever depth its captain takes it to.
        for car in self.cars.values().filter(|car| car.spec.lift == Lift::Dive) {
            let Some(body) = self.bodies.get_mut(car.body) else { continue };
            let water = water_under(&self.waters, waves, body, car.spec);
            dive(body, car.spec, car.flight, car.condition, water.as_ref(), dt);
        }
        for car in self.cars.values_mut().filter(|car| car.spec.lift.wheeled()) {
            // Wheel rays must not hit their own chassis, nor a tracked hull's the trees it goes through.
            let mut filter = QueryFilter::default().exclude_rigid_body(car.body);
            if tracked(car.spec) {
                filter = filter.groups(super::plows_trees());
            }
            let queries = self.broad_phase.as_query_pipeline_mut(
                self.narrow_phase.query_dispatcher(),
                &mut self.bodies,
                &mut self.colliders,
                filter,
            );
            car.controller.update_vehicle(dt, queries);
            if car.spec.roll > 0.0 {
                restore_roll(car, &mut self.bodies);
            }
        }
        // A car is pressed down or tilted in the air. A tank turns on the spot, but only while some
        // track is on the ground. The wheels' side grip fights any spin every tick, so the rate is
        // kept here and written back each time.
        for (id, car) in self.cars.iter_mut().filter(|(_, car)| car.spec.lift.wheeled()) {
            let mut grounded = car.controller.wheels().iter().any(|wheel| wheel.raycast_info().is_in_contact);
            let Some(body) = self.bodies.get_mut(car.body) else { continue };
            if let Some(airframe) = car.spec.airframe.as_ref().filter(|_| car.spec.lift == Lift::Wing) {
                body.reset_forces(false);
                // A seaplane's pontoons carry it on open water, which is as good as being on the ground.
                if let Some(floats) = &airframe.floats
                    && let Some(water) = water_under(&self.waters, waves, body, car.spec)
                {
                    grounded |= float(body, airframe, floats, car.flight, car.condition, &water, dt);
                }
                wing(body, airframe, car.flight, car.condition.power, grounded, self.bound, floor(id), dt);
                continue;
            }
            // Open water holds it up while it skims, which is as good as being on the ground.
            if let Some(water) = water_under(&self.waters, waves, body, car.spec) {
                let boosted = car.spec.buttons == Buttons::Sled && boosting(car.flight, car.tank);
                grounded |= wade(body, car.spec, car.flight, car.steer, car.condition, boosted, &water, dt);
            }
            // A sled has no downforce to hold it down over a crest, and steers itself in the air.
            if let Some(runners) = &car.spec.runners {
                on_runners(body, runners, car.air, grounded, dt);
            } else if car.spec.steer_lock > 0.0 {
                assist(body, car.air, grounded, dt);
            }
            let speed = body.linvel().dot(*body.rotation() * Vector::new(0.0, 0.0, -1.0));
            if car.spec.slide > 0.0 && grounded && turned(car.spec, car.steer, speed) < SLIDE_STEER {
                let mut spin = body.angvel();
                spin.y *= (-SLIDE_CATCH * dt).exp();
                body.set_angvel(spin, true);
            }
            if let Some(tank) = &mut car.tank {
                if car.spec.buttons == Buttons::Sled {
                    car.firing = boost(car.flight, tank, dt);
                } else {
                    body.reset_forces(false);
                    car.firing = thrust(body, car.flight, tank, grounded, floor(id), dt);
                }
            }
            // A tracked hull on the ground holds whatever turn its driver asks for, none included: its
            // tracks keep it from swinging round downhill on a side slope. Anything else is left alone
            // when not turned.
            if !grounded || (car.turn == 0.0 && !tracked(car.spec)) {
                car.spin = 0.0;
                continue;
            }
            car.spin += (car.turn - car.spin).clamp(-TURN_ACCEL * dt, TURN_ACCEL * dt);
            let mut spin = body.angvel();
            // Clockwise seen from above is a negative rotation about +Y.
            spin.y = -car.spin;
            body.set_angvel(spin, true);
        }
        self.step_pipeline();
        self.carry_mounted();
    }

    /// Fixes vehicle `id` (a ship's gun) to vehicle `on` (its ship) at `at` in `on`'s frame, turned
    /// `yaw` there, and puts it there. From now on it goes wherever that spot goes, after every step,
    /// and nothing else moves it; it touches nothing, but what moves is still stopped by it, and shots
    /// still meet it. Whatever kicks it kicks what it is fixed to.
    pub fn mount_car(&mut self, id: u32, on: u32, at: [f32; 3], yaw: f32) {
        let Some(car) = self.cars.get_mut(&id) else { return };
        car.mounted = Some(Mounted { on, at: pose_of(at, yaw) });
        if let Some(body) = self.bodies.get_mut(car.body) {
            body.set_body_type(RigidBodyType::KinematicPositionBased, true);
        }
        for &handle in std::iter::once(&car.collider).chain(&car.extra) {
            if let Some(collider) = self.colliders.get_mut(handle) {
                collider.set_collision_groups(InteractionGroups::none());
            }
        }
        self.carry_mounted();
    }

    /// The vehicle `id` is fixed to, if it is.
    pub fn mounted_on(&self, id: u32) -> Option<u32> {
        self.cars.get(&id)?.mounted.map(|mounted| mounted.on)
    }

    /// The vehicles fixed to vehicle `id`, in id order.
    pub fn fittings_of(&self, id: u32) -> Vec<u32> {
        self.cars.iter().filter(|(_, car)| car.mounted.is_some_and(|mounted| mounted.on == id)).map(|(&fitting, _)| fitting).collect()
    }

    /// Puts every fixed vehicle where it is fixed, on what it is fixed to as that is now.
    fn carry_mounted(&mut self) {
        let placed: Vec<(RigidBodyHandle, Pose)> = self
            .cars
            .values()
            .filter_map(|car| {
                let mounted = car.mounted?;
                let on = self.bodies.get(self.cars.get(&mounted.on)?.body)?;
                Some((car.body, *on.position() * mounted.at))
            })
            .collect();
        for (handle, pose) in placed {
            if let Some(body) = self.bodies.get_mut(handle) {
                body.set_position(pose, true);
            }
        }
    }

    /// Every `(car, tree)` pair in contact after the last step: what a car has driven into.
    pub fn car_tree_contacts(&self) -> Vec<(u32, Obj)> {
        let mut found = Vec::new();
        for (&id, car) in &self.cars {
            for &handle in std::iter::once(&car.collider).chain(&car.extra) {
                for pair in self.narrow_phase.contact_pairs_with(handle) {
                    if !pair.has_any_active_contact() {
                        continue;
                    }
                    let other = if pair.collider1 == handle { pair.collider2 } else { pair.collider1 };
                    if let Some(object @ Obj::Tree(_)) = self.objects.get(&other).copied() {
                        found.push((id, object));
                    }
                }
            }
        }
        found
    }

    /// Every contact of a vehicle's solids with anything after the last step, with how hard it pushed.
    pub fn car_impacts(&self) -> Vec<CarImpact> {
        let mut found = Vec::new();
        for (&id, car) in &self.cars {
            let Some(body) = self.bodies.get(car.body) else { continue };
            for &handle in std::iter::once(&car.collider).chain(&car.extra) {
                let Some(collider) = self.colliders.get(handle) else { continue };
                // A contact point is in the collider's frame, which for the turret is off the body's.
                let offset = collider.position_wrt_parent().copied().unwrap_or(Pose::IDENTITY);
                for pair in self.narrow_phase.contact_pairs_with(handle) {
                    if !pair.has_any_active_contact() {
                        continue;
                    }
                    // The middle of the touching points: a flat hit lands mid-face, not on a corner.
                    let ours = pair.collider1 == handle;
                    let other = if ours { pair.collider2 } else { pair.collider1 };
                    let (mut sum, mut count) = (Vector::ZERO, 0.0);
                    for manifold in pair.manifolds() {
                        for contact in manifold.points.iter().filter(|contact| contact.dist < 0.02) {
                            sum += if ours { contact.local_p1 } else { contact.local_p2 };
                            count += 1.0;
                        }
                    }
                    if count == 0.0 {
                        continue;
                    }
                    let local = offset * (sum / count);
                    let forward = body.rotation() * Vector::new(0.0, 0.0, -1.0);
                    let other_car = self.cars.iter().find(|(_, car)| car.owns(other)).map(|(&other, _)| other);
                    found.push(CarImpact {
                        car: id,
                        other_car,
                        object: self.objects.get(&other).copied(),
                        construct: self.construct_chunks.contains_key(&other),
                        local: local.into(),
                        point: (body.position() * local).into(),
                        dv: pair.total_impulse_magnitude() / body.mass(),
                        stopped: pair.total_impulse().dot(forward).abs() / body.mass(),
                        ground: self.colliders.get(other).is_some_and(|collider| collider.shape().as_heightfield().is_some()),
                    });
                }
            }
        }
        found
    }

    /// The car, if any, overlapping a standing character capsule centred at `position`.
    pub fn car_hit(&self, position: [f32; 3]) -> Option<CarHit> {
        self.car_touching(position)
    }

    /// The car, if any, overlapping an upright capsule centred at `centre`: what a creature that is
    /// not a soldier is run down by.
    pub fn car_hit_capsule(&self, centre: [f32; 3], half_height: f32, radius: f32) -> Option<CarHit> {
        self.car_touching_shape(centre, SharedShape::capsule_y(half_height, radius).as_ref())
    }

    /// The car, if any, that a character centred at `position` is standing on.
    pub fn car_under(&self, position: [f32; 3]) -> Option<CarHit> {
        self.car_touching([position[0], position[1] - RIDE_PROBE, position[2]])
    }

    fn car_touching(&self, position: [f32; 3]) -> Option<CarHit> {
        self.car_touching_shape(position, self.character_shape.shape())
    }

    fn car_touching_shape(&self, position: [f32; 3], shape: &dyn rapier3d::geometry::Shape) -> Option<CarHit> {
        let queries = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            QueryFilter::only_dynamic(),
        );
        let (hit, _) = queries.intersect_shape(Pose::from_translation(Vector::from(position)), shape).next()?;
        let (&car, body) = self.cars.iter().find(|(_, car)| car.owns(hit)).and_then(|(id, car)| Some((id, self.bodies.get(car.body)?)))?;
        Some(CarHit { car, position: body.translation().into(), velocity: body.linvel().into() })
    }

    /// Height of the terrain under `at`, metres: what fliers' ceilings are measured from. Buildings
    /// and bridges do not count, so a helicopter holds its height over the rooftops. 0 on a flat
    /// world, and where the tile under `at` has not streamed in.
    fn terrain_below(&self, at: Vector) -> f32 {
        let terrain = |_: ColliderHandle, collider: &Collider| collider.shape().as_heightfield().is_some();
        let filter = QueryFilter::only_fixed().predicate(&terrain);
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, filter);
        // From above the highest ground any world may have, so a flier sunk into a hill still finds it.
        let ray = Ray::new(Vector::new(at.x, TERRAIN_TOP, at.z), Vector::new(0.0, -1.0, 0.0));
        queries.cast_ray(&ray, TERRAIN_TOP + RAY_FLOOR, true).map_or(0.0, |(_, distance)| TERRAIN_TOP - distance)
    }

    /// How far a ray from `origin` along the unit vector `dir` travels before it hits the world or a
    /// vehicle other than `ignore`, up to `max`. `None` when it reaches `max` clear.
    pub fn cast_ray(&self, origin: [f32; 3], dir: [f32; 3], max: f32, ignore: u32) -> Option<f32> {
        self.cast_ray_hit(origin, dir, max, ignore).map(|(distance, _)| distance)
    }

    /// Like [`Self::cast_ray`], and also says which vehicle it hit, if it was a vehicle and not the world.
    pub fn cast_ray_hit(&self, origin: [f32; 3], dir: [f32; 3], max: f32, ignore: u32) -> Option<(f32, Option<u32>)> {
        let mut filter = QueryFilter::default();
        // A ship's gun fires over its own ship's side, and its shot never hits it.
        let host = self.cars.get(&ignore).and_then(|car| car.mounted).and_then(|mounted| self.cars.get(&mounted.on)).map(|on| on.body);
        let not_host = |_: ColliderHandle, collider: &Collider| collider.parent().is_none() || collider.parent() != host;
        if let Some(car) = self.cars.get(&ignore) {
            filter = filter.exclude_rigid_body(car.body);
            if host.is_some() {
                filter = filter.predicate(&not_host);
            }
        }
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, filter);
        let ray = Ray::new(Vector::from(origin), Vector::from(dir));
        let (collider, distance) = queries.cast_ray(&ray, max, true)?;
        let car = self.cars.iter().find(|(_, car)| car.owns(collider)).map(|(&id, _)| id);
        Some((distance, car))
    }

    /// Where a ray from `origin` along the unit `dir` meets vehicle `id`'s own solids within `max`, for armor
    /// to judge: how far, which solid it was (`None` for the hull), and the surface normal and the point in
    /// the vehicle's frame. A ray that starts inside a solid has a zero normal.
    pub fn armor_strike(&self, origin: [f32; 3], dir: [f32; 3], max: f32, id: u32) -> Option<Strike> {
        let car = self.cars.get(&id)?;
        let body = self.bodies.get(car.body)?;
        let mine = |handle: ColliderHandle, _: &Collider| car.owns(handle);
        let filter = QueryFilter::default().predicate(&mine);
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, filter);
        let ray = Ray::new(Vector::from(origin), Vector::from(dir));
        let (collider, hit) = queries.cast_ray_and_get_normal(&ray, max, true)?;
        let to_local = body.rotation().inverse();
        let solid = car.extra.iter().position(|&handle| handle == collider).map(|index| index as u8);
        // A tracked hull's armour is laid out on its box, not on the swept-up ends it drives on: a shot
        // into one meets the face of the box it comes in through, where that face is. The box is cast
        // from a box's breadth back, as the shot may already be inside it but short of the hull.
        if solid.is_none() && tracked(car.spec) {
            let half = Vector::from(car.spec.half_extents);
            let back = 2.0 * half.length();
            let along = to_local * Vector::from(dir);
            let start = to_local * (Vector::from(origin) - body.translation());
            let face = Cuboid::new(half).cast_local_ray_and_get_normal(&Ray::new(start - along * back, along), max + back, true);
            if let Some(face) = face.filter(|face| face.normal.length_squared() > 0.5) {
                let distance = (face.time_of_impact - back).max(0.0);
                return Some(Strike { distance, solid, normal: face.normal.into(), point: (start - along * back + along * face.time_of_impact).into() });
            }
        }
        let point = Vector::from(origin) + Vector::from(dir) * hit.time_of_impact;
        Some(Strike {
            distance: hit.time_of_impact,
            solid,
            normal: (to_local * hit.normal).into(),
            point: (to_local * (point - body.translation())).into(),
        })
    }

    /// How far a ray travels before it hits the world, and which building or tree that was, if it
    /// was one. Vehicles are not ignored.
    pub fn cast_ray_object(&self, origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<(f32, Option<super::Obj>)> {
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, QueryFilter::default());
        let ray = Ray::new(Vector::from(origin), Vector::from(dir));
        let (collider, distance) = queries.cast_ray(&ray, max, true)?;
        Some((distance, self.object_of(collider)))
    }

    /// Whether a ray from `origin` along the unit `dir` meets a chunk of placed cubes first, within `max`.
    pub fn ray_meets_construct(&self, origin: [f32; 3], dir: [f32; 3], max: f32) -> bool {
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, QueryFilter::default());
        let ray = Ray::new(Vector::from(origin), Vector::from(dir));
        queries.cast_ray(&ray, max, true).is_some_and(|(collider, _)| self.is_construct(collider))
    }

    /// How far a ray travels before it meets the world or a vehicle, and the surface normal there.
    pub fn cast_ray_normal(&self, origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<(f32, [f32; 3])> {
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, QueryFilter::default());
        let ray = Ray::new(Vector::from(origin), Vector::from(dir));
        let (_, hit) = queries.cast_ray_and_get_normal(&ray, max, true)?;
        Some((hit.time_of_impact, hit.normal.into()))
    }

    /// Changes vehicle `id`'s velocity by `delta` m/s at once, as a gun's recoil does.
    pub fn kick_car(&mut self, id: u32, delta: [f32; 3]) {
        let Some(car) = self.cars.get(&id) else { return };
        // A fixed gun cannot move: what it is fixed to takes the kick, where the gun stands.
        if let Some(mounted) = car.mounted {
            let [hx, hy, hz] = car.spec.half_extents;
            let mass = car.spec.density * 8.0 * hx * hy * hz;
            let at = self.bodies.get(car.body).map(|body| body.translation());
            if let (Some(at), Some(on)) = (at, self.cars.get(&mounted.on)) && let Some(body) = self.bodies.get_mut(on.body) {
                body.apply_impulse_at_point(Vector::from(delta) * mass, at, true);
            }
            return;
        }
        if let Some(body) = self.bodies.get_mut(car.body) {
            body.apply_impulse(Vector::from(delta) * body.mass(), true);
        }
    }

    /// Adds `spin` rad/s to vehicle `id`'s angular velocity, given in its own frame (+X right,
    /// +Z back), so a wreck can be sent end over end or onto its side.
    pub fn spin_car(&mut self, id: u32, spin: [f32; 3]) {
        let Some(car) = self.cars.get(&id) else { return };
        if let Some(body) = self.bodies.get_mut(car.body) {
            let world = *body.rotation() * Vector::from(spin);
            let now = body.angvel();
            body.set_angvel(now + world, true);
        }
    }

    /// Shoves every vehicle within `radius` of `point` away from it, harder the closer it is.
    /// Returns each one it reached with how close it was: 1 at the point, 0 at the edge.
    pub fn blast_cars(&mut self, point: [f32; 3], radius: f32) -> Vec<(u32, f32)> {
        let point = Vector::from(point);
        let mut reached = Vec::new();
        for (&id, car) in &self.cars {
            let Some(body) = self.bodies.get_mut(car.body) else { continue };
            let away = body.translation() - point;
            let distance = away.length();
            if distance >= radius {
                continue;
            }
            let falloff = 1.0 - distance / radius;
            let direction = (away / distance.max(1e-3) + Vector::new(0.0, 0.4, 0.0)).normalize();
            let impulse = direction * falloff * BLAST_VELOCITY * body.mass();
            if car.spec.lift == Lift::Walker {
                // A tall hull catches it high up, which tips it as much as it pushes it.
                let shoulders = body.translation() + *body.rotation() * Vector::new(0.0, WALKER_BLAST_LEVER, 0.0);
                body.apply_impulse_at_point(impulse, shoulders, true);
            } else {
                body.apply_impulse(impulse, true);
            }
            reached.push((id, falloff));
        }
        reached
    }

    pub fn car_motion(&self, id: u32) -> Option<CarMotion> {
        let car = self.cars.get(&id)?;
        let body = self.bodies.get(car.body)?;
        Some(CarMotion { linvel: body.linvel().into(), angvel: body.angvel().into(), steer: car.steer, spin: car.spin })
    }

    /// A Roadster's air tank or a hen's stamina (share of full) and the `THRUST_*` flags of the last step.
    pub fn car_thrust(&self, id: u32) -> Option<(f32, u8)> {
        let car = self.cars.get(&id)?;
        Some((car.tank?, car.firing))
    }

    pub fn car_pose(&self, id: u32) -> Option<CarPose> {
        let car = self.cars.get(&id)?;
        let body = self.bodies.get(car.body)?;
        let rotation = *body.rotation();
        let forward = rotation * Vector::new(0.0, 0.0, -1.0);
        // A fixed gun goes as fast as the spot it is fixed to.
        let velocity = match car.mounted.and_then(|mounted| self.cars.get(&mounted.on)).and_then(|on| self.bodies.get(on.body)) {
            Some(on) => on.velocity_at_point(body.translation()),
            None => body.linvel(),
        };
        Some(CarPose {
            position: body.translation().into(),
            rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
            forward_speed: forward.dot(velocity),
            steer: if car.spec.steer_lock > 0.0 { car.steer / car.spec.steer_lock } else { 0.0 },
            up_y: (rotation * Vector::new(0.0, 1.0, 0.0)).y,
            velocity: velocity.into(),
        })
    }
}

#[cfg(test)]
mod tests;

/// Rapier's ray-cast wheels push sideways from a tenth of the way down from the centre of mass to
/// the ground (their private `roll_influence`), so their side grip barely rolls the chassis. This
/// gives back `Spec::roll` of the rest of that leverage, as a twist about the centre of mass.
fn restore_roll(car: &Car, bodies: &mut RigidBodySet) {
    let Some(body) = bodies.get_mut(car.body) else { return };
    let up = *body.rotation() * Vector::new(0.0, 1.0, 0.0);
    let centre = body.center_of_mass();
    let mut twist = Vector::ZERO;
    for wheel in car.controller.wheels() {
        let contact = wheel.raycast_info();
        if !contact.is_in_contact || wheel.side_impulse == 0.0 {
            continue;
        }
        let normal = contact.contact_normal_ws;
        let side = (wheel.axle() - normal * wheel.axle().dot(normal)).normalize_or_zero();
        let below = up * up.dot(contact.contact_point_ws - centre) * (1.0 - RAPIER_ROLL_INFLUENCE);
        twist += below.cross(side * wheel.side_impulse);
    }
    body.apply_torque_impulse(twist * car.spec.roll, true);
}
