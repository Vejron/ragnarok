//! The Saab 900 Turbo: everything the game knows about it.

use super::*;

/// The Saab 900 Turbo (`KIND_SAAB_900`): a three-door hatchback from Trollhättan with a wraparound
/// windscreen and a long, sloping nose. Its turbo wakes late, so it is no quicker than a family car
/// off the line but runs on to the highest top speed of the everyday cars. Four seats behind glass.
pub const SAAB_900_SEATS: usize = 4;
pub const SAAB_900_HALF_EXTENTS: [f32; 3] = [0.85, 0.32, 2.33];
pub const SAAB_900_WHEEL_RADIUS: f32 = 0.31;
pub const SAAB_900_TRACK_HALF: f32 = 0.72;
pub const SAAB_900_AXLE_Z: f32 = 1.26;
pub const SAAB_900_HARD_POINT_Y: f32 = 0.0;
pub const SAAB_900_SUSPENSION_REST: f32 = 0.248;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const SAAB_900_RIDE_HEIGHT: f32 = 0.465;
pub const SAAB_900_MAX_STEER: f32 = 0.5;
/// The fastest of the everyday cars flat out, m/s.
pub const SAAB_900_MAX_SPEED: f32 = 48.0;
/// The driver on the left, then the passengers, front to back.
pub const SAAB_900_SEAT_HEADS: [[f32; 3]; 4] = [[-0.38, 0.71, 0.0], [0.38, 0.71, 0.0], [-0.4, 0.69, 0.88], [0.4, 0.69, 0.88]];

/// The engine under the bonnet and the fuel tank under the boot floor; the crew sit behind glass.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.05, -1.65], [0.4, 0.25, 0.4], 110),
        Module::new(Part::Fuel, [0.0, -0.15, 1.65], [0.4, 0.12, 0.3], 90),
    ],
    canvas: false,
};

/// The driver's door, measured off the model (`build_car.py` prints it): its hinge, length and height.
/// The passenger's is its mirror.
pub const SAAB_900_DOOR: DoorDef = DoorDef::front([-0.85, -0.285, -0.91], 1.2, 1.19);

pub(super) const SAAB_900: VehicleDef = VehicleDef {
    kind: KIND_SAAB_900,
    name: "Saab 900 Turbo",
    hp: 100,
    seats: SAAB_900_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &SAAB_900_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: SAAB_900_HALF_EXTENTS,
        // 5.1 m³ of box is about 1.25 tonnes.
        density: 246.0,
        wheel_radius: SAAB_900_WHEEL_RADIUS,
        track_half: SAAB_900_TRACK_HALF,
        axles: &[-SAAB_900_AXLE_Z, SAAB_900_AXLE_Z],
        hard_point_y: SAAB_900_HARD_POINT_Y,
        suspension_rest: SAAB_900_SUSPENSION_REST,
        max_suspension_force: 50_000.0,
        // About 8 m/s² away from a standstill: the turbo is still waking.
        engine_force: 10_000.0,
        max_speed: SAAB_900_MAX_SPEED,
        steer_lock: SAAB_900_MAX_STEER,
        steer_axles: Spec::FRONT_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        // Low enough that it stays on its wheels at full lock flat out (measured in vehicle/tests.rs).
        com_drop: 0.4,
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
    doors: &[SAAB_900_DOOR, SAAB_900_DOOR.mirrored()],
};
