//! The Saab 9000: everything the game knows about it.

use super::*;

/// The Saab 9000 CS (`KIND_SAAB_9000`): a big five-door hatchback with a long glass tailgate, roomy
/// and quick, its turbo pulling hard through the middle gears. Five seats behind glass.
pub const SAAB_9000_SEATS: usize = 5;
pub const SAAB_9000_HALF_EXTENTS: [f32; 3] = [0.88, 0.33, 2.3];
pub const SAAB_9000_WHEEL_RADIUS: f32 = 0.32;
pub const SAAB_9000_TRACK_HALF: f32 = 0.76;
pub const SAAB_9000_AXLE_Z: f32 = 1.335;
pub const SAAB_9000_HARD_POINT_Y: f32 = 0.0;
pub const SAAB_9000_SUSPENSION_REST: f32 = 0.248;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const SAAB_9000_RIDE_HEIGHT: f32 = 0.475;
pub const SAAB_9000_MAX_STEER: f32 = 0.5;
/// Flat out, m/s.
pub const SAAB_9000_MAX_SPEED: f32 = 45.0;
/// The driver on the left, then the passengers, front to back.
pub const SAAB_9000_SEAT_HEADS: [[f32; 3]; 5] = [[-0.38, 0.7, 0.0], [0.38, 0.7, 0.0], [-0.4, 0.68, 0.88], [0.4, 0.68, 0.88], [0.0, 0.68, 0.88]];

/// The engine under the bonnet and the fuel tank under the boot floor; the crew sit behind glass.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.05, -1.6], [0.4, 0.25, 0.4], 110),
        Module::new(Part::Fuel, [0.0, -0.15, 1.6], [0.4, 0.12, 0.3], 90),
    ],
    canvas: false,
};

pub(super) const SAAB_9000: VehicleDef = VehicleDef {
    kind: KIND_SAAB_9000,
    name: "Saab 9000",
    hp: 100,
    seats: SAAB_9000_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &SAAB_9000_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: SAAB_9000_HALF_EXTENTS,
        // 5.3 m³ of box is about 1.35 tonnes.
        density: 253.0,
        wheel_radius: SAAB_9000_WHEEL_RADIUS,
        track_half: SAAB_9000_TRACK_HALF,
        axles: &[-SAAB_9000_AXLE_Z, SAAB_9000_AXLE_Z],
        hard_point_y: SAAB_9000_HARD_POINT_Y,
        suspension_rest: SAAB_9000_SUSPENSION_REST,
        max_suspension_force: 50_000.0,
        // About 8.5 m/s² away from a standstill.
        engine_force: 11_500.0,
        max_speed: SAAB_9000_MAX_SPEED,
        steer_lock: SAAB_9000_MAX_STEER,
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
        sweep: 0.85,
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
