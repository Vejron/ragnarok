//! The bus: everything the game knows about it.

use super::*;

/// The bus (`KIND_BUS`): a twelve-metre city bus on two axles, with a driver at the front and twelve
/// passengers in six rows of window seats behind. It is heavy, slow to get going and slow flat out,
/// and it needs most of a road to turn. On a world with bus lines the server drives them
/// (`Buses`); one whose driver is gone drives like any other car.
pub const BUS_SEATS: usize = 13;
/// Half the frame under the floor, metres: the body above it is a solid of its own (`BUS_BODY_HALF`).
pub const BUS_HALF_EXTENTS: [f32; 3] = [1.25, 0.5, 6.0];
/// Half the body over the frame, and its middle in the hull frame, metres: the roof is 2.3 m over the
/// hull's middle.
pub const BUS_BODY_HALF: [f32; 3] = [1.25, 0.9, 6.0];
pub const BUS_BODY_CENTRE: [f32; 3] = [0.0, 1.4, 0.0];
pub const BUS_WHEEL_RADIUS: f32 = 0.5;
pub const BUS_TRACK_HALF: f32 = 1.05;
/// The axles are six metres apart, with three metres of bus past each.
pub const BUS_AXLE_Z: f32 = 3.0;
pub const BUS_HARD_POINT_Y: f32 = -0.2;
pub const BUS_SUSPENSION_REST: f32 = 0.45;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const BUS_RIDE_HEIGHT: f32 = 1.057;
/// A sharp lock for so long a wheelbase: it turns in a circle of about nine metres' radius.
pub const BUS_MAX_STEER: f32 = 0.6;
/// About 58 km/h.
pub const BUS_MAX_SPEED: f32 = 16.0;
/// The driver at the front on the left, then six rows of two window seats, front to back.
pub const BUS_SEAT_HEADS: [[f32; 3]; 13] = [
    [-0.75, 1.3, -4.9],
    [-0.85, 1.3, -3.2],
    [0.85, 1.3, -3.2],
    [-0.85, 1.3, -1.7],
    [0.85, 1.3, -1.7],
    [-0.85, 1.3, -0.2],
    [0.85, 1.3, -0.2],
    [-0.85, 1.3, 1.3],
    [0.85, 1.3, 1.3],
    [-0.85, 1.3, 2.8],
    [0.85, 1.3, 2.8],
    [-0.85, 1.3, 4.3],
    [0.85, 1.3, 4.3],
];
/// Where it is boarded from, in the hull frame: its front door, on the right behind the front axle's
/// overhang.
pub const BUS_DOOR: [f32; 3] = [1.25, 0.0, -4.6];
/// Where a rider who gets off stands, in the hull frame (the middle of a standing soldier): on the
/// kerb outside the front door.
pub const BUS_DOOR_EXIT: [f32; 3] = [2.1, -0.1, -4.6];

/// The engine across the back under the rear seats and the fuel tank under the floor amidships; its
/// riders sit behind glass, hit as anyone is.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.0, 5.3], [0.9, 0.4, 0.6], 160),
        Module::new(Part::Fuel, [0.0, -0.2, 1.0], [0.6, 0.2, 0.6], 120),
    ],
    canvas: false,
};

pub(super) const BUS: VehicleDef = VehicleDef {
    kind: KIND_BUS,
    name: "Bus",
    hp: 200,
    seats: BUS_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &BUS_SEAT_HEADS,
    crew_guns: true,
    // Twelve tonnes: it rams harder than a car and is hurt less by what it hits.
    body: Body { drop_y: 1.3, ram: 2.0, crash: 0.5, ..CAR },
    spec: Spec {
        half_extents: BUS_HALF_EXTENTS,
        // 30 m³ of frame box is about twelve tonnes; the body over it adds no mass.
        density: 400.0,
        wheel_radius: BUS_WHEEL_RADIUS,
        track_half: BUS_TRACK_HALF,
        axles: &[-BUS_AXLE_Z, BUS_AXLE_Z],
        hard_point_y: BUS_HARD_POINT_Y,
        suspension_rest: BUS_SUSPENSION_REST,
        max_suspension_force: 400_000.0,
        // About 3.5 m/s² away from a stop, under half a jeep's.
        engine_force: 42_000.0,
        max_speed: BUS_MAX_SPEED,
        steer_lock: BUS_MAX_STEER,
        steer_axles: Spec::FRONT_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        // Its engine and frame are low under a tall, light body.
        com_drop: 0.3,
        tank: Some(BOOST_FULL),
        buttons: Buttons::Boost,
        boost: Some(Boost::HEAVY),
        aims: false,
        solids: &[Solid { shape: Shape::Cuboid(BUS_BODY_HALF), at: BUS_BODY_CENTRE, role: Role::Part }],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        // The underside rises from each axle out to the bumper.
        sweep: 3.0,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        // Far too heavy to skim: it sinks and wades along the bed.
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
    board: Some(BUS_DOOR),
    exit: Some(BUS_DOOR_EXIT),
    fittings: &[],
};
