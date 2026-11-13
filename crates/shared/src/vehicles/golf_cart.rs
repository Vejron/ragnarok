//! The golf cart: everything the game knows about it.

use super::*;

/// The golf cart (`KIND_GOLF_CART`): two seats side by side under a canopy roof, small wheels and an
/// electric motor. Light and slow, it potters between the holes at the speed of a brisk cycle ride,
/// with a bag rack on the back.
pub const GOLF_CART_SEATS: usize = 2;
pub const GOLF_CART_HALF_EXTENTS: [f32; 3] = [0.6, 0.28, 1.2];
pub const GOLF_CART_WHEEL_RADIUS: f32 = 0.23;
pub const GOLF_CART_TRACK_HALF: f32 = 0.5;
pub const GOLF_CART_AXLE_Z: f32 = 0.82;
pub const GOLF_CART_HARD_POINT_Y: f32 = -0.06;
pub const GOLF_CART_SUSPENSION_REST: f32 = 0.2;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const GOLF_CART_RIDE_HEIGHT: f32 = 0.397;
pub const GOLF_CART_MAX_STEER: f32 = 0.55;
/// About 43 km/h, which is quick for a golf cart.
pub const GOLF_CART_MAX_SPEED: f32 = 12.0;
pub const GOLF_CART_SEAT_HEADS: [[f32; 3]; 2] = [[-0.28, 0.82, 0.25], [0.28, 0.82, 0.25]];

/// The motor under the back of the seat and the batteries under its cushion; the crew are in the open.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, -0.1, 0.85], [0.25, 0.15, 0.2], 60),
        Module::new(Part::Fuel, [0.0, -0.1, 0.3], [0.45, 0.15, 0.3], 60),
    ],
    canvas: false,
};

pub(super) const GOLF_CART: VehicleDef = VehicleDef {
    kind: KIND_GOLF_CART,
    name: "Golf cart",
    hp: 70,
    seats: GOLF_CART_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Open,
    seat_heads: &GOLF_CART_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: GOLF_CART_HALF_EXTENTS,
        // 1.6 m³ of box is about 450 kg, most of it batteries, carried low.
        density: 280.0,
        wheel_radius: GOLF_CART_WHEEL_RADIUS,
        track_half: GOLF_CART_TRACK_HALF,
        axles: &[-GOLF_CART_AXLE_Z, GOLF_CART_AXLE_Z],
        hard_point_y: GOLF_CART_HARD_POINT_Y,
        suspension_rest: GOLF_CART_SUSPENSION_REST,
        max_suspension_force: 30_000.0,
        engine_force: 3_000.0,
        max_speed: GOLF_CART_MAX_SPEED,
        steer_lock: GOLF_CART_MAX_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        com_drop: 0.1,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        // Too slow to skim: it sinks and wades along the bed.
        skim: None,
        bow: 0.0,
        springs: Springs::FIRM,
        airframe: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
    board: None,
    exit: None,
    fittings: &[],
};
