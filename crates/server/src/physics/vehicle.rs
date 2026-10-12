//! Ground vehicles: dynamic rigid bodies with ray-cast wheels (Rapier's `DynamicRayCastVehicleController`).
//!
//! The chassis is a box; each wheel is a suspension ray that pushes the chassis up and applies
//! engine, brake and side-grip impulses at its contact point. Body-local axes are +X right,
//! +Y up and -Z forward, so a yaw of 0 faces -Z exactly like a stickman.
//!
//! A car steers its front wheels. A tank has six unsteered wheels and turns on the spot: the
//! physics sets its yaw rate directly, which is how skid steering looks from the outside.
//! A helicopter has no wheels at all: its rotor is a push along the hull's up axis, and the hull is
//! levelled and turned by setting its angular velocity, so it is flown by leaning (see [`fly`]).
//! A plane rolls on wheels like a car, with no engine on them: its propeller pushes the body along the
//! nose, and its wings hold it up once it is fast enough (see [`wing`]).
//! "Car" in the names below means any of these vehicles; [`Spec`] says which one.

use super::{Obj, Physics};
use rapier3d::control::{DynamicRayCastVehicleController, WheelTuning};
use rapier3d::prelude::*;
use shared::consts::*;
use shared::protocol::{DAMAGE_WHEELS, KIND_HELI, KIND_PLANE, KIND_SPORTS, KIND_TANK, WHEEL_GONE, vehicle_hp};

/// Gravity acting on vehicle bodies. Stickmen use `GRAVITY`; the ray-cast suspension is tuned
/// around the real-world value.
pub const CAR_GRAVITY: f32 = 9.81;

/// What holds a vehicle up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Lift {
    /// Its wheels, and nothing else.
    Wheels,
    /// A rotor, with no wheels at all.
    Rotor,
    /// Wheels on the ground and wings in the air, with a propeller to push it.
    Wing,
}

/// What differs between vehicle kinds.
pub(super) struct Spec {
    pub half_extents: [f32; 3],
    /// Chassis density in kg/m³.
    density: f32,
    wheel_radius: f32,
    track_half: f32,
    /// Z of each axle, front first.
    axles: &'static [f32],
    hard_point_y: f32,
    suspension_rest: f32,
    max_suspension_force: f32,
    /// Total engine force over all wheels, newtons.
    engine_force: f32,
    max_speed: f32,
    /// Steering lock of the front axle at standstill, radians. Zero for a tank, which turns by `turn_rate`.
    steer_lock: f32,
    /// Yaw rate at full stick for a vehicle that turns on the spot, rad/s.
    turn_rate: f32,
    lift: Lift,
    linear_damping: f32,
    /// How far below the box's centre the centre of mass sits, metres. A low one keeps a car on its
    /// wheels through hard corners; zero leaves the box's own.
    com_drop: f32,
}

static JEEP: Spec = Spec {
    half_extents: JEEP_HALF_EXTENTS,
    // The box is 6.6 m³, so a jeep weighs about 1.3 tonnes. Big wheels and a soft top speed: it
    // gets there slowly but clears kerbs the sports car would ground on.
    density: 200.0,
    wheel_radius: JEEP_WHEEL_RADIUS,
    track_half: JEEP_TRACK_HALF,
    axles: &[-JEEP_AXLE_Z, JEEP_AXLE_Z],
    hard_point_y: JEEP_HARD_POINT_Y,
    suspension_rest: JEEP_SUSPENSION_REST,
    max_suspension_force: 60_000.0,
    engine_force: 10_000.0,
    max_speed: JEEP_MAX_SPEED,
    steer_lock: JEEP_MAX_STEER,
    turn_rate: 0.0,
    lift: Lift::Wheels,
    linear_damping: 0.05,
    com_drop: 0.15,
};

static SPORTS: Spec = Spec {
    half_extents: SPORTS_HALF_EXTENTS,
    // 5 m³ of flat body is about a tonne, with half as much engine again to push it.
    density: 200.0,
    wheel_radius: SPORTS_WHEEL_RADIUS,
    track_half: SPORTS_TRACK_HALF,
    axles: &[-SPORTS_AXLE_Z, SPORTS_AXLE_Z],
    hard_point_y: SPORTS_HARD_POINT_Y,
    suspension_rest: SPORTS_SUSPENSION_REST,
    max_suspension_force: 60_000.0,
    engine_force: 15_000.0,
    max_speed: SPORTS_MAX_SPEED,
    steer_lock: SPORTS_MAX_STEER,
    turn_rate: 0.0,
    lift: Lift::Wheels,
    linear_damping: 0.05,
    com_drop: 0.02,
};

static TANK: Spec = Spec {
    half_extents: TANK_HALF_EXTENTS,
    // 13.5 m³ of hull is about 3.4 tonnes: three times the car, with a stronger engine to match.
    density: 250.0,
    wheel_radius: TANK_WHEEL_RADIUS,
    track_half: TANK_TRACK_HALF,
    axles: &TANK_AXLES_Z,
    hard_point_y: TANK_HARD_POINT_Y,
    suspension_rest: TANK_SUSPENSION_REST,
    max_suspension_force: 200_000.0,
    engine_force: 22_000.0,
    max_speed: TANK_MAX_SPEED,
    steer_lock: 0.0,
    turn_rate: TANK_TURN_RATE,
    lift: Lift::Wheels,
    linear_damping: 0.05,
    com_drop: 0.0,
};

static HELI: Spec = Spec {
    half_extents: HELI_HALF_EXTENTS,
    // 3.8 m³ of hull is about 1.1 tonnes. Nothing below uses the wheel fields.
    density: 300.0,
    wheel_radius: 0.0,
    track_half: 0.0,
    axles: &[],
    hard_point_y: 0.0,
    suspension_rest: 0.0,
    max_suspension_force: 0.0,
    engine_force: 0.0,
    max_speed: 0.0,
    steer_lock: 0.0,
    turn_rate: 0.0,
    lift: Lift::Rotor,
    // Air drag is what limits the speed: the rotor's lean gives about 4.9 m/s², so about 25 m/s.
    linear_damping: 0.2,
    com_drop: 0.0,
};

static PLANE: Spec = Spec {
    half_extents: PLANE_HALF_EXTENTS,
    // 6.7 m³ of fuselage is about 600 kg, a Dr.I fully loaded. The wings are a weightless second box.
    density: 90.0,
    wheel_radius: PLANE_WHEEL_RADIUS,
    track_half: PLANE_TRACK_HALF,
    axles: &PLANE_AXLES_Z,
    hard_point_y: PLANE_HARD_POINT_Y,
    suspension_rest: PLANE_SUSPENSION_REST,
    max_suspension_force: 30_000.0,
    // The wheels only roll and steer; the propeller is a force on the body.
    engine_force: 0.0,
    max_speed: 0.0,
    steer_lock: PLANE_MAX_STEER,
    turn_rate: 0.0,
    lift: Lift::Wing,
    // Drag is `PLANE_DRAG`, by the square of the speed.
    linear_damping: 0.0,
    com_drop: 0.0,
};

/// The spec for a vehicle kind. Unknown kinds are jeeps.
pub(super) fn spec(kind: u8) -> &'static Spec {
    match kind {
        KIND_TANK => &TANK,
        KIND_HELI => &HELI,
        KIND_PLANE => &PLANE,
        KIND_SPORTS => &SPORTS,
        _ => &JEEP,
    }
}

/// Half extents of the chassis box of a vehicle kind.
pub fn half_extents(kind: u8) -> [f32; 3] {
    spec(kind).half_extents
}

/// How quickly a tank's yaw rate follows the stick, rad/s².
const TURN_ACCEL: f32 = 3.0;
/// Speed change a blast gives a vehicle at its centre, m/s.
const BLAST_VELOCITY: f32 = 5.0;
/// Impulse each wheel may use for braking per tick, per kilogram of chassis.
const BRAKE_PER_KG: f32 = 0.045;
/// Light brake that stands in for rolling resistance when the driver is off the pedals.
const COAST_BRAKE_PER_KG: f32 = 0.0015;
/// How fast the front wheels turn towards the steering input, radians per second.
const STEER_RATE: f32 = 7.0;
/// Wheels straighten this much faster than they turn in.
const STEER_RETURN: f32 = 1.5;
/// Steering lock shrinks with speed: `steer_lock / (1 + speed / STEER_FADE_SPEED)`.
const STEER_FADE_SPEED: f32 = 20.0;
/// The engine pulls with full force up to this fraction of the top speed, then fades to nothing at it.
const FULL_POWER: f32 = 0.65;
/// Side grip of a car's rear wheels while the handbrake is held, against 1 normally. Grip is the share
/// of the wheel's sideways slip cancelled each tick, so even 0.3 holds firm; this lets the tail slide out.
const DRIFT_GRIP: f32 = 0.05;
/// Downforce on a car with a wheel down, per kilogram, per (m/s)²: a third of its weight again at 40 m/s.
const DOWNFORCE_PER_KG: f32 = 0.0001;
/// How quickly the throttle pitches and the steering rolls a car with no wheel down, rad/s².
const AIR_PITCH: f32 = 3.0;
const AIR_ROLL: f32 = 3.0;
/// Side grip of the rear wheels and the tyres' friction, as `WheelTuning` sets them.
const SIDE_GRIP: f32 = 1.0;
const FRICTION_SLIP: f32 = 12.0;
/// How far a wrecked wheel points off straight, radians: a left wheel turns left, a right one right.
const TOE_MAX: f32 = 0.12;
/// Share of its side grip and friction a wrecked wheel loses.
const WHEEL_GRIP_LOSS: f32 = 0.85;
const WHEEL_SLIP_LOSS: f32 = 0.6;
/// A wheel that has come off: a stub that never reaches the ground.
const LOST_WHEEL_RADIUS: f32 = 0.05;

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
}

/// How worn a vehicle is, which both ends' physics apply: the engine's power and the state of a
/// car's four wheels (front left, front right, rear left, rear right; 0 sound, 255 come off).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarCondition {
    pub power: f32,
    pub wheels: [u8; 4],
}

impl CarCondition {
    pub const NEW: CarCondition = CarCondition { power: 1.0, wheels: [0; 4] };

    /// From a vehicle's health and damage as the snapshot carries them. The engine keeps full power
    /// down to half health and loses up to half of it below. Mirrored by `conditionOf` in `vehicle.ts`.
    pub fn of(kind: u8, hp: u8, damage: [u8; 8]) -> CarCondition {
        let health = f32::from(hp) / f32::from(vehicle_hp(kind));
        let mut wheels = [0; 4];
        wheels.copy_from_slice(&damage[DAMAGE_WHEELS..DAMAGE_WHEELS + 4]);
        CarCondition { power: (0.5 + health).min(1.0), wheels }
    }
}

impl CarControls {
    /// Nobody asks for anything.
    pub const IDLE: CarControls = CarControls { throttle: 0.0, steer: 0.0, handbrake: false, lift: 0.0, powered: false, aim: None };
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
    /// Where they touched, in the vehicle's own frame (+X right, +Y up, -Z forward).
    pub local: [f32; 3],
    /// Where they touched, in the world.
    pub point: [f32; 3],
    /// Speed the contact took out of the vehicle this step, m/s: its impulse over the vehicle's mass.
    pub dv: f32,
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
    /// A tank's turret or a plane's wings, a second solid on the same body. It has no mass of its own.
    extra: Option<ColliderHandle>,
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
}

impl Car {
    /// Whether `collider` is one of this vehicle's solids.
    fn owns(&self, collider: ColliderHandle) -> bool {
        self.collider == collider || self.extra == Some(collider)
    }
}

fn pose_of(position: [f32; 3], yaw: f32) -> Pose {
    Pose::from_parts(Vector::from(position), Rotation::from_rotation_y(yaw))
}

/// How far below its feet a standing character probes for a car to ride. The character controller
/// rests 2 cm above whatever it stands on, so this reaches into the roof.
const RIDE_PROBE: f32 = 0.06;

/// Share of full engine force at `speed` for a top speed of `max`: all of it up to [`FULL_POWER`]
/// of the way, then less and less until none at `max`.
fn pull(speed: f32, max: f32) -> f32 {
    ((max - speed.abs()) / (max * (1.0 - FULL_POWER))).clamp(0.0, 1.0)
}

/// A plane's wheels for one tick: no engine, the front pair steered towards where the pilot looks
/// (and by A/D), and braked while the stick is pulled back or nobody is aboard.
fn roll_plane(car: &mut Car, controls: CarControls, body: &RigidBody, speed: f32, mass: f32, dt: f32) {
    let _ = body;
    let wanted = if controls.powered { controls.steer.clamp(-1.0, 1.0) } else { 0.0 };
    let lock = car.spec.steer_lock / (1.0 + speed.abs() / STEER_FADE_SPEED);
    let target = wanted * lock;
    let returning = target.abs() < car.steer.abs() || target * car.steer < 0.0;
    let step = STEER_RATE * if returning { STEER_RETURN } else { 1.0 } * dt;
    car.steer += (target - car.steer).clamp(-step, step);
    let braking = !controls.powered || controls.lift < -0.5;
    let brake = if braking { 0.5 * BRAKE_PER_KG * mass } else { COAST_BRAKE_PER_KG * mass };
    let steer = car.steer;
    for (index, wheel) in car.controller.wheels_mut().iter_mut().enumerate() {
        wheel.steering = if index < 2 { -steer } else { 0.0 };
        wheel.engine_force = 0.0;
        wheel.brake = brake;
    }
}

/// Keeps a car planted and lets it be steered in the air: downforce while any wheel is down, and a
/// nudge of pitch (throttle) and roll (steering) while none is.
fn assist(body: &mut RigidBody, (throttle, steer): (f32, f32), grounded: bool, dt: f32) {
    let rotation = *body.rotation();
    if grounded {
        let push = DOWNFORCE_PER_KG * body.mass() * body.linvel().length_squared() * dt;
        body.apply_impulse(rotation * Vector::new(0.0, -push, 0.0), true);
    } else {
        // + about X lifts the nose and + about Z lifts the right side, so forward and right are negative.
        let nudge = rotation * Vector::new(-throttle * AIR_PITCH * dt, 0.0, -steer * AIR_ROLL * dt);
        body.set_angvel(body.angvel() + nudge, true);
    }
}

/// One tick of rotor flight for a powered helicopter body.
///
/// The disc leans by setting the angular velocity that carries the hull's up axis towards the
/// wanted lean (forward with the stick, sideways with the turn), and the yaw rate is set straight
/// from the stick, like a tank's. The rotor then pushes along the up axis hard enough to hover
/// however it leans, and holds a vertical speed set by the climb input. Everything is a function
/// of the body's state and the controls alone, so the client can replay it.
fn fly(body: &mut RigidBody, controls: CarControls, dt: f32) {
    let rotation = *body.rotation();
    let up = rotation * Vector::Y;
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let flat = Vector::new(nose.x, 0.0, nose.z);
    let heading = if flat.length() > 1e-3 { flat.normalize() } else { Vector::new(0.0, 0.0, -1.0) };
    let right = heading.cross(Vector::Y);

    let throttle = controls.throttle.clamp(-1.0, 1.0);
    let steer = controls.steer.clamp(-1.0, 1.0);
    let lean = (Vector::Y + heading * (throttle * HELI_TILT) + right * (steer * HELI_BANK)).normalize();
    let mut wanted = up.cross(lean) * HELI_LEVEL_GAIN;
    // Clockwise seen from above is a negative rotation about +Y.
    wanted.y = -steer * HELI_YAW_RATE;
    let current = body.angvel();
    body.set_angvel(current + (wanted - current) * (HELI_RESPONSE * dt).min(1.0), true);

    // The climb input asks for a vertical speed; with none held it asks for zero, so letting go of
    // the stick holds altitude. Climbing fades out below the ceiling and reverses above it.
    let lift = controls.lift.clamp(-1.0, 1.0);
    let headroom = ((HELI_CEILING - body.translation().y) / 20.0).clamp(-1.0, 1.0);
    let mut target = if lift > 0.0 { lift * HELI_CLIMB_SPEED * headroom.max(0.0) } else { lift * HELI_SINK_SPEED };
    if headroom < 0.0 {
        target = target.min(headroom * HELI_CLIMB_SPEED);
    }
    let extra = ((target - body.linvel().y) * HELI_VERT_GAIN).clamp(-HELI_VERT_ACCEL, HELI_VERT_ACCEL);
    let accel = (CAR_GRAVITY + extra) / up.y.max(0.5);
    // A force rather than an impulse: it is integrated together with gravity, so hovering is exact.
    body.add_force(up * (accel * body.mass()), true);
}

/// The share of a plane's thrust its throttle asks for. With the stick released it cruises in the
/// air but idles on the ground, so a pilot who climbs in does not roll off at once.
fn throttle_setting(throttle: f32, grounded: bool) -> f32 {
    let throttle = throttle.clamp(-1.0, 1.0);
    if throttle > 0.0 {
        PLANE_CRUISE_THROTTLE + throttle * (1.0 - PLANE_CRUISE_THROTTLE)
    } else if throttle < 0.0 {
        PLANE_CRUISE_THROTTLE + throttle * (PLANE_CRUISE_THROTTLE - PLANE_IDLE_THROTTLE)
    } else if grounded {
        PLANE_IDLE_THROTTLE
    } else {
        PLANE_CRUISE_THROTTLE
    }
}

/// The unit direction a pilot looking at `[yaw, pitch]` wants to fly, turned back towards the middle
/// near the edge of a world `bound` metres from its centre to each side.
fn aim_direction(aim: [f32; 2], position: Vector, bound: f32) -> Vector {
    let [yaw, pitch] = aim;
    let direction = Vector::new(-yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
    // How far into the margin it is along each axis, 0 to 1, towards the side it is on.
    let into = |coordinate: f32| ((coordinate.abs() - (bound - PLANE_EDGE_MARGIN)) / PLANE_EDGE_MARGIN).clamp(0.0, 1.0) * coordinate.signum();
    let (x, z) = (into(position.x), into(position.z));
    let near = x.abs().max(z.abs());
    if near == 0.0 {
        return direction;
    }
    // Inwards and level, blended in the deeper it goes.
    let inward = Vector::new(-x, 0.0, -z).normalize();
    let blended = direction * (1.0 - near) + inward * near;
    if blended.length() > 1e-3 { blended.normalize() } else { inward }
}

/// How deep into the edge margin `position` is, 0 (outside it) to 1 (at the wall).
fn edge_depth(position: Vector, bound: f32) -> f32 {
    let into = |coordinate: f32| ((coordinate.abs() - (bound - PLANE_EDGE_MARGIN)) / PLANE_EDGE_MARGIN).clamp(0.0, 1.0);
    into(position.x).max(into(position.z))
}

/// What a plane flies itself to do to head for `target`, a world direction: `[pitch, yaw, roll]`, each
/// -1..=1 as a share of the full rate. It banks into the turn and pulls once banked, takes the turn
/// level when the target is behind, and levels its wings when the target is nearly ahead. The pilot
/// uses it near the edge of the world and an empty plane to glide.
fn autopilot(rotation: Rotation, target: Vector, airspeed: f32) -> [f32; 3] {
    let up = rotation * Vector::Y;
    let right = rotation * Vector::X;
    let local = rotation.inverse() * target;
    // How far off the nose the target is, and which way round it lies seen from the cockpit:
    // `across` right and `above` up, a unit pair.
    let off = (-local.z).clamp(-1.0, 1.0).acos();
    let lateral = local.x.hypot(local.y);
    let (mut across, mut above) = if lateral > 1e-3 { (local.x / lateral, local.y / lateral) } else { (1.0, 0.0) };
    let bank = (-right.y).atan2(up.y);
    // Anywhere behind, a level turn to whichever side it is on, rather than a loop over the top or a
    // dive: the target is taken to be level off that wing, which the bank brings overhead.
    if off > std::f32::consts::FRAC_PI_2 {
        // Nearly dead astern, keep turning the way it already leans, so it does not dither.
        let lean = if across.abs() < 0.5 && bank.abs() > 0.2 { bank } else { across };
        let side = if lean < 0.0 { -1.0 } else { 1.0 };
        (across, above) = (side * bank.cos(), bank.sin().abs());
    }
    let mut pitch = (above * off * PLANE_AIM_GAIN).clamp(-1.0, 1.0);
    if pitch > 0.0 {
        // A wide turn waits for the bank before it pulls, and nobody pulls hard near the stall.
        let banked = if off > 1.2 { (bank.abs() / PLANE_MAX_BANK).clamp(0.25, 1.0) } else { 1.0 };
        let margin = ((airspeed - PLANE_STALL_SPEED) / (0.4 * PLANE_STALL_SPEED)).clamp(0.2, 1.0);
        pitch *= banked * margin;
    }
    let yaw = (across * off * PLANE_AIM_GAIN).clamp(-1.0, 1.0);
    let level = (-bank * 2.0).clamp(-1.0, 1.0);
    let into = (across * off * 3.0).clamp(-1.0, 1.0);
    let blend = (off / PLANE_BANK_ANGLE).min(1.0);
    let roll = (level + (into - level) * blend).clamp((-PLANE_MAX_BANK - bank) * 3.0, (PLANE_MAX_BANK - bank) * 3.0);
    [pitch, yaw, roll.clamp(-1.0, 1.0)]
}

/// One tick of a plane's flight, applied before the rigid-body step on top of its wheels.
///
/// The propeller pushes along the nose. Above the stall speed the wings hold the plane up, turn its
/// flight path towards where the nose points (lift grows with the angle between them) and the fin
/// kills sideslip; below it the lift fades away and the nose drops towards where the plane is going.
/// Drag grows with the square of the speed.
///
/// The mouse sets a heading: the plane turns towards `aim` as the autopilot would, banking into the turn
/// and levelling when it arrives; W/S and A/D take pitch and roll over by hand while they are held.
/// The controls are a stick and a throttle. Throttle: `lift` is Space (+1, full) and Shift (-1, cut),
/// with cruise when neither is held. Stick: `throttle` is the forward axis, W (+1) pushes the nose down and
/// S pulls it up; with the stick centred the nose settles back to the horizon. A/D (`steer`) roll the
/// wings, which level again when released, and a banked plane turns of its own accord with a little
/// rudder from the stick. Authority is lost at low speed. Near the edge of the world the plane turns itself
/// back, and with nobody aboard it glides. Everything is a function of the body's state and the controls
/// alone, so the client can replay it.
fn wing(body: &mut RigidBody, controls: CarControls, power: f32, grounded: bool, bound: f32, dt: f32) {
    let rotation = *body.rotation();
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let up = rotation * Vector::Y;
    let right = rotation * Vector::X;
    let velocity = body.linvel();
    let position = body.translation();
    let airspeed = velocity.dot(nose).max(0.0);
    let speed = velocity.length();

    // How much the wings fly, 0 (stalled) to 1.
    let flying = ((airspeed - 0.7 * PLANE_STALL_SPEED) / (0.3 * PLANE_STALL_SPEED)).clamp(0.0, 1.0);
    // Level flight needs no angle of attack above `PLANE_LEVEL_SPEED`; slower, the wings carry less and
    // the plane sinks unless the nose comes up.
    // Close to the ground they carry it all (ground effect), so it lifts off at the stall speed.
    let carried = if grounded { 1.0 } else { (airspeed / PLANE_LEVEL_SPEED).powi(2).min(1.0) };
    let lift = (flying * (CAR_GRAVITY * carried - PLANE_AOA_GAIN * velocity.dot(up))).clamp(-PLANE_MAX_LIFT, PLANE_MAX_LIFT);
    let side = -PLANE_SIDE_GAIN * velocity.dot(right) * (airspeed / PLANE_STALL_SPEED).min(1.0);
    let headroom = ((PLANE_CEILING - position.y) / 20.0).clamp(0.0, 1.0);
    let thrust = if controls.powered { throttle_setting(controls.lift, grounded) * PLANE_THRUST * power * headroom } else { 0.0 };
    let accel = nose * thrust + up * lift + right * side - velocity * (speed * PLANE_DRAG);
    // A force rather than an impulse, integrated together with gravity.
    body.add_force(accel * body.mass(), true);

    // A stalled plane noses over towards where it is going.
    let stalled = if grounded || speed < 2.0 { Vector::ZERO } else { nose.cross(velocity / speed) * (PLANE_STALL_DROP * (1.0 - flying)) };
    let current = body.angvel();
    let wanted = if controls.powered || !grounded {
        let heading = (-nose.x).atan2(-nose.z);
        let near = edge_depth(position, bound);
        // What the plane flies itself to do: glide on level (a little nose down with nobody aboard) and,
        // near the edge, turn back to the middle.
        let hold = aim_direction([heading, if controls.powered { 0.0 } else { PLANE_GLIDE_PITCH }], position, bound);
        let auto = autopilot(rotation, hold, airspeed);
        let [pitch, yaw, roll] = if controls.powered {
            let bank = (-right.y).atan2(up.y);
            let steer = controls.steer.clamp(-1.0, 1.0);
            let pull = (-controls.throttle).clamp(-1.0, 1.0);
            // With the mouse the plane flies itself towards where it points, as the autopilot does; the
            // keys then take an axis over while they are held, and with neither it flies level.
            let mouse = controls.aim.map(|[yaw, pitch]| {
                let aim = aim_direction([yaw, pitch.clamp(-PLANE_AIM_MAX_PITCH, PLANE_AIM_MAX_PITCH)], position, bound);
                autopilot(rotation, aim, airspeed)
            });
            let level_pitch = (-nose.y.clamp(-1.0, 1.0).asin() * PLANE_LEVEL_GAIN).clamp(-1.0, 1.0);
            let pitch = if pull.abs() > 0.05 { pull } else { mouse.map_or(level_pitch, |m| m[0]) };
            let roll = if steer.abs() > 0.05 { steer } else { mouse.map_or((-bank * 2.0).clamp(-1.0, 1.0), |m| m[2]) };
            let roll = roll.clamp((-PLANE_MAX_BANK - bank) * 3.0, (PLANE_MAX_BANK - bank) * 3.0).clamp(-1.0, 1.0);
            let yaw = match mouse {
                Some(m) if steer.abs() <= 0.05 => m[1],
                _ => (steer * 0.3 + bank.sin() * 0.8).clamp(-1.0, 1.0),
            };
            let own = [pitch, yaw, roll];
            // The autopilot takes over completely by a third of the way in.
            let taken = (near * 3.0).min(1.0);
            std::array::from_fn(|i| own[i] * (1.0 - taken) + auto[i] * taken)
        } else {
            auto
        };
        let authority = (airspeed / PLANE_CRUISE_SPEED).powi(2).min(1.3);
        // + about X lifts the nose, + about Y turns left and + about Z lifts the right wing.
        let turn = Vector::new(pitch * PLANE_PITCH_RATE, -yaw * PLANE_YAW_RATE, -roll * PLANE_ROLL_RATE) * authority;
        rotation * turn + stalled
    } else {
        current
    };
    // On the ground the wheels steer, so the controls only take over as the plane lifts off.
    let response = (PLANE_RESPONSE * dt).min(1.0) * if grounded { flying } else { 1.0 };
    body.set_angvel(current + (wanted - current) * response, true);
}

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

    /// Heading of the forward axis in the stickman convention: 0 faces -Z, + turns left (towards -X).
    pub fn yaw(&self) -> f32 {
        let [x, y, z, w] = self.rotation;
        (2.0 * (x * z + w * y)).atan2(1.0 - 2.0 * (x * x + y * y))
    }
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
        if kind != KIND_PLANE {
            return true;
        }
        let [wx, wy, wz] = PLANE_WING_HALF_EXTENTS;
        let wings = SharedShape::cuboid(wx + 0.1, wy + 0.1, wz + 0.1);
        let at = pose_of(position, yaw) * Vector::from(PLANE_WING_CENTRE);
        !self.touches_static(wings.as_ref(), Pose::from_parts(at, Rotation::from_rotation_y(yaw)))
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
            .build();
        let body = self.bodies.insert(body);
        let mut chassis = ColliderBuilder::cuboid(hx, hy, hz).friction(0.4);
        chassis = if spec.com_drop > 0.0 {
            // The box's own mass and inertia, with the centre of mass lowered.
            let mass = spec.density * 8.0 * hx * hy * hz;
            let inertia = Vector::new(hy * hy + hz * hz, hx * hx + hz * hz, hx * hx + hy * hy) * (mass / 3.0);
            chassis.mass_properties(MassProperties::new(Vector::new(0.0, -spec.com_drop, 0.0), mass, inertia))
        } else {
            chassis.density(spec.density)
        };
        let collider = self.colliders.insert_with_parent(chassis.build(), body, &mut self.bodies);
        // The turret is a cylinder, so it needs no turning with the gun. It adds no mass, which keeps
        // the hull's balance and the tuning that goes with it. A plane's wings are a weightless box
        // the same way.
        let extra = match kind {
            KIND_TANK => Some(ColliderBuilder::cylinder(TANK_TURRET_HALF_HEIGHT, TANK_TURRET_RADIUS).translation(Vector::from(TANK_TURRET_PIVOT))),
            KIND_PLANE => {
                let [x, y, z] = PLANE_WING_HALF_EXTENTS;
                Some(ColliderBuilder::cuboid(x, y, z).translation(Vector::from(PLANE_WING_CENTRE)))
            }
            _ => None,
        }
        .map(|collider| self.colliders.insert_with_parent(collider.density(0.0).friction(0.4).build(), body, &mut self.bodies));
        // A helicopter has no wheels, so its controller is never updated.
        let mut controller = DynamicRayCastVehicleController::new(body);
        // Our forward is -Z, but the controller only uses this axis to sign its own speed estimate,
        // which we do not read.
        controller.index_forward_axis = 2;
        let tuning = WheelTuning {
            suspension_stiffness: 25.0,
            suspension_compression: 2.2,
            suspension_damping: 3.0,
            max_suspension_travel: 0.3,
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
        self.cars.insert(id, Car { spec, body, collider, extra, controller, steer: 0.0, turn: 0.0, spin: 0.0, flight: CarControls::IDLE, air: (0.0, 0.0), condition: CarCondition::NEW });
    }

    /// Puts a car back upright and at rest.
    pub fn reset_car(&mut self, id: u32, position: [f32; 3], yaw: f32) {
        let Some(car) = self.cars.get_mut(&id) else { return };
        car.steer = 0.0;
        car.turn = 0.0;
        car.spin = 0.0;
        if let Some(body) = self.bodies.get_mut(car.body) {
            body.set_position(pose_of(position, yaw), true);
            body.set_linvel(Vector::ZERO, true);
            body.set_angvel(Vector::ZERO, true);
        }
    }

    /// Test helper: rolls a car about its forward axis and moves it.
    #[cfg(test)]
    pub fn set_car_roll(&mut self, id: u32, roll: f32, position: [f32; 3]) {
        let Some(car) = self.cars.get(&id) else { return };
        if let Some(body) = self.bodies.get_mut(car.body) {
            body.set_position(Pose::from_parts(Vector::from(position), Rotation::from_rotation_z(roll)), true);
            body.set_linvel(Vector::ZERO, true);
            body.set_angvel(Vector::ZERO, true);
        }
    }

    /// Test helper: points vehicle `id` `pitch` radians up from level at `yaw`, at `position`, moving
    /// along its nose at `speed`.
    #[cfg(test)]
    pub fn set_car_dive(&mut self, id: u32, position: [f32; 3], yaw: f32, pitch: f32, speed: f32) {
        let Some(car) = self.cars.get(&id) else { return };
        if let Some(body) = self.bodies.get_mut(car.body) {
            let rotation = Rotation::from_rotation_y(yaw) * Rotation::from_rotation_x(pitch);
            body.set_position(Pose::from_parts(Vector::from(position), rotation), true);
            body.set_linvel(rotation * Vector::new(0.0, 0.0, -speed), true);
            body.set_angvel(Vector::ZERO, true);
        }
    }

    /// How worn vehicle `id` is from now on; applied by [`Physics::drive_car`].
    pub fn set_car_condition(&mut self, id: u32, condition: CarCondition) {
        if let Some(car) = self.cars.get_mut(&id) {
            car.condition = condition;
        }
    }

    /// Applies the driver's controls to the wheels for the next [`Physics::step_cars`].
    pub fn drive_car(&mut self, id: u32, controls: CarControls, dt: f32) {
        let Some(car) = self.cars.get_mut(&id) else { return };
        if car.spec.lift == Lift::Rotor {
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
        let lock = car.spec.steer_lock / (1.0 + speed.abs() / STEER_FADE_SPEED);
        let target = controls.steer.clamp(-1.0, 1.0) * lock;
        let returning = target.abs() < car.steer.abs() || target * car.steer < 0.0;
        let step = STEER_RATE * if returning { STEER_RETURN } else { 1.0 } * dt;
        car.steer += (target - car.steer).clamp(-step, step);
        car.turn = controls.steer.clamp(-1.0, 1.0) * car.spec.turn_rate;
        car.air = (controls.throttle.clamp(-1.0, 1.0), controls.steer.clamp(-1.0, 1.0));

        // Pushing against the direction of travel brakes instead of reversing the engine.
        let against = controls.throttle * speed < -1.0;
        let (engine, brake) = if controls.throttle == 0.0 {
            (0.0, COAST_BRAKE_PER_KG * mass)
        } else if against {
            (0.0, BRAKE_PER_KG * mass)
        } else {
            let headroom = if controls.throttle > 0.0 { pull(speed, car.spec.max_speed) } else { 0.5 };
            (controls.throttle * car.spec.engine_force / car.controller.wheels().len() as f32 * headroom * car.condition.power, 0.0)
        };
        let handbrake = if controls.handbrake { BRAKE_PER_KG * mass } else { 0.0 };

        // Only a car's first axle steers and is spared the handbrake, which also lets its tail slide.
        let front_wheels = if car.spec.steer_lock > 0.0 { 2 } else { 0 };
        let drift = controls.handbrake && front_wheels > 0;
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
            wheel.engine_force = if gone { 0.0 } else { engine };
            wheel.brake = if gone { 0.0 } else if front { brake } else { brake.max(handbrake) };
            wheel.side_friction_stiffness = if drift && !front { DRIFT_GRIP } else { SIDE_GRIP } * (1.0 - WHEEL_GRIP_LOSS * wear);
            wheel.friction_slip = FRICTION_SLIP * (1.0 - WHEEL_SLIP_LOSS * wear);
            wheel.radius = if gone { LOST_WHEEL_RADIUS } else { spec.wheel_radius };
            wheel.suspension_rest_length = if gone { 0.0 } else { spec.suspension_rest };
        }
    }

    /// Takes destroyed buildings and trees out of the collider set. With no car the pipeline does
    /// not run otherwise, and the broad-phase would keep pointing at colliders that are gone.
    fn apply_removals(&mut self) {
        if self.removals.is_empty() {
            return;
        }
        for object in std::mem::take(&mut self.removals) {
            if let Some(handle) = self.handles.remove(&object) {
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
        if self.cars.is_empty() {
            return;
        }
        for car in self.cars.values().filter(|car| car.spec.lift == Lift::Rotor) {
            if let Some(body) = self.bodies.get_mut(car.body) {
                // Forces persist from step to step until reset.
                body.reset_forces(false);
                if car.flight.powered {
                    fly(body, car.flight, dt);
                }
            }
        }
        for car in self.cars.values_mut().filter(|car| car.spec.lift != Lift::Rotor) {
            // Wheel rays must not hit their own chassis.
            let filter = QueryFilter::default().exclude_rigid_body(car.body);
            let queries = self.broad_phase.as_query_pipeline_mut(
                self.narrow_phase.query_dispatcher(),
                &mut self.bodies,
                &mut self.colliders,
                filter,
            );
            car.controller.update_vehicle(dt, queries);
        }
        // A car is pressed down or tilted in the air. A tank turns on the spot, but only while some
        // track is on the ground. The wheels' side grip fights any spin every tick, so the rate is
        // kept here and written back each time.
        for car in self.cars.values_mut().filter(|car| car.spec.lift != Lift::Rotor) {
            let grounded = car.controller.wheels().iter().any(|wheel| wheel.raycast_info().is_in_contact);
            let Some(body) = self.bodies.get_mut(car.body) else { continue };
            if car.spec.lift == Lift::Wing {
                body.reset_forces(false);
                wing(body, car.flight, car.condition.power, grounded, self.bound, dt);
                continue;
            }
            if car.spec.steer_lock > 0.0 {
                assist(body, car.air, grounded, dt);
            }
            if car.turn == 0.0 || !grounded {
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
    }

    /// Every `(car, tree)` pair in contact after the last step: what a car has driven into.
    pub fn car_tree_contacts(&self) -> Vec<(u32, Obj)> {
        let mut found = Vec::new();
        for (&id, car) in &self.cars {
            for handle in [Some(car.collider), car.extra].into_iter().flatten() {
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
            for handle in [Some(car.collider), car.extra].into_iter().flatten() {
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
                    let other_car = self.cars.iter().find(|(_, car)| car.owns(other)).map(|(&other, _)| other);
                    found.push(CarImpact {
                        car: id,
                        other_car,
                        object: self.objects.get(&other).copied(),
                        local: local.into(),
                        point: (body.position() * local).into(),
                        dv: pair.total_impulse_magnitude() / body.mass(),
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

    /// The car, if any, that a character centred at `position` is standing on.
    pub fn car_under(&self, position: [f32; 3]) -> Option<CarHit> {
        self.car_touching([position[0], position[1] - RIDE_PROBE, position[2]])
    }

    fn car_touching(&self, position: [f32; 3]) -> Option<CarHit> {
        let queries = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            QueryFilter::only_dynamic(),
        );
        let (hit, _) = queries.intersect_shape(Pose::from_translation(Vector::from(position)), self.character_shape.shape()).next()?;
        let (&car, body) = self.cars.iter().find(|(_, car)| car.owns(hit)).and_then(|(id, car)| Some((id, self.bodies.get(car.body)?)))?;
        Some(CarHit { car, position: body.translation().into(), velocity: body.linvel().into() })
    }

    /// How far a ray from `origin` along the unit vector `dir` travels before it hits the world or a
    /// vehicle other than `ignore`, up to `max`. `None` when it reaches `max` clear.
    pub fn cast_ray(&self, origin: [f32; 3], dir: [f32; 3], max: f32, ignore: u32) -> Option<f32> {
        self.cast_ray_hit(origin, dir, max, ignore).map(|(distance, _)| distance)
    }

    /// Like [`Self::cast_ray`], and also says which vehicle it hit, if it was a vehicle and not the world.
    pub fn cast_ray_hit(&self, origin: [f32; 3], dir: [f32; 3], max: f32, ignore: u32) -> Option<(f32, Option<u32>)> {
        let mut filter = QueryFilter::default();
        if let Some(car) = self.cars.get(&ignore) {
            filter = filter.exclude_rigid_body(car.body);
        }
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, filter);
        let ray = Ray::new(Vector::from(origin), Vector::from(dir));
        let (collider, distance) = queries.cast_ray(&ray, max, true)?;
        let car = self.cars.iter().find(|(_, car)| car.owns(collider)).map(|(&id, _)| id);
        Some((distance, car))
    }

    /// How far a ray travels before it hits the world, and which building or tree that was, if it
    /// was one. Vehicles are not ignored.
    pub fn cast_ray_object(&self, origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<(f32, Option<super::Obj>)> {
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, QueryFilter::default());
        let ray = Ray::new(Vector::from(origin), Vector::from(dir));
        let (collider, distance) = queries.cast_ray(&ray, max, true)?;
        Some((distance, self.object_of(collider)))
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
            body.apply_impulse(direction * falloff * BLAST_VELOCITY * body.mass(), true);
            reached.push((id, falloff));
        }
        reached
    }

    pub fn car_motion(&self, id: u32) -> Option<CarMotion> {
        let car = self.cars.get(&id)?;
        let body = self.bodies.get(car.body)?;
        Some(CarMotion { linvel: body.linvel().into(), angvel: body.angvel().into(), steer: car.steer, spin: car.spin })
    }

    pub fn car_pose(&self, id: u32) -> Option<CarPose> {
        let car = self.cars.get(&id)?;
        let body = self.bodies.get(car.body)?;
        let rotation = *body.rotation();
        let forward = rotation * Vector::new(0.0, 0.0, -1.0);
        Some(CarPose {
            position: body.translation().into(),
            rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
            forward_speed: forward.dot(body.linvel()),
            steer: if car.spec.steer_lock > 0.0 { car.steer / car.spec.steer_lock } else { 0.0 },
            up_y: (rotation * Vector::new(0.0, 1.0, 0.0)).y,
            velocity: body.linvel().into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::consts::TICK_DT;
    use shared::protocol::{KIND_HELI, KIND_JEEP, KIND_PLANE, KIND_SPORTS, KIND_TANK};
    use shared::world::{CollisionMesh, WORLD_VERSION, WorldFile};

    fn flat() -> WorldFile {
        WorldFile { version: WORLD_VERSION, half_extent: 20_000, ..Default::default() }
    }

    /// A 4 m tall box from x = -5..5 at z = -60..-58, straight ahead of a car at the origin.
    fn world_with_wall() -> WorldFile {
        let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
        let vertices = vec![
            v(-5.0, 0.0, -60.0), v(5.0, 0.0, -60.0), v(5.0, 0.0, -58.0), v(-5.0, 0.0, -58.0),
            v(-5.0, 4.0, -60.0), v(5.0, 4.0, -60.0), v(5.0, 4.0, -58.0), v(-5.0, 4.0, -58.0),
        ];
        let triangles = vec![
            [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
            [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
        ];
        WorldFile { collision: CollisionMesh { vertices, triangles }, ..flat() }
    }

    fn with_car(world: &WorldFile) -> Physics {
        let mut physics = Physics::new(world).unwrap();
        physics.add_car(1, KIND_JEEP, [0.0, 1.0, 0.0], 0.0);
        physics
    }

    fn run(physics: &mut Physics, controls: CarControls, ticks: u32) -> CarPose {
        for _ in 0..ticks {
            physics.drive_car(1, controls, TICK_DT);
            physics.step_cars(TICK_DT);
        }
        physics.car_pose(1).unwrap()
    }

    /// Heading of the car's forward axis: 0 faces -Z, positive turns left.
    fn yaw(pose: &CarPose) -> f32 {
        pose.yaw()
    }

    const FORWARD: CarControls = CarControls { throttle: 1.0, ..CarControls::IDLE };
    const COAST: CarControls = CarControls::IDLE;

    /// A scripted drive of the cars and a tank on flat ground, for the fixture in `vehicle.spec.ts`.
    fn scripted_controls(tick: u32) -> CarControls {
        let phase = tick / 60;
        match phase {
            0 => COAST,
            1 | 2 => FORWARD,
            3 | 4 => CarControls { throttle: 1.0, steer: 0.6, ..CarControls::IDLE },
            5 => CarControls { throttle: 1.0, steer: -1.0, ..CarControls::IDLE },
            6 => CarControls { throttle: -1.0, ..CarControls::IDLE },
            _ => CarControls { throttle: 0.5, handbrake: true, ..CarControls::IDLE },
        }
    }

    /// `(kind, tick, position)` after `scripted_controls`, mirrored in `vehicle.spec.ts`.
    const SCRIPTED_CHECKPOINTS: [(u8, u32, [f32; 3]); 12] = [
        (KIND_JEEP, 90, [0.0, 0.8572, -0.9723]),
        (KIND_JEEP, 210, [2.5239, 0.8564, -22.4112]),
        (KIND_JEEP, 300, [30.9153, 0.8560, -23.3722]),
        (KIND_JEEP, 480, [64.3589, 0.8568, -59.5452]),
        (KIND_SPORTS, 90, [-0.0019, 0.3975, -1.9154]),
        (KIND_SPORTS, 210, [4.3494, 0.3961, -44.2191]),
        (KIND_SPORTS, 300, [56.9594, 0.3953, -54.7882]),
        (KIND_SPORTS, 480, [125.6157, 0.3959, -128.6126]),
        (KIND_TANK, 90, [0.0, 0.9400, -0.8340]),
        (KIND_TANK, 210, [0.5739, 0.9400, -17.5632]),
        (KIND_TANK, 300, [11.9063, 0.9400, -29.5214]),
        (KIND_TANK, 480, [26.5903, 0.9400, -40.3629]),
    ];

    #[test]
    fn scripted_drive_matches_fixture() {
        for kind in [KIND_JEEP, KIND_SPORTS, KIND_TANK] {
            let mut physics = Physics::new(&flat()).unwrap();
            physics.add_car(1, kind, [0.0, 1.5, 0.0], 0.0);
            for tick in 0..480 {
                physics.drive_car(1, scripted_controls(tick), TICK_DT);
                physics.step_cars(TICK_DT);
                let done = tick + 1;
                if let Some((_, _, expected)) = SCRIPTED_CHECKPOINTS.iter().find(|(k, t, _)| *k == kind && *t == done) {
                    let position = physics.car_pose(1).unwrap().position;
                    let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
                    assert!(close, "kind {kind} tick {done}: {position:?}");
                }
            }
        }
    }

    /// A 12° slope rising towards -Z (the way `FORWARD` drives): 5 m cells over 200 m. `vehicle.spec.ts`
    /// builds the same heights.
    fn ramp() -> WorldFile {
        const SLOPE: f32 = 0.2125566;
        let side = 41u32;
        let heights = (0..side)
            .flat_map(|j| (0..side).map(move |_| j))
            .map(|j| ((100.0 - (j as f32 * 5.0 - 100.0)) * SLOPE * 100.0).round() as i16)
            .collect();
        WorldFile { ground: shared::terrain::Ground { origin: -10_000, cell: 500, side, heights }, ..flat() }
    }

    /// `(tick, position)` of a car driven `FORWARD` up `ramp` from [0, _, 40], mirrored in `vehicle.spec.ts`.
    const RAMP_CHECKPOINTS: [(u32, [f32; 3]); 3] = [
        (60, [0.0000, 13.8145, 39.1671]),
        (180, [0.0000, 17.2683, 22.8746]),
        (360, [0.0000, 29.7911, -36.0072]),
    ];

    #[test]
    fn a_car_climbs_a_slope_like_wasm() {
        let world = ramp();
        let mut physics = Physics::new(&world).unwrap();
        let ground = world.ground.height_at(0.0, 40.0);
        physics.add_car(1, KIND_JEEP, [0.0, ground + 1.5, 40.0], 0.0);
        for tick in 0..360 {
            physics.drive_car(1, FORWARD, TICK_DT);
            physics.step_cars(TICK_DT);
            let done = tick + 1;
            if let Some((_, expected)) = RAMP_CHECKPOINTS.iter().find(|(t, _)| *t == done) {
                let position = physics.car_pose(1).unwrap().position;
                let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
                assert!(close, "tick {done}: {position:?}");
            }
        }
        let pose = physics.car_pose(1).unwrap();
        let ground = world.ground.height_at(pose.position[0], pose.position[2]);
        assert!(pose.position[2] < 20.0, "it climbs: {pose:?}");
        assert!(pose.position[1] - ground < 1.2 && pose.position[1] - ground > 0.3, "rides on the slope: {pose:?} on {ground}");
        assert!(pose.up_y > 0.9);
    }

    #[test]
    fn ride_heights_differ_between_cars() {
        let mut heights = vec![];
        for (kind, ride) in [(KIND_JEEP, JEEP_RIDE_HEIGHT), (KIND_SPORTS, SPORTS_RIDE_HEIGHT)] {
            let mut physics = Physics::new(&flat()).unwrap();
            physics.add_car(1, kind, [0.0, 1.5, 0.0], 0.0);
            let pose = run(&mut physics, COAST, 240);
            assert!((pose.position[1] - ride).abs() < 0.01, "kind {kind} settles at {ride}: {pose:?}");
            heights.push(pose.position[1] - half_extents(kind)[1]);
        }
        // Ground clearance under the chassis: the jeep clears at least four times what the sports car does.
        assert!(heights[0] > 0.4 && heights[1] < 0.15 && heights[0] > 3.0 * heights[1], "{heights:?}");
    }

    #[test]
    fn the_sports_car_is_quicker_than_the_jeep() {
        let mut reached = vec![];
        for kind in [KIND_JEEP, KIND_SPORTS] {
            let mut physics = Physics::new(&flat()).unwrap();
            physics.add_car(1, kind, [0.0, 1.5, 0.0], 0.0);
            run(&mut physics, COAST, 60);
            let early = run(&mut physics, FORWARD, 60 * 3);
            let late = run(&mut physics, FORWARD, 60 * 12);
            assert!(late.up_y > 0.98 && late.position[0].abs() < 1.0, "kind {kind} drives straight: {late:?}");
            reached.push((early.forward_speed, late.forward_speed));
        }
        let [(jeep_early, jeep_top), (sports_early, sports_top)] = reached[..] else { unreachable!() };
        assert!(sports_early > jeep_early * 1.2, "quicker off the line: {reached:?}");
        assert!(sports_top > jeep_top * 1.3, "higher top speed: {reached:?}");
        assert!(sports_top <= SPORTS_MAX_SPEED + 1.0 && jeep_top <= JEEP_MAX_SPEED + 1.0, "{reached:?}");
    }

    #[test]
    fn settles_on_its_suspension() {
        let mut physics = with_car(&flat());
        let pose = run(&mut physics, COAST, 180);
        assert!(pose.up_y > 0.99, "{pose:?}");
        assert!(pose.position[1] > 0.5 && pose.position[1] < 1.0, "ride height {pose:?}");
        assert!(pose.forward_speed.abs() < 0.1);
    }

    #[test]
    fn accelerates_forward_to_a_sensible_speed() {
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 60);
        let pose = run(&mut physics, FORWARD, 60 * 6);
        assert!(pose.position[2] < -20.0, "should drive towards -Z: {pose:?}");
        assert!(pose.position[0].abs() < 1.0, "no drift sideways: {pose:?}");
        assert!(pose.forward_speed > 8.0 && pose.forward_speed <= JEEP_MAX_SPEED + 1.0, "{pose:?}");
        assert!(pose.up_y > 0.98, "{pose:?}");
    }

    #[test]
    fn reverses() {
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 60);
        let pose = run(&mut physics, CarControls { throttle: -1.0, ..COAST }, 60 * 3);
        assert!(pose.position[2] > 3.0 && pose.forward_speed < -1.0, "{pose:?}");
    }

    #[test]
    fn steering_right_turns_the_heading_clockwise() {
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 60);
        // Short of a half turn, so the yaw doesn't wrap.
        let pose = run(&mut physics, CarControls { throttle: 1.0, steer: 1.0, ..CarControls::IDLE }, 60 * 2);
        assert!(yaw(&pose) < -0.3, "right turn lowers the yaw: {}", yaw(&pose));
        assert!(pose.position[0] > 1.0, "ends up to the right (+X): {pose:?}");
        assert!(pose.steer > 0.0);
    }

    #[test]
    fn steering_left_mirrors_it() {
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 60);
        let pose = run(&mut physics, CarControls { throttle: 1.0, steer: -1.0, ..CarControls::IDLE }, 60 * 2);
        assert!(yaw(&pose) > 0.3 && pose.position[0] < -1.0, "{pose:?}");
    }

    /// Angle between where the car points and where it is going, radians.
    fn slip_angle(pose: &CarPose) -> f32 {
        let heading = pose.yaw();
        let [vx, _, vz] = pose.velocity;
        let travel = (-vx).atan2(-vz);
        let mut angle = (travel - heading).abs();
        if angle > std::f32::consts::PI {
            angle = 2.0 * std::f32::consts::PI - angle;
        }
        angle
    }

    #[test]
    fn the_handbrake_swings_the_tail_out() {
        let mut slips = vec![];
        for handbrake in [false, true] {
            let mut physics = with_car(&flat());
            run(&mut physics, COAST, 60);
            run(&mut physics, FORWARD, 60 * 2);
            let pose = run(&mut physics, CarControls { throttle: 1.0, steer: 1.0, handbrake, ..CarControls::IDLE }, 40);
            assert!(pose.up_y > 0.9, "stays on its wheels: {pose:?}");
            slips.push(slip_angle(&pose));
        }
        assert!(slips[1] > slips[0] + 0.15 && slips[1] > 0.3, "slides with the handbrake: {slips:?}");
    }

    #[test]
    fn a_hard_corner_at_speed_rolls_it_but_a_firm_one_does_not() {
        for kind in [KIND_JEEP, KIND_SPORTS] {
            let mut worst = [1.0_f32; 2];
            for (i, steer) in [0.35_f32, 1.0].into_iter().enumerate() {
                let mut physics = Physics::new(&flat()).unwrap();
                // Back from the middle, so the run-up stays inside the 200 m world.
                physics.add_car(1, kind, [0.0, 1.5, 190.0], 0.0);
                run(&mut physics, COAST, 60);
                let fast = run(&mut physics, FORWARD, 60 * 6);
                assert!(fast.forward_speed > 20.0, "kind {kind}: {fast:?}");
                for _ in 0..60 * 3 {
                    let pose = run(&mut physics, CarControls { throttle: 1.0, steer, ..CarControls::IDLE }, 1);
                    worst[i] = worst[i].min(pose.up_y);
                }
            }
            assert!(worst[0] > 0.8, "kind {kind} rolls in a firm corner: {worst:?}");
            assert!(worst[1] < 0.3, "kind {kind} stays up at full lock: {worst:?}");
        }
    }

    #[test]
    fn a_wrecked_front_wheel_pulls_the_car_to_its_side() {
        for (wheels, side) in [([220, 0, 0, 0], -1.0), ([0, 220, 0, 0], 1.0)] {
            let mut physics = with_car(&flat());
            physics.set_car_condition(1, CarCondition { power: 1.0, wheels });
            run(&mut physics, COAST, 60);
            let pose = run(&mut physics, FORWARD, 60 * 3);
            assert!(pose.position[0] * side > 2.0, "{wheels:?} pulls towards {side}: {pose:?}");
            assert!(pose.up_y > 0.95, "{pose:?}");
        }
        let mut sound = with_car(&flat());
        run(&mut sound, COAST, 60);
        assert!(run(&mut sound, FORWARD, 60 * 3).position[0].abs() < 0.3, "a sound car runs straight");
    }

    #[test]
    fn a_lost_wheel_drops_its_corner_and_drags() {
        let mut sound = with_car(&flat());
        run(&mut sound, COAST, 60);
        let sound = run(&mut sound, FORWARD, 60 * 3);
        let mut physics = with_car(&flat());
        physics.set_car_condition(1, CarCondition { power: 1.0, wheels: [0, 0, 0, WHEEL_GONE] });
        let rest = run(&mut physics, COAST, 90);
        // The right rear corner is down: rolled right (the right side lower) and nose up.
        let rotation = Rotation::from_xyzw(rest.rotation[0], rest.rotation[1], rest.rotation[2], rest.rotation[3]);
        let right = rotation * Vector::X;
        let nose = rotation * Vector::new(0.0, 0.0, -1.0);
        assert!(right.y < -0.05 && nose.y > 0.02, "tilted onto the lost wheel: right {right:?} nose {nose:?}");
        let pose = run(&mut physics, FORWARD, 60 * 3);
        assert!(pose.forward_speed < sound.forward_speed * 0.8, "drags: {} against {}", pose.forward_speed, sound.forward_speed);
    }

    #[test]
    fn a_battered_engine_is_slower() {
        let mut speeds = vec![];
        for health in [100, 10] {
            let mut physics = with_car(&flat());
            physics.set_car_condition(1, CarCondition::of(KIND_JEEP, health, [0; 8]));
            run(&mut physics, COAST, 60);
            speeds.push(run(&mut physics, FORWARD, 60 * 2).forward_speed);
        }
        assert!(speeds[1] < speeds[0] * 0.75, "{speeds:?}");
        assert_eq!(CarCondition::of(KIND_JEEP, 60, [0; 8]).power, 1.0, "full power down to half health");
    }

    /// `(tick, position)` of a jeep with a bent front left wheel and no rear right one, driven by
    /// `scripted_controls`, mirrored in `vehicle.spec.ts`.
    const DAMAGED_CHECKPOINTS: [(u32, [f32; 3]); 3] = [(90, [0.1147, 0.7626, -0.3659]), (300, [9.1750, 0.7638, -15.2648]), (480, [13.0374, 0.7641, -30.9333])];
    const DAMAGED_CONDITION: CarCondition = CarCondition { power: 0.8, wheels: [180, 0, 0, WHEEL_GONE] };

    #[test]
    fn a_damaged_drive_matches_fixture() {
        let mut physics = Physics::new(&flat()).unwrap();
        physics.add_car(1, KIND_JEEP, [0.0, 1.5, 0.0], 0.0);
        physics.set_car_condition(1, DAMAGED_CONDITION);
        for tick in 0..480 {
            physics.drive_car(1, scripted_controls(tick), TICK_DT);
            physics.step_cars(TICK_DT);
            let done = tick + 1;
            if let Some((_, expected)) = DAMAGED_CHECKPOINTS.iter().find(|(t, _)| *t == done) {
                let position = physics.car_pose(1).unwrap().position;
                let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
                assert!(close, "tick {done}: {position:?}");
            }
        }
    }

    #[test]
    fn brakes_to_a_stop_and_coasts_down() {
        let mut physics = with_car(&flat());
        run(&mut physics, FORWARD, 60 * 4);
        let braked = run(&mut physics, CarControls { throttle: -1.0, ..COAST }, 60 * 3);
        assert!(braked.forward_speed.abs() < 1.0 || braked.forward_speed < 0.0, "{braked:?}");
        let mut physics = with_car(&flat());
        let moving = run(&mut physics, FORWARD, 60 * 4);
        let coasted = run(&mut physics, COAST, 60 * 10);
        assert!(coasted.forward_speed < moving.forward_speed * 0.5, "{moving:?} -> {coasted:?}");
    }

    #[test]
    fn a_wall_stops_the_car() {
        let mut physics = with_car(&world_with_wall());
        let pose = run(&mut physics, FORWARD, 60 * 12);
        assert!(pose.position[2] > -58.0 - 0.1, "stopped before the wall: {pose:?}");
        assert!(pose.position[2] < -50.0, "reached the wall: {pose:?}");
    }

    #[test]
    fn spawn_checks_see_walls() {
        let physics = Physics::new(&world_with_wall()).unwrap();
        assert!(physics.car_fits(KIND_JEEP, [0.0, 1.0, 0.0], 0.0));
        assert!(!physics.car_fits(KIND_JEEP, [0.0, 1.0, -58.5], 0.0));
    }

    #[test]
    fn cars_are_solid_to_characters() {
        use crate::physics::{Character, STANDING_Y};
        use shared::protocol::InputFrame;
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 60);
        let mut character = Character { position: [-4.0, STANDING_Y, 0.0], grounded: true, ..Default::default() };
        // Yaw -π/2 faces +X, into the car's flank.
        let toward_car = InputFrame(1, 0, 127, -1571, 0, 0, 0, 0);
        for _ in 0..180 {
            physics.move_character(&mut character, &toward_car, TICK_DT);
        }
        let x = character.position[0];
        assert!(x < -JEEP_HALF_EXTENTS[0] - CAPSULE_RADIUS + 0.1 && x > -3.0, "stopped at the car's side: {x}");
        assert!(physics.car_hit(character.position).is_none(), "resting against a car is not a hit");
    }

    #[test]
    fn a_moving_car_overlaps_a_character_in_its_path() {
        use crate::physics::STANDING_Y;
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 60);
        let in_path = [0.0, STANDING_Y, -12.0];
        let mut hit = None;
        for _ in 0..180 {
            run(&mut physics, FORWARD, 1);
            hit = hit.or(physics.car_hit(in_path));
        }
        let hit = hit.expect("the car drives into the spot");
        assert_eq!(hit.car, 1);
        assert!(hit.velocity[2] < -3.0, "{hit:?}");
    }

    fn with_tank(world: &WorldFile) -> Physics {
        let mut physics = Physics::new(world).unwrap();
        physics.add_car(1, KIND_TANK, [0.0, 1.2, 0.0], 0.0);
        physics
    }

    #[test]
    fn a_tank_settles_and_drives_slower_than_a_car() {
        let mut physics = with_tank(&flat());
        let rest = run(&mut physics, COAST, 180);
        assert!(rest.up_y > 0.99 && rest.position[1] > 0.6 && rest.position[1] < 1.3, "{rest:?}");
        let pose = run(&mut physics, FORWARD, 60 * 8);
        assert!(pose.position[2] < -20.0 && pose.position[0].abs() < 1.0, "{pose:?}");
        assert!(pose.forward_speed > 5.0 && pose.forward_speed <= TANK_MAX_SPEED + 0.5, "{pose:?}");
    }

    #[test]
    fn a_tank_turns_on_the_spot() {
        let mut physics = with_tank(&flat());
        run(&mut physics, COAST, 120);
        let start = physics.car_pose(1).unwrap();
        let right = run(&mut physics, CarControls { steer: 1.0, ..COAST }, 120);
        assert!(yaw(&right) < -0.8, "two seconds of right stick turns clockwise: {}", yaw(&right));
        let moved = (right.position[0] - start.position[0]).hypot(right.position[2] - start.position[2]);
        assert!(moved < 1.5, "pivots instead of driving a circle: {moved} m");
        let back = run(&mut physics, CarControls { steer: -1.0, ..COAST }, 120);
        assert!(yaw(&back) > yaw(&right) + 0.8, "left stick turns back: {} -> {}", yaw(&right), yaw(&back));
        // Let go and it stops turning.
        let held = run(&mut physics, COAST, 90);
        assert!((yaw(&run(&mut physics, COAST, 30)) - yaw(&held)).abs() < 0.05);
    }

    #[test]
    fn a_tank_can_turn_and_drive_together() {
        let mut physics = with_tank(&flat());
        run(&mut physics, COAST, 60);
        let pose = run(&mut physics, CarControls { throttle: 1.0, steer: 1.0, ..CarControls::IDLE }, 60 * 2);
        assert!(yaw(&pose) < -1.0 && pose.position[0] > 2.0 && pose.up_y > 0.95, "{pose:?}");
    }

    fn with_heli(height: f32) -> Physics {
        let mut physics = Physics::new(&flat()).unwrap();
        physics.add_car(1, KIND_HELI, [0.0, height, 0.0], 0.0);
        physics
    }

    const PILOTED: CarControls = CarControls { powered: true, ..CarControls::IDLE };

    /// A scripted flight for the fixture in `vehicle.spec.ts`: hover, climb, lean forward, turn, sink.
    fn scripted_flight(tick: u32) -> CarControls {
        match tick / 60 {
            0 => PILOTED,
            1 | 2 => CarControls { lift: 1.0, ..PILOTED },
            3 | 4 => CarControls { throttle: 1.0, ..PILOTED },
            5 | 6 => CarControls { throttle: 1.0, steer: 0.7, ..PILOTED },
            _ => CarControls { throttle: -0.5, steer: -1.0, lift: -1.0, ..PILOTED },
        }
    }

    /// `(tick, position)` of a helicopter started at `[0, 3, 0]`, mirrored in `vehicle.spec.ts`.
    const FLIGHT_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
        (90, [0.0, 3.6099, 0.0]),
        (210, [0.0, 11.8079, -0.0842]),
        (330, [0.1181, 13.1328, -8.3432]),
        (480, [12.1913, 11.3589, -26.3039]),
    ];

    #[test]
    fn scripted_flight_matches_fixture() {
        let mut physics = with_heli(3.0);
        for tick in 0..480 {
            physics.drive_car(1, scripted_flight(tick), TICK_DT);
            physics.step_cars(TICK_DT);
            let done = tick + 1;
            if let Some((_, expected)) = FLIGHT_CHECKPOINTS.iter().find(|(t, _)| *t == done) {
                let position = physics.car_pose(1).unwrap().position;
                let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
                assert!(close, "tick {done}: {position:?}");
            }
        }
    }

    #[test]
    fn a_powered_helicopter_hovers() {
        let mut physics = with_heli(10.0);
        let pose = run(&mut physics, PILOTED, 60 * 5);
        assert!((pose.position[1] - 10.0).abs() < 0.2 && pose.up_y > 0.999, "{pose:?}");
        assert!(pose.velocity.iter().all(|v| v.abs() < 0.1), "{pose:?}");
    }

    #[test]
    fn an_unpowered_helicopter_falls_and_rests() {
        let mut physics = with_heli(10.0);
        let pose = run(&mut physics, CarControls::IDLE, 60 * 4);
        assert!(pose.position[1] < 1.2 && pose.velocity[1].abs() < 0.5, "{pose:?}");
    }

    #[test]
    fn it_climbs_and_sinks_on_the_lift_input() {
        let mut physics = with_heli(10.0);
        let up = run(&mut physics, CarControls { lift: 1.0, ..PILOTED }, 60 * 2);
        assert!(up.position[1] > 14.0 && up.velocity[1] > 2.0, "{up:?}");
        // It still has the speed of the climb to shed first.
        let down = run(&mut physics, CarControls { lift: -1.0, ..PILOTED }, 60 * 6);
        assert!(down.position[1] < up.position[1] - 3.0 && down.velocity[1] < -3.0, "{up:?} -> {down:?}");
    }

    #[test]
    fn leaning_forward_flies_forward_at_a_sensible_speed_without_losing_height() {
        let mut physics = with_heli(20.0);
        let pose = run(&mut physics, CarControls { throttle: 1.0, ..PILOTED }, 60 * 8);
        assert!(pose.position[2] < -60.0 && pose.position[0].abs() < 1.0, "flies towards -Z: {pose:?}");
        assert!(pose.forward_speed > 12.0 && pose.forward_speed < 30.0, "{pose:?}");
        assert!((pose.position[1] - 20.0).abs() < 2.0, "holds altitude while leaning: {pose:?}");
        assert!(pose.up_y > 0.85, "{pose:?}");
        let back = run(&mut physics, CarControls { throttle: -1.0, ..PILOTED }, 60 * 3);
        assert!(back.forward_speed < pose.forward_speed - 5.0, "{pose:?} -> {back:?}");
    }

    #[test]
    fn the_stick_turns_it_like_a_tank_clockwise_on_the_right() {
        let mut physics = with_heli(10.0);
        let right = run(&mut physics, CarControls { steer: 1.0, ..PILOTED }, 60 * 2);
        assert!(yaw(&right) < -1.0, "{}", yaw(&right));
        let left = run(&mut physics, CarControls { steer: -1.0, ..PILOTED }, 60 * 4);
        assert!(yaw(&left) > yaw(&right) + 1.0, "{} -> {}", yaw(&right), yaw(&left));
    }

    #[test]
    fn it_levels_out_after_a_knock() {
        let mut physics = with_heli(15.0);
        run(&mut physics, PILOTED, 30);
        physics.set_car_roll(1, 0.8, [0.0, 15.0, 0.0]);
        let pose = run(&mut physics, PILOTED, 60 * 3);
        assert!(pose.up_y > 0.99, "{pose:?}");
    }

    #[test]
    fn the_ceiling_stops_the_climb() {
        let mut physics = with_heli(HELI_CEILING - 30.0);
        let pose = run(&mut physics, CarControls { lift: 1.0, ..PILOTED }, 60 * 20);
        assert!(pose.position[1] > HELI_CEILING - 10.0 && pose.position[1] < HELI_CEILING + 8.0, "{pose:?}");
    }

    /// Flat ground six kilometres across, so a plane has room.
    fn airfield() -> WorldFile {
        WorldFile { version: WORLD_VERSION, half_extent: 300_000, ..Default::default() }
    }

    fn with_plane() -> Physics {
        let mut physics = Physics::new(&airfield()).unwrap();
        physics.add_car(1, KIND_PLANE, [0.0, 1.5, 600.0], 0.0);
        physics
    }

    /// A pilot with the throttle at `lift` (Space +1, Shift -1), the stick pulled `pull` (S is +1, nose
    /// up; W is -1) and rolled `roll` (D is +1, right wing down).
    fn pilot(lift: f32, pull: f32, roll: f32) -> CarControls {
        CarControls { lift, throttle: -pull, steer: roll, powered: true, ..CarControls::IDLE }
    }

    /// Takes off on full throttle with a little back stick, climbs to about `height` metres, and lets go.
    fn airborne(physics: &mut Physics, height: f32) -> CarPose {
        let mut pose = run(physics, pilot(1.0, 0.3, 0.0), 60);
        while pose.position[1] < height {
            pose = run(physics, pilot(1.0, 0.3, 0.0), 30);
        }
        run(physics, pilot(0.0, 0.0, 0.0), 60 * 6)
    }

    /// Pitch of the nose, radians: + is up.
    fn nose_pitch(pose: &CarPose) -> f32 {
        let [x, y, z, w] = pose.rotation;
        let nose = Rotation::from_xyzw(x, y, z, w) * Vector::new(0.0, 0.0, -1.0);
        nose.y.clamp(-1.0, 1.0).asin()
    }

    #[test]
    fn a_parked_plane_sits_on_its_wheels_and_idles_when_a_pilot_climbs_in() {
        let mut physics = with_plane();
        let rest = run(&mut physics, CarControls { handbrake: true, ..CarControls::IDLE }, 120);
        assert!(rest.up_y > 0.99 && rest.position[1] > 0.9 && rest.position[1] < 1.4, "{rest:?}");
        assert!(rest.velocity.iter().all(|v| v.abs() < 0.1), "{rest:?}");
        // With the stick released it idles on the ground, creeping at most.
        let idle = run(&mut physics, pilot(0.0, 0.0, 0.0), 120);
        assert!(idle.forward_speed < 3.0 && (600.0 - idle.position[2]) < 4.0, "{idle:?}");
    }

    #[test]
    fn full_throttle_lifts_it_off_within_the_runway() {
        let mut physics = with_plane();
        let rest = run(&mut physics, CarControls { handbrake: true, ..CarControls::IDLE }, 60);
        let mut ticks = 0;
        let pose = loop {
            let pose = run(&mut physics, pilot(1.0, 0.25, 0.0), 1);
            ticks += 1;
            if pose.position[1] > rest.position[1] + 1.0 || ticks > 600 {
                break pose;
            }
        };
        let run_length = rest.position[2] - pose.position[2];
        assert!(run_length < PLANE_RUNWAY, "off the ground after {run_length} m: {pose:?}");
        assert!(pose.forward_speed > PLANE_STALL_SPEED * 0.9 && pose.position[0].abs() < 0.5, "{pose:?}");
    }

    #[test]
    fn it_cruises_level_and_tops_out_on_full_throttle() {
        let mut physics = with_plane();
        airborne(&mut physics, 40.0);
        let before = run(&mut physics, pilot(0.0, 0.0, 0.0), 60 * 10);
        let cruise = run(&mut physics, pilot(0.0, 0.0, 0.0), 60 * 3);
        assert!((cruise.forward_speed - PLANE_CRUISE_SPEED).abs() < 3.0, "{cruise:?}");
        assert!((cruise.position[1] - before.position[1]).abs() < 1.0 && cruise.up_y > 0.99, "level: {before:?} -> {cruise:?}");
        assert!(cruise.position[0].abs() < 1.0, "straight: {cruise:?}");
        let flat_out = run(&mut physics, pilot(1.0, 0.0, 0.0), 60 * 20);
        assert!(flat_out.forward_speed > 42.0 && flat_out.forward_speed < 46.0, "{flat_out:?}");
    }

    #[test]
    fn pulled_up_with_the_throttle_cut_it_stalls_drops_its_nose_and_recovers() {
        let mut physics = with_plane();
        airborne(&mut physics, 80.0);
        let (mut slowest, mut dropped) = (f32::MAX, false);
        for _ in 0..60 * 8 {
            let pose = run(&mut physics, pilot(-1.0, 1.0, 0.0), 1);
            slowest = slowest.min(pose.forward_speed);
            // The stick is still back, but the nose falls through the horizon.
            dropped |= slowest < PLANE_STALL_SPEED && nose_pitch(&pose) < -0.2;
        }
        assert!(slowest < PLANE_STALL_SPEED * 0.7, "it stalls: {slowest}");
        assert!(dropped, "and the nose drops");
        let recovered = run(&mut physics, pilot(1.0, 0.0, 0.0), 60 * 8);
        assert!(recovered.forward_speed > 25.0 && recovered.up_y > 0.9 && recovered.position[1] > 10.0, "{recovered:?}");
    }

    #[test]
    fn s_pulls_the_nose_up_and_w_pushes_it_down_and_letting_go_levels_it() {
        let mut physics = with_plane();
        let level = airborne(&mut physics, 60.0);
        let up = run(&mut physics, pilot(0.0, 1.0, 0.0), 45);
        assert!(nose_pitch(&up) > nose_pitch(&level) + 0.3, "{} -> {}", nose_pitch(&level), nose_pitch(&up));
        // Let go and the nose settles back to the horizon.
        let settled = run(&mut physics, pilot(1.0, 0.0, 0.0), 60 * 6);
        assert!(nose_pitch(&settled).abs() < 0.1, "{}", nose_pitch(&settled));
        let down = run(&mut physics, pilot(0.0, -1.0, 0.0), 45);
        assert!(nose_pitch(&down) < nose_pitch(&settled) - 0.3, "{} -> {}", nose_pitch(&settled), nose_pitch(&down));
    }

    #[test]
    fn space_and_shift_are_the_throttle() {
        let mut speeds = vec![];
        for lift in [-1.0, 0.0, 1.0] {
            let mut physics = with_plane();
            airborne(&mut physics, 40.0);
            speeds.push(run(&mut physics, pilot(lift, 0.0, 0.0), 60 * 8).forward_speed);
        }
        assert!(speeds[0] < speeds[1] - 5.0 && speeds[1] < speeds[2] - 3.0, "Shift slows, nothing cruises, Space speeds up: {speeds:?}");
    }

    #[test]
    fn rolling_banks_and_turns_it_and_letting_go_levels_the_wings() {
        for (roll, side) in [(1.0, -1.0), (-1.0, 1.0)] {
            let mut physics = with_plane();
            let start = airborne(&mut physics, 50.0);
            // Holding D rolls right, until the bank stops at its limit, and a banked plane turns.
            let banked = run(&mut physics, pilot(1.0, 0.0, roll), 90);
            assert!(banked.up_y < 0.7 && banked.up_y > 0.1, "banked: {banked:?}");
            let turning = run(&mut physics, pilot(1.0, 0.0, roll), 60 * 3);
            assert!(turning.yaw() * side > 0.5 || (turning.yaw() - start.yaw()) * side > 0.5, "turns to the {side} side: {}", turning.yaw());
            let released = run(&mut physics, pilot(1.0, 0.0, 0.0), 60 * 4);
            assert!(released.up_y > 0.97, "wings level again: {released:?}");
        }
    }

    #[test]
    fn the_mouse_turns_it_towards_where_the_pilot_points_banking_into_the_turn() {
        for turn in [1.2_f32, -1.2] {
            let mut physics = with_plane();
            let start = airborne(&mut physics, 60.0);
            let aim = Some([shared::quant::wrap_angle(start.yaw() + turn), 0.0]);
            let mouse = CarControls { aim, ..pilot(0.0, 0.0, 0.0) };
            let banking = run(&mut physics, mouse, 60);
            assert!(banking.up_y < 0.95, "it banks into the turn: {banking:?}");
            let arrived = run(&mut physics, mouse, 60 * 8);
            let error = shared::quant::wrap_angle(arrived.yaw() - shared::quant::wrap_angle(start.yaw() + turn)).abs();
            assert!(error < 0.2, "it points where the mouse does: {error} {arrived:?}");
            assert!(arrived.up_y > 0.97, "and levels its wings there: {arrived:?}");
        }
    }

    #[test]
    fn the_mouse_climbs_and_dives_it() {
        let mut physics = with_plane();
        let start = airborne(&mut physics, 80.0);
        let up = run(&mut physics, CarControls { aim: Some([start.yaw(), 0.4]), ..pilot(1.0, 0.0, 0.0) }, 60 * 4);
        assert!(up.position[1] > start.position[1] + 10.0, "climbs: {start:?} -> {up:?}");
        let down = run(&mut physics, CarControls { aim: Some([up.yaw(), -0.4]), ..pilot(0.0, 0.0, 0.0) }, 60 * 4);
        assert!(down.position[1] < up.position[1] - 10.0, "dives: {up:?} -> {down:?}");
    }

    #[test]
    fn without_a_pilot_it_comes_down() {
        let mut physics = with_plane();
        let flying = airborne(&mut physics, 60.0);
        let falling = run(&mut physics, CarControls::IDLE, 60 * 6);
        assert!(falling.position[1] < flying.position[1] - 10.0, "{flying:?} -> {falling:?}");
    }

    #[test]
    fn the_ceiling_stops_a_climbing_plane() {
        let mut physics = with_plane();
        run(&mut physics, pilot(1.0, 0.4, 0.0), 60 * 60);
        let pose = run(&mut physics, pilot(1.0, 0.4, 0.0), 60 * 20);
        assert!(pose.position[1] < PLANE_CEILING + 10.0 && pose.position[1] > PLANE_CEILING - 40.0, "{pose:?}");
    }

    #[test]
    fn it_turns_back_before_the_edge_of_the_world() {
        // Facing east, the wall 3 km out. The pilot just holds full throttle.
        let mut physics = Physics::new(&airfield()).unwrap();
        physics.add_car(1, KIND_PLANE, [0.0, 1.5, 600.0], -std::f32::consts::FRAC_PI_2);
        airborne(&mut physics, 40.0);
        let mut furthest: f32 = 0.0;
        for _ in 0..60 * 120 {
            let pose = run(&mut physics, pilot(1.0, 0.0, 0.0), 1);
            furthest = furthest.max(pose.position[0]);
            assert!(pose.position[1] > 5.0, "it does not fly into the ground: {pose:?}");
        }
        assert!(furthest > 2900.0 && furthest < 3000.0 - PLANE_WING_HALF_EXTENTS[0] - 1.0, "{furthest}");
    }

    #[test]
    fn the_wings_are_solid_out_to_their_tips() {
        let mut physics = with_plane();
        let rest = run(&mut physics, CarControls { handbrake: true, ..CarControls::IDLE }, 120);
        let [_, wy, wz] = PLANE_WING_CENTRE;
        let at = [rest.position[0] - 10.0, rest.position[1] + wy, rest.position[2] + wz];
        let hit = physics.cast_ray_hit(at, [1.0, 0.0, 0.0], 20.0, 99).expect("the wing stops it");
        assert!((hit.0 - (10.0 - PLANE_WING_HALF_EXTENTS[0])).abs() < 0.1 && hit.1 == Some(1), "{hit:?}");
    }

    #[test]
    fn the_runway_ends_at_a_wall() {
        let world = world_with_wall();
        let physics = Physics::new(&world).unwrap();
        // The wall is 58 m ahead (-Z) and nothing is behind, all the way to the world's edge.
        let ahead = physics.runway(&world.ground, [0.0, 1.5, 0.0], 0.0, 150.0);
        let behind = physics.runway(&world.ground, [0.0, 1.5, 0.0], std::f32::consts::PI, 150.0);
        assert!((ahead - 58.0).abs() < 0.5, "{ahead}");
        assert_eq!(behind, 150.0);
        let edge = physics.runway(&world.ground, [0.0, 1.5, 0.0], std::f32::consts::PI, 250.0);
        assert!((edge - 200.0).abs() < 1.0, "the boundary wall: {edge}");
        // Past the end of the wall, the wingtip still clips it.
        let beside = physics.runway(&world.ground, [7.0, 1.5, 0.0], 0.0, 150.0);
        assert!((beside - 58.0).abs() < 0.5, "{beside}");
        assert!(physics.runway(&world.ground, [9.5, 1.5, 0.0], 0.0, 150.0) > 99.0);
    }

    /// A scripted flight for the fixture in `vehicle.spec.ts`: parked, a take-off, a climbing turn,
    /// the blip switch in a turn, and a roll on the stick.
    fn scripted_plane(tick: u32) -> CarControls {
        match tick / 60 {
            0 => pilot(0.0, 0.0, 0.0),
            1..7 => pilot(1.0, 0.3, 0.0),
            7..9 => pilot(0.0, 0.0, 0.8),
            9 | 10 => pilot(-1.0, 0.5, 0.8),
            _ => pilot(1.0, 0.1, -0.5),
        }
    }

    /// `(tick, position)` of a plane started at `[0, 1.5, 600]` on the airfield, mirrored in `vehicle.spec.ts`.
    const PLANE_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
        (120, [0.0000, 1.0504, 595.1553]),
        (420, [0.0000, 12.9997, 500.8397]),
        (660, [17.7795, 11.9774, 408.6677]),
        (900, [78.6062, 2.9332, 305.7454]),
    ];

    #[test]
    fn scripted_plane_flight_matches_fixture() {
        let mut physics = with_plane();
        for tick in 0..900 {
            physics.drive_car(1, scripted_plane(tick), TICK_DT);
            physics.step_cars(TICK_DT);
            let done = tick + 1;
            if let Some((_, expected)) = PLANE_CHECKPOINTS.iter().find(|(t, _)| *t == done) {
                let position = physics.car_pose(1).unwrap().position;
                let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
                assert!(close, "tick {done}: {position:?}");
            }
        }
    }

    #[test]
    fn rays_see_walls_and_ignore_their_own_vehicle() {
        let mut physics = with_car(&world_with_wall());
        run(&mut physics, COAST, 60);
        let origin = [0.0, 1.5, 0.0];
        // Starting inside the car's own box, the ray still reaches the wall at z = -58.
        let distance = physics.cast_ray(origin, [0.0, 0.0, -1.0], 300.0, 1).expect("the wall is in range");
        assert!((distance - 58.0).abs() < 0.1, "{distance}");
        assert!(physics.cast_ray(origin, [0.0, 0.0, 1.0], 100.0, 1).is_none(), "nothing behind");
        assert!(physics.cast_ray(origin, [0.0, 0.0, -1.0], 30.0, 1).is_none(), "out of range");
        // Aimed at the car from outside, the ray stops at its side.
        let at_car = physics.cast_ray([-10.0, 0.5, 0.0], [1.0, 0.0, 0.0], 50.0, 99).expect("the car is solid to shells");
        assert!((at_car - (10.0 - JEEP_HALF_EXTENTS[0])).abs() < 0.2, "{at_car}");
    }

    #[test]
    fn a_blast_shoves_a_nearby_vehicle_away() {
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 120);
        physics.blast_cars([-3.0, 0.0, 0.0], EXPLOSION_RADIUS);
        // Measured in the hop it gives: once down, the tyres' side grip soon stops the slide.
        let after = run(&mut physics, COAST, 5);
        assert!(after.velocity[0] > 1.0, "pushed towards +X: {:?}", after.velocity);
        let mut far = with_car(&flat());
        run(&mut far, COAST, 120);
        far.blast_cars([-30.0, 0.0, 0.0], EXPLOSION_RADIUS);
        assert!(run(&mut far, COAST, 20).velocity[0].abs() < 0.2);
    }

    #[test]
    fn a_tanks_turret_is_solid_above_the_hull() {
        let mut physics = with_tank(&flat());
        let rest = run(&mut physics, COAST, 180);
        let [px, py, pz] = TANK_TURRET_PIVOT;
        // Above the hull's top, so only the turret can stop it. The ring sits off the hull's centre.
        let height = rest.position[1] + py;
        let hit = physics.cast_ray_hit([-10.0, height, rest.position[2] + pz], [1.0, 0.0, 0.0], 30.0, 99).expect("the turret is solid");
        assert!((hit.0 - (10.0 + px - TANK_TURRET_RADIUS)).abs() < 0.2, "{hit:?}");
        assert_eq!(hit.1, Some(1));
        // The gun's own tank does not stop its shell.
        assert!(physics.cast_ray([-10.0, height, rest.position[2] + pz], [1.0, 0.0, 0.0], 30.0, 1).is_none());
        // A standing character on the roof of the turret is carried by the tank.
        let on_top = [rest.position[0] + px, rest.position[1] + py + TANK_TURRET_HALF_HEIGHT + crate::physics::STANDING_Y - 0.05, rest.position[2] + pz];
        assert_eq!(physics.car_under(on_top).map(|hit| hit.car), Some(1));
    }
}
