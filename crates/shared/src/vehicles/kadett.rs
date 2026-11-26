//! The Opel Kadett: everything the game knows about it.

use super::*;

/// The Opel Kadett E (`KIND_KADETT`): a wedge-shaped five-door hatchback, the everyday small car of
/// the late eighties. Five seats behind glass.
pub const KADETT_SEATS: usize = 5;
pub const KADETT_HALF_EXTENTS: [f32; 3] = [0.83, 0.3, 2.0];
pub const KADETT_WHEEL_RADIUS: f32 = 0.29;
pub const KADETT_TRACK_HALF: f32 = 0.7;
pub const KADETT_AXLE_Z: f32 = 1.26;
pub const KADETT_HARD_POINT_Y: f32 = 0.0;
pub const KADETT_SUSPENSION_REST: f32 = 0.248;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const KADETT_RIDE_HEIGHT: f32 = 0.445;
pub const KADETT_MAX_STEER: f32 = 0.55;
/// Flat out, m/s.
pub const KADETT_MAX_SPEED: f32 = 38.0;
/// The driver on the left, then the passengers, front to back.
pub const KADETT_SEAT_HEADS: [[f32; 3]; 5] = [[-0.38, 0.73, -0.05], [0.38, 0.73, -0.05], [-0.4, 0.71, 0.75], [0.4, 0.71, 0.75], [0.0, 0.71, 0.75]];

/// The engine under the bonnet and the fuel tank under the boot floor; the crew sit behind glass.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.05, -1.45], [0.4, 0.25, 0.4], 110),
        Module::new(Part::Fuel, [0.0, -0.15, 1.35], [0.4, 0.12, 0.3], 90),
    ],
    canvas: false,
};

/// The driver's door, measured off the model (`build_car.py` prints it): its hinge, length and height.
/// The passenger's is its mirror.
pub const KADETT_DOOR: DoorDef = DoorDef::front([-0.83, -0.275, -1.0], 1.05, 1.18);

pub(super) const KADETT: VehicleDef = VehicleDef {
    kind: KIND_KADETT,
    name: "Opel Kadett",
    hp: 100,
    seats: KADETT_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &KADETT_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: KADETT_HALF_EXTENTS,
        // 4.0 m³ of box is about 950 kg.
        density: 238.0,
        wheel_radius: KADETT_WHEEL_RADIUS,
        track_half: KADETT_TRACK_HALF,
        axles: &[-KADETT_AXLE_Z, KADETT_AXLE_Z],
        hard_point_y: KADETT_HARD_POINT_Y,
        suspension_rest: KADETT_SUSPENSION_REST,
        max_suspension_force: 50_000.0,
        // About 7 m/s² away from a standstill.
        engine_force: 6_650.0,
        max_speed: KADETT_MAX_SPEED,
        steer_lock: KADETT_MAX_STEER,
        steer_axles: Spec::FRONT_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        // Low enough that it stays on its wheels at full lock flat out (measured in vehicle/tests.rs).
        com_drop: 0.3,
        tank: Some(BOOST_FULL),
        fuel: Spec::FUEL,
        buttons: Buttons::Boost,
        boost: Some(Boost::CAR),
        aims: false,
        solids: &[],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        // The underside rises from each axle out to the bumper.
        sweep: 0.75,
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
    doors: &[KADETT_DOOR, KADETT_DOOR.mirrored()],
};
