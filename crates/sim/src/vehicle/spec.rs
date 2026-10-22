//! The tuning of each vehicle kind that the physics reads: its chassis, wheels, suspension and
//! what holds it up.

use super::*;

/// What holds a vehicle up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lift {
    /// Its wheels, and nothing else.
    Wheels,
    /// A rotor, with no wheels at all.
    Rotor,
    /// Wheels on the ground and wings in the air, with a propeller to push it.
    Wing,
    /// Two spring legs that are ray casts, and a torque that keeps the hull upright.
    Walker,
    /// Four small rotors that hold it up and turn it towards where its pilot looks.
    Drone,
    /// A loitering drone's motor and wings: always cruising forward, turning towards the look.
    Loiter,
}

impl Lift {
    /// Whether the kind has a wheel controller to update.
    pub(crate) fn wheeled(self) -> bool {
        matches!(self, Lift::Wheels | Lift::Wing)
    }
}

/// What differs between vehicle kinds.
pub(crate) struct Spec {
    pub half_extents: [f32; 3],
    /// Chassis density in kg/m³.
    pub(crate) density: f32,
    pub(crate) wheel_radius: f32,
    pub(crate) track_half: f32,
    /// Z of each axle, front first.
    pub(crate) axles: &'static [f32],
    pub(crate) hard_point_y: f32,
    pub(crate) suspension_rest: f32,
    pub(crate) max_suspension_force: f32,
    /// Total engine force over all wheels, newtons.
    pub(crate) engine_force: f32,
    pub(crate) max_speed: f32,
    /// Steering lock of the front axle at standstill, radians. Zero for a tank, which turns by `turn_rate`.
    pub(crate) steer_lock: f32,
    /// Yaw rate at full stick for a vehicle that turns on the spot, rad/s.
    pub(crate) turn_rate: f32,
    pub(crate) lift: Lift,
    pub(crate) linear_damping: f32,
    /// How far below the box's centre the centre of mass sits, metres. A low one keeps a car on its
    /// wheels through hard corners; zero leaves the box's own.
    pub(crate) com_drop: f32,
    /// A full tank of what its tricks run on: a Roadster's air, or a hen's wings' stamina. `None` for
    /// everything else. A walker with one has wings (see `flap`).
    pub(crate) tank: Option<f32>,
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
    tank: None,
};

/// The field mortar: a towed gun on two wheels. Nothing drives it, so it has no engine; its wheels only
/// roll it, and the brake holds it once it is unhitched.
pub(super) static MORTAR: Spec = Spec {
    half_extents: MORTAR_HALF_EXTENTS,
    // 6 m³ of steel, about 900 kg: a jeep can pull it, a sports car struggles.
    density: 150.0,
    wheel_radius: MORTAR_WHEEL_RADIUS,
    track_half: MORTAR_TRACK_HALF,
    // Two axles, one fore and one aft, so it stands on four wheels and does not teeter on a pair at its middle.
    axles: &[-0.9, 0.9],
    hard_point_y: -0.25,
    suspension_rest: 0.3,
    max_suspension_force: 30_000.0,
    engine_force: 0.0,
    max_speed: 1.0,
    steer_lock: 0.0,
    turn_rate: 0.0,
    lift: Lift::Wheels,
    linear_damping: 0.05,
    com_drop: 0.0,
    tank: None,
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
    tank: None,
};

pub(super) static ROADSTER: Spec = Spec {
    half_extents: ROADSTER_HALF_EXTENTS,
    // The sports car's body with a bigger motor; its thrusters are forces of their own (see `thrust`).
    density: 200.0,
    wheel_radius: ROADSTER_WHEEL_RADIUS,
    track_half: ROADSTER_TRACK_HALF,
    axles: &[-ROADSTER_AXLE_Z, ROADSTER_AXLE_Z],
    hard_point_y: ROADSTER_HARD_POINT_Y,
    suspension_rest: ROADSTER_SUSPENSION_REST,
    max_suspension_force: 60_000.0,
    engine_force: 18_000.0,
    max_speed: ROADSTER_MAX_SPEED,
    steer_lock: ROADSTER_MAX_STEER,
    turn_rate: 0.0,
    lift: Lift::Wheels,
    linear_damping: 0.05,
    com_drop: 0.02,
    tank: Some(ROADSTER_AIR),
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
    tank: None,
};

/// The Strv S: a tank with a fixed gun in a casemate (a weightless solid, see `add_car`), and quicker: more engine for a little less hull,
/// and a faster pivot.
pub(super) static STRV: Spec = Spec {
    half_extents: STRV_HALF_EXTENTS,
    // 12.6 m³ of hull, about 12.6 tonnes. The engine and suspension cap scale with it.
    density: 1000.0,
    wheel_radius: STRV_WHEEL_RADIUS,
    track_half: STRV_TRACK_HALF,
    axles: &STRV_AXLES_Z,
    hard_point_y: STRV_HARD_POINT_Y,
    suspension_rest: STRV_SUSPENSION_REST,
    max_suspension_force: 750_000.0,
    engine_force: 112_000.0,
    max_speed: STRV_MAX_SPEED,
    steer_lock: 0.0,
    turn_rate: STRV_TURN_RATE,
    lift: Lift::Wheels,
    linear_damping: 0.05,
    com_drop: 0.0,
    tank: None,
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
    tank: None,
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
    tank: None,
};

pub(super) static WALKER: Spec = Spec {
    half_extents: WALKER_HALF_EXTENTS,
    // 22.5 m³ of hull at this density is about 5.6 tonnes. Nothing below uses the wheel fields; the legs
    // are the `WALKER_*` constants.
    density: 250.0,
    wheel_radius: 0.0,
    track_half: 0.0,
    axles: &[],
    hard_point_y: 0.0,
    suspension_rest: 0.0,
    max_suspension_force: 0.0,
    engine_force: 0.0,
    max_speed: WALKER_MAX_SPEED,
    steer_lock: 0.0,
    turn_rate: WALKER_TURN_RATE,
    lift: Lift::Walker,
    linear_damping: 0.05,
    com_drop: 0.0,
    tank: None,
};

/// The hen: the walker's legs under a lighter hull, so it gets off the ground more easily, and wings.
pub(super) static HEN: Spec = Spec {
    half_extents: HEN_HALF_EXTENTS,
    // About 4.5 tonnes. Nothing below uses the wheel fields; the legs are the `WALKER_*` constants.
    density: 200.0,
    wheel_radius: 0.0,
    track_half: 0.0,
    axles: &[],
    hard_point_y: 0.0,
    suspension_rest: 0.0,
    max_suspension_force: 0.0,
    engine_force: 0.0,
    max_speed: WALKER_MAX_SPEED,
    steer_lock: 0.0,
    turn_rate: WALKER_TURN_RATE,
    lift: Lift::Walker,
    linear_damping: 0.05,
    com_drop: 0.0,
    tank: Some(HEN_STAMINA),
};

pub(super) static DRONE: Spec = Spec {
    half_extents: DRONE_HALF_EXTENTS,
    // 0.025 m³ of airframe and battery, about 1.5 kg. Nothing below uses the wheel fields.
    density: 60.0,
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
    lift: Lift::Drone,
    linear_damping: DRONE_DAMPING,
    com_drop: 0.0,
    tank: None,
};

pub(super) static LOITER: Spec = Spec {
    half_extents: LOITER_HALF_EXTENTS,
    // A box 2.5 x 0.5 x 3.5 m around the wing, about 200 kg. Nothing below uses the wheel fields.
    density: 45.0,
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
    lift: Lift::Loiter,
    linear_damping: LOITER_DAMPING,
    com_drop: 0.0,
    tank: None,
};

/// The spec for a vehicle kind. Unknown kinds are jeeps.
pub(crate) fn spec(kind: u8) -> &'static Spec {
    match kind {
        KIND_WALKER => &WALKER,
        KIND_HEN => &HEN,
        KIND_TANK => &TANK,
        KIND_STRV => &STRV,
        KIND_HELI => &HELI,
        KIND_PLANE => &PLANE,
        KIND_SPORTS => &SPORTS,
        KIND_ROADSTER => &ROADSTER,
        KIND_MORTAR => &MORTAR,
        KIND_DRONE => &DRONE,
        KIND_LOITER => &LOITER,
        _ => &JEEP,
    }
}

/// Half extents of the chassis box of a vehicle kind.
pub fn half_extents(kind: u8) -> [f32; 3] {
    spec(kind).half_extents
}
