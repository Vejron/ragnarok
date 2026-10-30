//! The Roadster: everything the game knows about it.

use super::*;

/// The Roadster (`KIND_ROADSTER`): Tesla's 2026 two-seater with SpaceX cold-gas thrusters fed from a
/// tank of compressed air. Same chassis and wheels as the sports car, a stronger motor, and three
/// tricks: rear nozzles that push it far past what the tyres could (Shift), a hop straight up off the
/// ground (Space), and lift nozzles that hold it in the air while Space stays held. Thrust drains
/// the tank, which refills while it sits on its wheels.
pub const ROADSTER_SEATS: usize = 2;
pub const ROADSTER_HALF_EXTENTS: [f32; 3] = SPORTS_HALF_EXTENTS;
pub const ROADSTER_WHEEL_RADIUS: f32 = SPORTS_WHEEL_RADIUS;
pub const ROADSTER_TRACK_HALF: f32 = SPORTS_TRACK_HALF;
pub const ROADSTER_AXLE_Z: f32 = SPORTS_AXLE_Z;
pub const ROADSTER_HARD_POINT_Y: f32 = SPORTS_HARD_POINT_Y;
pub const ROADSTER_SUSPENSION_REST: f32 = SPORTS_SUSPENSION_REST;
pub const ROADSTER_RIDE_HEIGHT: f32 = SPORTS_RIDE_HEIGHT;
pub const ROADSTER_MAX_STEER: f32 = 0.4;
pub const ROADSTER_MAX_SPEED: f32 = 62.0;
/// The air tank, as a share of full.
pub const ROADSTER_AIR: f32 = 1.0;
/// The rear nozzles' push along the nose, m/s²: well past the tyres' grip.
pub const ROADSTER_BOOST_ACCEL: f32 = 24.0;
/// Air the rear nozzles use, share of the tank per second: about three seconds of boost.
pub const ROADSTER_BOOST_DRAIN: f32 = 0.33;
/// Upward speed a hop adds, m/s: enough to clear a row of cars.
pub const ROADSTER_HOP_SPEED: f32 = 9.0;
/// Air a hop uses, share of the tank.
pub const ROADSTER_HOP_COST: f32 = 0.15;
/// While hovering, the lift nozzles hold this upward speed, m/s, and use this share of the tank per second.
pub const ROADSTER_HOVER_CLIMB: f32 = 1.0;
pub const ROADSTER_HOVER_DRAIN: f32 = 0.25;
/// How hard the lift nozzles chase that speed, per second, and the most they add to gravity's worth, m/s².
pub const ROADSTER_HOVER_GAIN: f32 = 3.0;
pub const ROADSTER_HOVER_ACCEL: f32 = 6.0;
/// While hovering, the lift nozzles lean the car this far (radians, roughly) with the stick, level
/// it this hard and this fast (per second), and turn it at this yaw rate, rad/s.
pub const ROADSTER_HOVER_TILT: f32 = 0.2;
pub const ROADSTER_HOVER_LEVEL: f32 = 4.0;
pub const ROADSTER_HOVER_RESPONSE: f32 = 6.0;
pub const ROADSTER_HOVER_YAW: f32 = 1.2;
/// The lift nozzles fade out over 20 m below this height.
pub const ROADSTER_CEILING: f32 = 40.0;
/// Refill rate on the ground with no nozzle firing, share of the tank per second.
pub const ROADSTER_AIR_REFILL: f32 = 0.2;
pub const ROADSTER_SEAT_HEADS: [[f32; 3]; 2] = [[-0.33, 0.42, 0.12], [0.33, 0.42, 0.12]];

/// The engine up front and the fuel tank behind the seats; the crew are in the open.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.0, -1.6], [0.45, 0.22, 0.45], 110),
        Module::new(Part::Fuel, [0.0, -0.05, 1.75], [0.4, 0.12, 0.25], 90),
    ],
    canvas: false,
};

pub(super) const ROADSTER: VehicleDef = VehicleDef {
    kind: KIND_ROADSTER,
    name: "Roadster",
    hp: 100,
    seats: ROADSTER_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Open,
    seat_heads: &ROADSTER_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
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
        buttons: Buttons::Stunt,
        aims: false,
        solids: &[],
        tongue: 0.0,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
};
