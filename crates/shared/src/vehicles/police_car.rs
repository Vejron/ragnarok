//! The police car: everything the game knows about it.

use super::*;

/// The police car (`KIND_POLICE_CAR`): a Volvo 240 estate of the 1980s in white with a blue stripe
/// and a light bar on its roof. The same body, weight and springs as the Volvo 240 (`volvo_240.rs`),
/// with an engine that keeps pulling to a higher top speed, so it can catch the traffic. Five seats
/// behind glass.
pub const POLICE_CAR_SEATS: usize = 5;
pub const POLICE_CAR_HALF_EXTENTS: [f32; 3] = [0.86, 0.36, 2.4];
pub const POLICE_CAR_WHEEL_RADIUS: f32 = 0.32;
pub const POLICE_CAR_TRACK_HALF: f32 = 0.73;
pub const POLICE_CAR_AXLE_Z: f32 = 1.32;
pub const POLICE_CAR_HARD_POINT_Y: f32 = 0.0;
pub const POLICE_CAR_SUSPENSION_REST: f32 = 0.303;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const POLICE_CAR_RIDE_HEIGHT: f32 = 0.506;
pub const POLICE_CAR_MAX_STEER: f32 = 0.55;
/// Flat out, m/s: about 15 % more than the Volvo 240's, well over what the traffic cruises at.
pub const POLICE_CAR_MAX_SPEED: f32 = 41.5;
/// The driver on the left, then the passengers, front to back.
pub const POLICE_CAR_SEAT_HEADS: [[f32; 3]; 5] = [[-0.38, 0.67, 0.0], [0.38, 0.67, 0.0], [-0.4, 0.65, 0.88], [0.4, 0.65, 0.88], [0.0, 0.65, 0.88]];

/// The engine under the bonnet and the fuel tank under the boot floor; the crew sit behind glass.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.05, -1.7], [0.4, 0.25, 0.4], 110),
        Module::new(Part::Fuel, [0.0, -0.18, 1.7], [0.4, 0.12, 0.3], 90),
    ],
    canvas: false,
};

/// The driver's door, measured off the model (`build_car.py` prints it): its hinge, length and height.
/// The passenger's is its mirror.
pub const POLICE_CAR_DOOR: DoorDef = DoorDef::front([-0.86, -0.316, -1.28], 1.03, 1.22);

pub(super) const POLICE_CAR: VehicleDef = VehicleDef {
    kind: KIND_POLICE_CAR,
    name: "Police car",
    hp: 100,
    seats: POLICE_CAR_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &POLICE_CAR_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: POLICE_CAR_HALF_EXTENTS,
        // 5.9 m³ of box is about 1.45 tonnes, the heaviest of the saloons.
        density: 244.0,
        wheel_radius: POLICE_CAR_WHEEL_RADIUS,
        track_half: POLICE_CAR_TRACK_HALF,
        axles: &[-POLICE_CAR_AXLE_Z, POLICE_CAR_AXLE_Z],
        hard_point_y: POLICE_CAR_HARD_POINT_Y,
        suspension_rest: POLICE_CAR_SUSPENSION_REST,
        max_suspension_force: 50_000.0,
        // About 5 m/s² away from a standstill: it takes its time.
        engine_force: 7_250.0,
        max_speed: POLICE_CAR_MAX_SPEED,
        steer_lock: POLICE_CAR_MAX_STEER,
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
        // Softer than the saloons: it leans in a bend and wallows over a bump.
        springs: Springs { stiffness: 20.0, compression: 1.8, damping: 2.6, travel: 0.35 },
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
    doors: &[POLICE_CAR_DOOR, POLICE_CAR_DOOR.mirrored()],
};
