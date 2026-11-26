//! The Opel Omega: everything the game knows about it.

use super::*;

/// The Opel Omega A (`KIND_OMEGA`): a long, smooth saloon with flush glass and a six under the
/// bonnet, that cruises fast and quietly. Five seats behind glass.
pub const OMEGA_SEATS: usize = 5;
pub const OMEGA_HALF_EXTENTS: [f32; 3] = [0.88, 0.33, 2.33];
pub const OMEGA_WHEEL_RADIUS: f32 = 0.31;
pub const OMEGA_TRACK_HALF: f32 = 0.725;
pub const OMEGA_AXLE_Z: f32 = 1.365;
pub const OMEGA_HARD_POINT_Y: f32 = 0.0;
pub const OMEGA_SUSPENSION_REST: f32 = 0.258;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const OMEGA_RIDE_HEIGHT: f32 = 0.475;
pub const OMEGA_MAX_STEER: f32 = 0.55;
/// Flat out, m/s.
pub const OMEGA_MAX_SPEED: f32 = 42.0;
/// The driver on the left, then the passengers, front to back.
pub const OMEGA_SEAT_HEADS: [[f32; 3]; 5] = [[-0.38, 0.7, 0.0], [0.38, 0.7, 0.0], [-0.4, 0.68, 0.88], [0.4, 0.68, 0.88], [0.0, 0.68, 0.88]];

/// Its cabin over the hull box, measured off the model's glass: what a round meets above the hull,
/// from the beltline to the roof. It is glass (`GLAZED`): a round goes through it to whoever sits behind.
const CABIN: Solid = Solid::cabin(OMEGA_HALF_EXTENTS[1], 0.95, [-1.23, 1.47], [-0.78, 0.92], [0.82, 0.69]);

/// The engine under the bonnet and the fuel tank under the boot floor; the crew sit behind glass.
const ARMOR: Armor = Armor {
    plates: GLAZED.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.05, -1.65], [0.4, 0.25, 0.4], 110),
        Module::new(Part::Fuel, [0.0, -0.15, 1.7], [0.4, 0.12, 0.3], 90),
    ],
    canvas: false,
};

/// The driver's door, measured off the model (`build_car.py` prints it): its hinge, length and height.
/// The passenger's is its mirror.
pub const OMEGA_DOOR: DoorDef = DoorDef::front([-0.88, -0.295, -1.17], 1.14, 1.2);

pub(super) const OMEGA: VehicleDef = VehicleDef {
    kind: KIND_OMEGA,
    name: "Opel Omega",
    hp: 100,
    seats: OMEGA_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &OMEGA_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: OMEGA_HALF_EXTENTS,
        // 5.4 m³ of box is about 1.35 tonnes.
        density: 249.0,
        wheel_radius: OMEGA_WHEEL_RADIUS,
        track_half: OMEGA_TRACK_HALF,
        axles: &[-OMEGA_AXLE_Z, OMEGA_AXLE_Z],
        hard_point_y: OMEGA_HARD_POINT_Y,
        suspension_rest: OMEGA_SUSPENSION_REST,
        max_suspension_force: 50_000.0,
        // About 7 m/s² away from a standstill.
        engine_force: 9_450.0,
        max_speed: OMEGA_MAX_SPEED,
        steer_lock: OMEGA_MAX_STEER,
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
        solids: &[CABIN],
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
    doors: &[OMEGA_DOOR, OMEGA_DOOR.mirrored()],
};
