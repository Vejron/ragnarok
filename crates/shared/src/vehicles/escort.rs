//! The Ford Escort: everything the game knows about it.

use super::*;

/// The Ford Escort Mk3 (`KIND_ESCORT`): a small hatchback with a bustle at its back, cheap, light
/// and slow. Five seats behind glass.
pub const ESCORT_SEATS: usize = 5;
pub const ESCORT_HALF_EXTENTS: [f32; 3] = [0.82, 0.3, 2.0];
pub const ESCORT_WHEEL_RADIUS: f32 = 0.28;
pub const ESCORT_TRACK_HALF: f32 = 0.7;
pub const ESCORT_AXLE_Z: f32 = 1.2;
pub const ESCORT_HARD_POINT_Y: f32 = 0.0;
pub const ESCORT_SUSPENSION_REST: f32 = 0.258;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const ESCORT_RIDE_HEIGHT: f32 = 0.445;
pub const ESCORT_MAX_STEER: f32 = 0.55;
/// Slow flat out, m/s.
pub const ESCORT_MAX_SPEED: f32 = 35.0;
/// The driver on the left, then the passengers, front to back.
pub const ESCORT_SEAT_HEADS: [[f32; 3]; 5] = [[-0.38, 0.73, -0.05], [0.38, 0.73, -0.05], [-0.4, 0.71, 0.75], [0.4, 0.71, 0.75], [0.0, 0.71, 0.75]];

/// The engine under the bonnet and the fuel tank under the boot floor; the crew sit behind glass.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.05, -1.45], [0.4, 0.25, 0.4], 110),
        Module::new(Part::Fuel, [0.0, -0.15, 1.4], [0.4, 0.12, 0.3], 90),
    ],
    canvas: false,
};

/// The driver's door, measured off the model (`build_car.py` prints it): its hinge, length and height.
/// The passenger's is its mirror.
pub const ESCORT_DOOR: DoorDef = DoorDef::front([-0.82, -0.275, -1.0], 1.05, 1.16);

pub(super) const ESCORT: VehicleDef = VehicleDef {
    kind: KIND_ESCORT,
    name: "Ford Escort",
    hp: 100,
    seats: ESCORT_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &ESCORT_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: ESCORT_HALF_EXTENTS,
        // 3.9 m³ of box is about 900 kg.
        density: 229.0,
        wheel_radius: ESCORT_WHEEL_RADIUS,
        track_half: ESCORT_TRACK_HALF,
        axles: &[-ESCORT_AXLE_Z, ESCORT_AXLE_Z],
        hard_point_y: ESCORT_HARD_POINT_Y,
        suspension_rest: ESCORT_SUSPENSION_REST,
        max_suspension_force: 50_000.0,
        // About 5.5 m/s² away from a standstill.
        engine_force: 4_950.0,
        max_speed: ESCORT_MAX_SPEED,
        steer_lock: ESCORT_MAX_STEER,
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
    doors: &[ESCORT_DOOR, ESCORT_DOOR.mirrored()],
};
