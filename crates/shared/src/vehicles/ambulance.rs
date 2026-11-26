//! The ambulance: everything the game knows about it.

use super::*;

/// The ambulance (`KIND_AMBULANCE`): a Volkswagen Transporter T3 in white and yellow with a red
/// stripe and a light bar on its roof. Not the air ambulance (`KIND_AIR_AMBULANCE`), which flies. The
/// same body, weight and springs as the Transporter (`transporter.rs`) and its seven seats behind
/// glass: the driver and a passenger over the front wheels, then two by the sliding door and three at
/// the back. A little faster flat out than the van it is built on.
pub const AMBULANCE_SEATS: usize = 7;
pub const AMBULANCE_HALF_EXTENTS: [f32; 3] = [0.92, 0.5, 2.25];
pub const AMBULANCE_WHEEL_RADIUS: f32 = 0.33;
pub const AMBULANCE_TRACK_HALF: f32 = 0.785;
pub const AMBULANCE_AXLE_Z: f32 = 1.23;
pub const AMBULANCE_HARD_POINT_Y: f32 = 0.0;
pub const AMBULANCE_SUSPENSION_REST: f32 = 0.486;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const AMBULANCE_RIDE_HEIGHT: f32 = 0.685;
pub const AMBULANCE_MAX_STEER: f32 = 0.6;
/// Flat out, m/s: a little more than the Transporter's.
pub const AMBULANCE_MAX_SPEED: f32 = 34.0;
/// The driver on the left, then the passengers, front to back.
pub const AMBULANCE_SEAT_HEADS: [[f32; 3]; 7] = [[-0.45, 0.87, -1.25], [0.45, 0.87, -1.25], [-0.4, 0.87, 0.05], [0.4, 0.87, 0.05], [-0.5, 0.87, 1.15], [0.0, 0.87, 1.15], [0.5, 0.87, 1.15]];

/// Its cabin over the hull box, measured off the model's glass: what a round meets above the hull,
/// from the beltline to the roof. It is glass (`GLAZED`): a round goes through it to whoever sits behind.
const CABIN: Solid = Solid::cabin(AMBULANCE_HALF_EXTENTS[1], 1.26, [-2.17, 2.25], [-1.65, 2.23], [0.87, 0.81]);

/// The engine under the floor at the back and the fuel tank under the middle of the floor; the crew sit behind glass.
const ARMOR: Armor = Armor {
    plates: GLAZED.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, -0.1, 1.7], [0.4, 0.25, 0.4], 110),
        Module::new(Part::Fuel, [0.0, -0.3, 0.2], [0.4, 0.12, 0.3], 90),
    ],
    canvas: false,
};

/// The driver's door, measured off the model (`build_car.py` prints it): its hinge, length and height.
/// The passenger's is its mirror.
pub const AMBULANCE_DOOR: DoorDef = DoorDef::front([-0.913, -0.395, -2.17], 0.97, 1.61);

pub(super) const AMBULANCE: VehicleDef = VehicleDef {
    kind: KIND_AMBULANCE,
    name: "Ambulance",
    hp: 100,
    seats: AMBULANCE_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &AMBULANCE_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: AMBULANCE_HALF_EXTENTS,
        // 8.3 m³ of box, mostly air, is about 1.6 tonnes.
        density: 193.0,
        wheel_radius: AMBULANCE_WHEEL_RADIUS,
        track_half: AMBULANCE_TRACK_HALF,
        axles: &[-AMBULANCE_AXLE_Z, AMBULANCE_AXLE_Z],
        hard_point_y: AMBULANCE_HARD_POINT_Y,
        suspension_rest: AMBULANCE_SUSPENSION_REST,
        max_suspension_force: 50_000.0,
        // About 4 m/s² away from a standstill: loaded, it crawls.
        engine_force: 6_400.0,
        drive: Drive::Rear,
        max_speed: AMBULANCE_MAX_SPEED,
        steer_lock: AMBULANCE_MAX_STEER,
        steer_axles: Spec::FRONT_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        // Low enough that it stays on its wheels at full lock flat out (measured in vehicle/tests.rs).
        com_drop: 0.4,
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
        sweep: 0.6,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        // Too heavy and too slow to skim: it sinks.
        skim: None,
        swim: None,
        bow: 0.0,
        profile: Spec::BOX,
        // Soft, with a long travel: it pitches and rolls.
        springs: Springs { stiffness: 18.0, compression: 1.6, damping: 2.4, travel: 0.4 },
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
    doors: &[AMBULANCE_DOOR, AMBULANCE_DOOR.mirrored()],
};
