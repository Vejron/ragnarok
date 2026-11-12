//! The snowmobile: everything the game knows about it.

use super::*;

/// The snowmobile (`KIND_SNOWMOBILE`): a modern mountain sled, light and very powerful, for two
/// riders one behind the other. The physics drives it as a car: the front pair of wheels are its
/// skis, which steer, and two pairs behind stand in for the track, whose long footprint keeps it
/// straight with a gun in tow. Its power to weight takes it up slopes no car can manage, and a Pak
/// or a mortar up steep ones.
pub const SNOWMOBILE_SEATS: usize = 2;
pub const SNOWMOBILE_HALF_EXTENTS: [f32; 3] = [0.55, 0.3, 1.6];
pub const SNOWMOBILE_WHEEL_RADIUS: f32 = 0.22;
/// Half the ski stance: the skis are 1.1 m apart.
pub const SNOWMOBILE_TRACK_HALF: f32 = 0.55;
pub const SNOWMOBILE_AXLE_Z: f32 = 1.2;
/// The track touches down from just behind the seat's front to the tail: a middle axle stands in for its front end.
pub const SNOWMOBILE_TRACK_Z: f32 = 0.3;
pub const SNOWMOBILE_HARD_POINT_Y: f32 = -0.05;
/// Long-travel suspension, for bumps and drifts.
pub const SNOWMOBILE_SUSPENSION_REST: f32 = 0.35;
pub const SNOWMOBILE_RIDE_HEIGHT: f32 = 0.559;
/// Skis turn far: tight at a crawl, and the lock fades with speed as every car's does.
pub const SNOWMOBILE_MAX_STEER: f32 = 0.6;
pub const SNOWMOBILE_MAX_SPEED: f32 = 20.0;
/// The boost: holding run opens the engine right up, multiplying its pull and its top speed, while
/// the boost lasts (a share of full, `Spec::tank`). It runs down in about four seconds held and
/// fills again in about six once let go.
pub const SNOWMOBILE_BOOST: f32 = 1.0;
pub const SNOWMOBILE_BOOST_POWER: f32 = 1.8;
pub const SNOWMOBILE_BOOST_SPEED: f32 = 1.35;
pub const SNOWMOBILE_BOOST_DRAIN: f32 = 0.25;
pub const SNOWMOBILE_BOOST_REFILL: f32 = 0.17;
/// The driver sits up front, the passenger behind and a little higher on the seat's rise.
pub const SNOWMOBILE_SEAT_HEADS: [[f32; 3]; 2] = [[0.0, 1.05, 0.1], [0.0, 1.15, 0.8]];

/// The engine under the hood and the fuel tank under the seat; the riders are in the open.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.0, -0.95], [0.35, 0.2, 0.35], 90),
        Module::new(Part::Fuel, [0.0, 0.15, 0.2], [0.25, 0.1, 0.3], 70),
    ],
    canvas: false,
};

pub(super) const SNOWMOBILE: VehicleDef = VehicleDef {
    kind: KIND_SNOWMOBILE,
    name: "Snowmobile",
    hp: 80,
    seats: SNOWMOBILE_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Open,
    seat_heads: &SNOWMOBILE_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: SNOWMOBILE_HALF_EXTENTS,
        // The box is 2.1 m³, so a snowmobile with its riders' kit weighs about 420 kg, and its engine
        // pulls half again the jeep's: it climbs 45° alone and 25° towing a tonne of Pak.
        density: 200.0,
        wheel_radius: SNOWMOBILE_WHEEL_RADIUS,
        track_half: SNOWMOBILE_TRACK_HALF,
        axles: &[-SNOWMOBILE_AXLE_Z, SNOWMOBILE_TRACK_Z, SNOWMOBILE_AXLE_Z],
        hard_point_y: SNOWMOBILE_HARD_POINT_Y,
        suspension_rest: SNOWMOBILE_SUSPENSION_REST,
        max_suspension_force: 25_000.0,
        engine_force: 16_000.0,
        max_speed: SNOWMOBILE_MAX_SPEED,
        steer_lock: SNOWMOBILE_MAX_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        // The weight is put right down at the track, as a rider hangs off the uphill side: it holds a
        // steep side slope, and a full-lock turn while the tail slides, without rolling.
        com_drop: 0.5,
        tank: Some(SNOWMOBILE_BOOST),
        buttons: Buttons::Sled,
        aims: false,
        solids: &[],
        tongue: 0.0,
        // On snow the track lets go in a hard turn and the tail comes round.
        slide: 0.6,
        // But its skis bite and roll it over in a hard turn at full tilt, when the tail no longer slides.
        roll: 1.0,
        // The hood sweeps up from the front of the seat to over the ski tips, so it rides up a slope.
        nose: 0.6,
        // The skis are leaned and steered into a turn, not flicked.
        steer_time: 0.2,
        // Light and powerful, it planes over open water from a modest speed and carves on it.
        skim: Some(Skim { sink_speed: 6.0, plane_speed: 10.0, thrust: 0.15, grip: 0.6, bank_drag: 1.0 }),
        bow: 0.0,
        springs: Springs::FIRM,
        airframe: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
    board: None,
    exit: None,
    fittings: &[],
};
