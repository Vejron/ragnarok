//! The snowracer: everything the game knows about it.

use super::*;

/// The snowracer (`KIND_SNOWRACER`): a steerable sled, a short steel frame on three skis, with a seat
/// for a rider who steers by a little wheel and one more behind. It is built for the ski slopes: down
/// one it slides as fast as a snowmobile drives, with nothing but gravity pulling it. A small motor
/// under the seat takes it about on the flat and up a piste again at a crawl. The physics drives it
/// as a car: the front pair of wheels, close together, are its one steering ski, and the rear pair its
/// two runners. It slides on them rather than rolls (`Runners`).
pub const SNOWRACER_SEATS: usize = 2;
pub const SNOWRACER_HALF_EXTENTS: [f32; 3] = [0.3, 0.15, 0.75];
pub const SNOWRACER_WHEEL_RADIUS: f32 = 0.1;
/// Half the runners' stance: they are 0.5 m apart. The front ski stands on the same pair of points.
pub const SNOWRACER_TRACK_HALF: f32 = 0.25;
/// The front ski is under the nose, and the runners' middle under the passenger.
pub const SNOWRACER_AXLE_Z: f32 = 0.55;
pub const SNOWRACER_HARD_POINT_Y: f32 = -0.05;
/// A little give in the skis' mounts, for bumps.
pub const SNOWRACER_SUSPENSION_REST: f32 = 0.15;
pub const SNOWRACER_RIDE_HEIGHT: f32 = 0.202;
/// The front ski turns far at a crawl, and the lock fades with speed, fast (`SNOWRACER_STEER_FADE`).
pub const SNOWRACER_MAX_STEER: f32 = 0.5;
/// The speed at which the ski's lock has halved, m/s: at a slope's pace it steers a little.
pub const SNOWRACER_STEER_FADE: f32 = 8.0;
/// The motor's top speed on the flat, m/s. Downhill the motor gives nothing past it, and gravity
/// takes the sled on to as fast as its drag lets it (`SNOWRACER_DRAG`).
pub const SNOWRACER_MAX_SPEED: f32 = 6.0;
/// Sliding friction on snow, as a waxed ski's (`SKI_MU_SNOW`).
pub const SNOWRACER_MU: f32 = 0.05;
/// Air drag per (m/s)², m/s² per (m/s)², of two riders sitting up on a sled.
pub const SNOWRACER_DRAG: f32 = 0.0006;
/// In the air the nose swings onto the line it flies at this rate, per second, so it neither dives
/// nor tumbles off a bump.
pub const SNOWRACER_AIR_FOLLOW: f32 = 5.0;
/// How far the rider leans the nose down (W) or up (S) off that line in the air, radians (20°).
pub const SNOWRACER_AIR_LEAN: f32 = 0.35;
/// How fast the rider turns it in the air at full stick, rad/s.
pub const SNOWRACER_AIR_TURN: f32 = 1.5;
/// The share of its weight it is spared in the air, so that it flies far off a bump.
pub const SNOWRACER_AIR_FLOAT: f32 = 0.45;
/// Both riders sit low on the seat, the passenger behind the rider with their legs either side of them.
pub const SNOWRACER_SEAT_HEADS: [[f32; 3]; 2] = [[0.0, 0.85, 0.0], [0.0, 0.85, 0.5]];

/// Thin steel tube, but nothing in it to shoot out: the motor is too small to hit and keeps running.
/// The riders are in the open.
const ARMOR: Armor = Armor { plates: SHEET.plates, modules: &[], canvas: false };

pub(super) const SNOWRACER: VehicleDef = VehicleDef {
    kind: KIND_SNOWRACER,
    name: "Snowracer",
    // As tough as anything can be: it takes no harm from crashes (`SLED`) and soaks up a great deal of fire.
    hp: 255,
    seats: SNOWRACER_SEATS,
    mounts: &[],
    // Its runners cannot be bent or shot off as a car's wheels can.
    car_wheels: false,
    crew: Exposure::Open,
    seat_heads: &SNOWRACER_SEAT_HEADS,
    crew_guns: true,
    body: SLED,
    spec: Spec {
        half_extents: SNOWRACER_HALF_EXTENTS,
        // The box is 0.27 m³, so with its riders' weight in it a snowracer is about 150 kg.
        density: 550.0,
        wheel_radius: SNOWRACER_WHEEL_RADIUS,
        track_half: SNOWRACER_TRACK_HALF,
        axles: &[-SNOWRACER_AXLE_Z, SNOWRACER_AXLE_Z],
        hard_point_y: SNOWRACER_HARD_POINT_Y,
        suspension_rest: SNOWRACER_SUSPENSION_REST,
        max_suspension_force: 6_000.0,
        // About 5 m/s²: it creeps up a 20° piste, where gravity and the snow take 4.1, and stalls on a 30° one.
        engine_force: 700.0,
        max_speed: SNOWRACER_MAX_SPEED,
        steer_lock: SNOWRACER_MAX_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        // Its runners' drag holds it back, not this.
        linear_damping: 0.01,
        // The riders sit low between the runners.
        com_drop: 0.2,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[],
        tongue: 0.0,
        // The runners let go in a hard turn at speed and the tail swings out.
        slide: 0.5,
        // But they bite and tip it over in a hard turn at full tilt.
        roll: 1.0,
        // The ski's tip turns up, so it rides up onto a bump.
        nose: 0.75,
        // The ski answers at once; it is speed that calms it (`SNOWRACER_STEER_FADE`).
        steer_time: 0.0,
        steer_fade: SNOWRACER_STEER_FADE,
        // Too small and slow to plane: it sinks.
        skim: None,
        bow: 0.0,
        springs: Springs::FIRM,
        airframe: None,
        runners: Some(Runners {
            mu: SNOWRACER_MU,
            drag: SNOWRACER_DRAG,
            follow: SNOWRACER_AIR_FOLLOW,
            lean: SNOWRACER_AIR_LEAN,
            air_turn: SNOWRACER_AIR_TURN,
            float: SNOWRACER_AIR_FLOAT,
        }),
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
    board: None,
    exit: None,
    fittings: &[],
};
