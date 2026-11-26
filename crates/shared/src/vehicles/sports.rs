//! The sports car, an Opel Manta: everything the game knows about it.

use super::*;

/// The sports car (`KIND_SPORTS`): an Opel Manta GT/E, a low rear-drive coupé from the early eighties
/// with a long bonnet and a short roof. The quickest car on the road away from a standstill, with the
/// chassis only about 12 cm off the ground. Four seats behind glass, the back two cramped.
pub const SPORTS_SEATS: usize = 4;
pub const SPORTS_HALF_EXTENTS: [f32; 3] = [0.84, 0.3, 2.2];
pub const SPORTS_WHEEL_RADIUS: f32 = 0.3;
pub const SPORTS_TRACK_HALF: f32 = 0.7;
pub const SPORTS_AXLE_Z: f32 = 1.26;
pub const SPORTS_HARD_POINT_Y: f32 = 0.05;
pub const SPORTS_SUSPENSION_REST: f32 = 0.26;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const SPORTS_RIDE_HEIGHT: f32 = 0.417;
pub const SPORTS_MAX_STEER: f32 = 0.5;
pub const SPORTS_MAX_SPEED: f32 = 46.0;
/// The driver on the left, a passenger beside them, and two more on the low back bench.
pub const SPORTS_SEAT_HEADS: [[f32; 3]; 4] = [[-0.37, 0.72, 0.1], [0.37, 0.72, 0.1], [-0.38, 0.66, 0.85], [0.38, 0.66, 0.85]];

/// The engine up front and the fuel tank under the boot floor; the crew sit behind glass.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.0, -1.6], [0.45, 0.22, 0.45], 110),
        Module::new(Part::Fuel, [0.0, -0.15, 1.7], [0.4, 0.12, 0.25], 90),
    ],
    canvas: false,
};

/// The driver's door, measured off the model (`build_car.py` prints it): its hinge, length and height.
/// The passenger's is its mirror.
pub const SPORTS_DOOR: DoorDef = DoorDef::front([-0.84, -0.267, -0.7], 1.28, 1.1);

pub(super) const SPORTS: VehicleDef = VehicleDef {
    kind: KIND_SPORTS,
    name: "Opel Manta",
    hp: 100,
    seats: SPORTS_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &SPORTS_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: SPORTS_HALF_EXTENTS,
        // 4.4 m³ of low body is about 1.05 tonnes.
        density: 237.0,
        wheel_radius: SPORTS_WHEEL_RADIUS,
        track_half: SPORTS_TRACK_HALF,
        axles: &[-SPORTS_AXLE_Z, SPORTS_AXLE_Z],
        hard_point_y: SPORTS_HARD_POINT_Y,
        suspension_rest: SPORTS_SUSPENSION_REST,
        max_suspension_force: 60_000.0,
        // About 12 m/s² away from a standstill: the quickest off the line.
        engine_force: 12_600.0,
        max_speed: SPORTS_MAX_SPEED,
        steer_lock: SPORTS_MAX_STEER,
        steer_axles: Spec::FRONT_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        // Low enough that it stays on its wheels at full lock flat out (measured in vehicle/tests.rs).
        com_drop: 0.3,
        tank: Some(BOOST_FULL),
        fuel: Some(480.0),
        buttons: Buttons::Boost,
        boost: Some(Boost::CAR),
        aims: false,
        solids: &[],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        // The underside rises from under each axle out to the bumper.
        sweep: 0.8,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        // It skips over water flat out, and a turn drops it in.
        skim: Some(Skim { sink_speed: 22.0, plane_speed: 30.0, thrust: 0.15, grip: 0.35, bank_drag: 1.8 }),
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
    doors: &[SPORTS_DOOR, SPORTS_DOOR.mirrored()],
};
