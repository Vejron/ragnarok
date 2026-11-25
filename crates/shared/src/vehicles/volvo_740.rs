//! The Volvo 740: everything the game knows about it.

use super::*;

/// The Volvo 740 saloon (`KIND_VOLVO_740`): a square-cut four-door, solid and unhurried, in the
/// middle of the pack for pace. Five seats behind glass.
pub const VOLVO_740_SEATS: usize = 5;
pub const VOLVO_740_HALF_EXTENTS: [f32; 3] = [0.88, 0.34, 2.4];
pub const VOLVO_740_WHEEL_RADIUS: f32 = 0.32;
pub const VOLVO_740_TRACK_HALF: f32 = 0.735;
pub const VOLVO_740_AXLE_Z: f32 = 1.385;
pub const VOLVO_740_HARD_POINT_Y: f32 = 0.0;
pub const VOLVO_740_SUSPENSION_REST: f32 = 0.258;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const VOLVO_740_RIDE_HEIGHT: f32 = 0.485;
pub const VOLVO_740_MAX_STEER: f32 = 0.55;
/// Flat out, m/s.
pub const VOLVO_740_MAX_SPEED: f32 = 40.0;
/// The driver on the left, then the passengers, front to back.
pub const VOLVO_740_SEAT_HEADS: [[f32; 3]; 5] = [[-0.38, 0.69, 0.0], [0.38, 0.69, 0.0], [-0.4, 0.67, 0.88], [0.4, 0.67, 0.88], [0.0, 0.67, 0.88]];

/// The engine under the bonnet and the fuel tank under the boot floor; the crew sit behind glass.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.05, -1.7], [0.4, 0.25, 0.4], 110),
        Module::new(Part::Fuel, [0.0, -0.15, 1.7], [0.4, 0.12, 0.3], 90),
    ],
    canvas: false,
};

/// The driver's door, measured off the model (`build_car.py` prints it): its hinge, length and height.
/// The passenger's is its mirror.
pub const VOLVO_740_DOOR: DoorDef = DoorDef::front([-0.88, -0.305, -1.18], 1.03, 1.18);

pub(super) const VOLVO_740: VehicleDef = VehicleDef {
    kind: KIND_VOLVO_740,
    name: "Volvo 740",
    hp: 100,
    seats: VOLVO_740_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &VOLVO_740_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: VOLVO_740_HALF_EXTENTS,
        // 5.7 m³ of box is about 1.4 tonnes.
        density: 244.0,
        wheel_radius: VOLVO_740_WHEEL_RADIUS,
        track_half: VOLVO_740_TRACK_HALF,
        axles: &[-VOLVO_740_AXLE_Z, VOLVO_740_AXLE_Z],
        hard_point_y: VOLVO_740_HARD_POINT_Y,
        suspension_rest: VOLVO_740_SUSPENSION_REST,
        max_suspension_force: 50_000.0,
        // About 6.5 m/s² away from a standstill.
        engine_force: 9_100.0,
        max_speed: VOLVO_740_MAX_SPEED,
        steer_lock: VOLVO_740_MAX_STEER,
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
        sweep: 0.9,
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
    doors: &[VOLVO_740_DOOR, VOLVO_740_DOOR.mirrored()],
};
