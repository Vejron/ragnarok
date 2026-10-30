//! The sports car: everything the game knows about it.

use super::*;

/// The sports car (`KIND_SPORTS`): a low two-seater with small wheels and a big engine. Quick in a
/// straight line, with the chassis only about 12 cm off the ground.
pub const SPORTS_SEATS: usize = 2;
pub const SPORTS_HALF_EXTENTS: [f32; 3] = [0.95, 0.3, 2.2];
pub const SPORTS_WHEEL_RADIUS: f32 = 0.32;
pub const SPORTS_TRACK_HALF: f32 = 0.82;
pub const SPORTS_AXLE_Z: f32 = 1.45;
pub const SPORTS_HARD_POINT_Y: f32 = 0.05;
pub const SPORTS_SUSPENSION_REST: f32 = 0.22;
pub const SPORTS_RIDE_HEIGHT: f32 = 0.397;
pub const SPORTS_MAX_STEER: f32 = 0.4;
pub const SPORTS_MAX_SPEED: f32 = 50.0;
pub const SPORTS_SEAT_HEADS: [[f32; 3]; 2] = [[-0.35, 0.4, 0.1], [0.35, 0.4, 0.1]];

/// The engine up front and the fuel tank behind the seats; the crew are in the open.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.0, -1.6], [0.45, 0.22, 0.45], 110),
        Module::new(Part::Fuel, [0.0, -0.05, 1.75], [0.4, 0.12, 0.25], 90),
    ],
    canvas: false,
};

pub(super) const SPORTS: VehicleDef = VehicleDef {
    kind: KIND_SPORTS,
    name: "Sports car",
    hp: 100,
    seats: SPORTS_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Open,
    seat_heads: &SPORTS_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
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
        buttons: Buttons::Drive,
        aims: false,
        solids: &[],
        tongue: 0.0,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
};
