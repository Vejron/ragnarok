//! The tuning of each vehicle kind that the physics reads: its chassis, wheels, suspension and
//! what holds it up.

use super::*;

/// What holds a vehicle up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::physics) enum Lift {
    /// Its wheels, and nothing else.
    Wheels,
    /// A rotor, with no wheels at all.
    Rotor,
    /// Wheels on the ground and wings in the air, with a propeller to push it.
    Wing,
}

/// What differs between vehicle kinds.
pub(in crate::physics) struct Spec {
    pub half_extents: [f32; 3],
    /// Chassis density in kg/m³.
    pub(in crate::physics) density: f32,
    pub(in crate::physics) wheel_radius: f32,
    pub(in crate::physics) track_half: f32,
    /// Z of each axle, front first.
    pub(in crate::physics) axles: &'static [f32],
    pub(in crate::physics) hard_point_y: f32,
    pub(in crate::physics) suspension_rest: f32,
    pub(in crate::physics) max_suspension_force: f32,
    /// Total engine force over all wheels, newtons.
    pub(in crate::physics) engine_force: f32,
    pub(in crate::physics) max_speed: f32,
    /// Steering lock of the front axle at standstill, radians. Zero for a tank, which turns by `turn_rate`.
    pub(in crate::physics) steer_lock: f32,
    /// Yaw rate at full stick for a vehicle that turns on the spot, rad/s.
    pub(in crate::physics) turn_rate: f32,
    pub(in crate::physics) lift: Lift,
    pub(in crate::physics) linear_damping: f32,
    /// How far below the box's centre the centre of mass sits, metres. A low one keeps a car on its
    /// wheels through hard corners; zero leaves the box's own.
    pub(in crate::physics) com_drop: f32,
}

pub(super) static JEEP: Spec = Spec {
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

pub(super) static SPORTS: Spec = Spec {
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

pub(super) static TANK: Spec = Spec {
    half_extents: TANK_HALF_EXTENTS,
    // 13.5 m³ of hull is about 13.5 tonnes: ten times the car, so ramming one throws it aside. The
    // engine and suspension cap scale with it, which keeps the acceleration and the ride as they were.
    density: 1000.0,
    wheel_radius: TANK_WHEEL_RADIUS,
    track_half: TANK_TRACK_HALF,
    axles: &TANK_AXLES_Z,
    hard_point_y: TANK_HARD_POINT_Y,
    suspension_rest: TANK_SUSPENSION_REST,
    max_suspension_force: 800_000.0,
    engine_force: 88_000.0,
    max_speed: TANK_MAX_SPEED,
    steer_lock: 0.0,
    turn_rate: TANK_TURN_RATE,
    lift: Lift::Wheels,
    linear_damping: 0.05,
    com_drop: 0.0,
};

pub(super) static HELI: Spec = Spec {
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

pub(super) static PLANE: Spec = Spec {
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
pub(in crate::physics) fn spec(kind: u8) -> &'static Spec {
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
