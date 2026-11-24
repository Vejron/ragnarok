//! The Ford Sierra: everything the game knows about it.

use super::*;

/// The Ford Sierra (`KIND_SIERRA`): the rounded "jelly mould" five-door hatchback, a family car of
/// middling pace. Five seats behind glass.
pub const SIERRA_SEATS: usize = 5;
pub const SIERRA_HALF_EXTENTS: [f32; 3] = [0.85, 0.32, 2.2];
pub const SIERRA_WHEEL_RADIUS: f32 = 0.3;
pub const SIERRA_TRACK_HALF: f32 = 0.725;
pub const SIERRA_AXLE_Z: f32 = 1.305;
pub const SIERRA_HARD_POINT_Y: f32 = 0.0;
pub const SIERRA_SUSPENSION_REST: f32 = 0.258;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const SIERRA_RIDE_HEIGHT: f32 = 0.465;
pub const SIERRA_MAX_STEER: f32 = 0.55;
/// Flat out, m/s.
pub const SIERRA_MAX_SPEED: f32 = 40.0;
/// The driver on the left, then the passengers, front to back.
pub const SIERRA_SEAT_HEADS: [[f32; 3]; 5] = [[-0.38, 0.71, 0.0], [0.38, 0.71, 0.0], [-0.4, 0.69, 0.88], [0.4, 0.69, 0.88], [0.0, 0.69, 0.88]];

/// The engine under the bonnet and the fuel tank under the boot floor; the crew sit behind glass.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.05, -1.55], [0.4, 0.25, 0.4], 110),
        Module::new(Part::Fuel, [0.0, -0.15, 1.55], [0.4, 0.12, 0.3], 90),
    ],
    canvas: false,
};

pub(super) const SIERRA: VehicleDef = VehicleDef {
    kind: KIND_SIERRA,
    name: "Ford Sierra",
    hp: 100,
    seats: SIERRA_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &SIERRA_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: SIERRA_HALF_EXTENTS,
        // 4.8 m³ of box is about 1.15 tonnes.
        density: 240.0,
        wheel_radius: SIERRA_WHEEL_RADIUS,
        track_half: SIERRA_TRACK_HALF,
        axles: &[-SIERRA_AXLE_Z, SIERRA_AXLE_Z],
        hard_point_y: SIERRA_HARD_POINT_Y,
        suspension_rest: SIERRA_SUSPENSION_REST,
        max_suspension_force: 50_000.0,
        // About 6.5 m/s² away from a standstill.
        engine_force: 7_500.0,
        max_speed: SIERRA_MAX_SPEED,
        steer_lock: SIERRA_MAX_STEER,
        steer_axles: Spec::FRONT_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        // Low enough that it stays on its wheels at full lock flat out (measured in vehicle/tests.rs).
        com_drop: 0.3,
        tank: Some(BOOST_FULL),
        buttons: Buttons::Boost,
        boost: Some(Boost::CAR),
        aims: false,
        solids: &[],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        // The underside rises from each axle out to the bumper.
        sweep: 0.8,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        // Too heavy and too slow to skim: it sinks.
        skim: None,
        swim: None,
        bow: 0.0,
        profile: Spec::BOX,
        springs: Springs::FIRM,
        airframe: None,
        rotor: None,
        gait: None,
        arms: None,
        runners: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
    board: None,
    exit: None,
    fittings: &[],
};
