//! The Golf GTI: everything the game knows about it.

use super::*;

/// The Volkswagen Golf GTI Mk2 (`KIND_GTI`): a short, light three-door hot hatch. The quickest of the
/// everyday cars off the line and darting through traffic, though a Saab turbo walks away from it
/// flat out. Four seats behind glass.
pub const GTI_SEATS: usize = 4;
pub const GTI_HALF_EXTENTS: [f32; 3] = [0.84, 0.3, 2.0];
pub const GTI_WHEEL_RADIUS: f32 = 0.29;
pub const GTI_TRACK_HALF: f32 = 0.715;
pub const GTI_AXLE_Z: f32 = 1.235;
pub const GTI_HARD_POINT_Y: f32 = 0.0;
pub const GTI_SUSPENSION_REST: f32 = 0.248;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const GTI_RIDE_HEIGHT: f32 = 0.445;
pub const GTI_MAX_STEER: f32 = 0.55;
/// Flat out, m/s.
pub const GTI_MAX_SPEED: f32 = 44.0;
/// The driver on the left, then the passengers, front to back.
pub const GTI_SEAT_HEADS: [[f32; 3]; 4] = [[-0.38, 0.73, -0.05], [0.38, 0.73, -0.05], [-0.4, 0.71, 0.75], [0.4, 0.71, 0.75]];

/// Its cabin over the hull box, measured off the model's glass: what a round meets above the hull,
/// from the beltline to the roof. It is glass (`GLAZED`): a round goes through it to whoever sits behind.
const CABIN: Solid = Solid::cabin(GTI_HALF_EXTENTS[1], 0.96, [-1.05, 1.86], [-0.70, 1.25], [0.80, 0.64]);

/// The engine under the bonnet and the fuel tank under the boot floor; the crew sit behind glass.
const ARMOR: Armor = Armor {
    plates: GLAZED.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.05, -1.45], [0.4, 0.25, 0.4], 110),
        Module::new(Part::Fuel, [0.0, -0.15, 1.35], [0.4, 0.12, 0.3], 90),
    ],
    canvas: false,
};

/// The driver's door, measured off the model (`build_car.py` prints it): its hinge, length and height.
/// The passenger's is its mirror.
pub const GTI_DOOR: DoorDef = DoorDef::front([-0.84, -0.275, -1.0], 1.32, 1.19);

pub(super) const GTI: VehicleDef = VehicleDef {
    kind: KIND_GTI,
    name: "Golf GTI",
    hp: 100,
    seats: GTI_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &GTI_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: GTI_HALF_EXTENTS,
        // 4.0 m³ of box is about 950 kg, the lightest of the closed cars.
        density: 236.0,
        wheel_radius: GTI_WHEEL_RADIUS,
        track_half: GTI_TRACK_HALF,
        axles: &[-GTI_AXLE_Z, GTI_AXLE_Z],
        hard_point_y: GTI_HARD_POINT_Y,
        suspension_rest: GTI_SUSPENSION_REST,
        max_suspension_force: 50_000.0,
        // About 10.5 m/s² away from a standstill.
        engine_force: 10_000.0,
        max_speed: GTI_MAX_SPEED,
        steer_lock: GTI_MAX_STEER,
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
        sweep: 0.75,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        // Light enough to skip over a lake flat out, and a turn drops it in.
        skim: Some(Skim { sink_speed: 20.0, plane_speed: 28.0, thrust: 0.15, grip: 0.4, bank_drag: 1.7 }),
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
    doors: &[GTI_DOOR, GTI_DOOR.mirrored()],
};
