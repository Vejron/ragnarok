//! The raft: everything the game knows about it.

use super::*;

/// The raft (`KIND_RAFT`): a small inflatable river-rafting boat for two, 3.6 m long and 1.6 m across
/// over its tubes, with a big outboard on its transom. Its driver sits on the left tube at the stern
/// with a hand on the tiller; the other sits on the thwart up in the bow. It floats a hand's breadth
/// deep, so it runs where the river is too thin for a heavier boat, and the motor shoves it out of the
/// water onto the plane at once.
pub const RAFT_SEATS: usize = 2;
/// Half the box over its tubes, metres: 3.6 m long, 1.6 m across and 0.5 m deep, the tubes' height.
pub const RAFT_HALF_EXTENTS: [f32; 3] = [0.8, 0.25, 1.8];
/// How deep it floats at rest, metres: an empty raft and its motor are light.
pub const RAFT_DRAFT: f32 = 0.12;
/// About 27 knots: a 40 hp outboard on a boat this light.
pub const RAFT_MAX_SPEED: f32 = 14.0;
/// How hard the outboard pushes it, m/s² at a standstill: it leaps off the mark.
pub const RAFT_THRUST: f32 = 6.0;
/// How hard it stops with the outboard put astern, m/s², and how far its bow digs in (radians) and its
/// tubes squat (metres) as it does.
pub const RAFT_BRAKE: f32 = 4.5;
pub const RAFT_BRAKE_DIP: f32 = 0.06;
pub const RAFT_BRAKE_SQUAT: f32 = 0.05;
/// Its top speed astern, m/s.
pub const RAFT_ASTERN_SPEED: f32 = 3.0;
/// How fast it turns under way with the outboard pushing, rad/s: it whips its light stern round, a
/// circle about 15 m across flat out.
pub const RAFT_TURN_RATE: f32 = 1.9;
/// How fast the outboard swings it round standing still, rad/s.
pub const RAFT_SPIN_RATE: f32 = 1.4;
/// How much higher it rides on the plane, metres, and the speeds it climbs onto it between, m/s: early,
/// as a light flat floor does.
pub const RAFT_PLANE_LIFT: f32 = 0.06;
pub const RAFT_PLANE_SPEED: [f32; 2] = [3.0, 7.0];
/// How far its bow lifts as it climbs onto the plane and how far it stays up once on it, radians: the
/// light bow rears up under the motor's shove.
pub const RAFT_BOW_RISE: f32 = 0.16;
pub const RAFT_BOW_PLANING: f32 = 0.05;
/// How far it banks into a turn, radians for each rad/s of turn and m/s of speed, and at most.
pub const RAFT_BANK: f32 = 0.016;
pub const RAFT_BANK_MAX: f32 = 0.22;
/// How quickly its rise and fall follows the water's, per second: the face of a wave lifts it at once.
pub const RAFT_HEAVE: f32 = 10.0;
/// The share of its sideways slide its floor and the outboard's leg take out a second: it carves its
/// turns, with a little slide at the start of a hard one.
pub const RAFT_KEEL: f32 = 3.5;
/// How hard it slows with the outboard let go, m/s²: it sits down in the water at once.
pub const RAFT_COAST: f32 = 3.0;
/// How hard it is brought level with the water under it, per second², and how quickly its roll and
/// pitch die away, per second: it pitches and rolls over every wave, and bobs.
pub const RAFT_LEVEL: f32 = 40.0;
pub const RAFT_ROCK: f32 = 2.5;
/// The share of its way through the water the water takes out a second while it coasts: the river
/// takes it up at once.
pub const RAFT_GRIP: f32 = 1.5;
/// How quickly its rate of turn follows the helm and the water, per second: it snaps round at once.
pub const RAFT_HELM: f32 = 7.0;
/// The share of its turning that comes from the outboard's thrust: let go, it answers the helm only
/// lazily.
pub const RAFT_THRUST_STEER: f32 = 0.65;
/// How far the water's own turning turns it, a share of the water's rotation: more than a speck of
/// foam turns, as the slack water holds one end while the current takes the other.
pub const RAFT_LEVER: f32 = 2.0;
/// The share of its drive and grip it keeps scraping over a shoal on its rubber floor.
pub const RAFT_SCRAPE: f32 = 0.5;
/// The outboard on the transom: where it turns on its bracket, in the hull frame, how long its tiller
/// is, metres, and how far it swings either way, radians.
pub const RAFT_MOTOR_PIVOT: [f32; 3] = [0.0, 0.1, 1.82];
pub const RAFT_TILLER: f32 = 0.75;
pub const RAFT_MOTOR_SWING: f32 = 0.5;
/// The driver on the left tube at the stern, a hand on the tiller, and the other on the thwart in the bow.
pub const RAFT_SEAT_HEADS: [[f32; 3]; 2] = [[-0.5, 1.05, 1.15], [0.0, 1.0, -0.85]];

/// Its tubes are rubber: a round goes through them and on into whoever sits behind. The motor is on the
/// transom, its fuel tank on the floor in front of it.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.25, 2.0], [0.2, 0.3, 0.25], 50),
        Module::new(Part::Fuel, [0.3, -0.1, 1.3], [0.2, 0.12, 0.25], 40),
    ],
    canvas: false,
};

/// Its side outline, `[z, y]`: a flat floor that sweeps up into the tubes at the bow and, less, at the
/// transom, so it rides up over a rock or a ledge of the bed it scrapes across instead of catching on it.
const RAFT_PROFILE: &[[f32; 2]] = &[[-1.8, 0.25], [1.8, 0.25], [1.8, -0.1], [1.4, -0.25], [-1.2, -0.25], [-1.8, 0.05]];

/// Its rubber takes a knock against a rock or a bank lightly, it rams nothing, and it starts floating at
/// its draft.
const RAFT_BODY: Body = Body { drop_y: RAFT_HALF_EXTENTS[1] - RAFT_DRAFT, ram: 0.3, crash: 0.4, belly: 8.0, ..CAR };

/// Its flat floor and outboard.
const RAFT_HULL: Boat = Boat {
    draft: RAFT_DRAFT,
    thrust: RAFT_THRUST,
    brake: RAFT_BRAKE,
    brake_dip: RAFT_BRAKE_DIP,
    brake_squat: RAFT_BRAKE_SQUAT,
    astern_speed: RAFT_ASTERN_SPEED,
    spin_rate: RAFT_SPIN_RATE,
    plane_lift: RAFT_PLANE_LIFT,
    plane_speed: RAFT_PLANE_SPEED,
    bow_rise: RAFT_BOW_RISE,
    bow_planing: RAFT_BOW_PLANING,
    bank: RAFT_BANK,
    bank_max: RAFT_BANK_MAX,
    heave: RAFT_HEAVE,
    coast: RAFT_COAST,
    level: RAFT_LEVEL,
    rock: RAFT_ROCK,
    keel: RAFT_KEEL,
    grip: RAFT_GRIP,
    helm: RAFT_HELM,
    thrust_steer: RAFT_THRUST_STEER,
    lever: RAFT_LEVER,
    scrape: RAFT_SCRAPE,
};

pub(super) const RAFT: VehicleDef = VehicleDef {
    kind: KIND_RAFT,
    name: "Raft",
    hp: 60,
    seats: RAFT_SEATS,
    mounts: &[],
    car_wheels: false,
    crew: Exposure::Open,
    seat_heads: &RAFT_SEAT_HEADS,
    crew_guns: true,
    body: RAFT_BODY,
    spec: Spec {
        half_extents: RAFT_HALF_EXTENTS,
        // 2.9 m³ of box is about 200 kg: the raft, its motor and a full tank.
        density: 70.0,
        wheel_radius: 0.0,
        track_half: 0.0,
        axles: &[],
        hard_point_y: 0.0,
        suspension_rest: 0.0,
        max_suspension_force: 0.0,
        engine_force: 0.0,
        drive: Drive::All,
        max_speed: RAFT_MAX_SPEED,
        steer_lock: 0.0,
        steer_axles: Spec::FRONT_STEER,
        turn_rate: RAFT_TURN_RATE,
        lift: Lift::Boat(&RAFT_HULL),
        linear_damping: 0.0,
        // Its weight is its floor and the motor low on the transom: it sits flat.
        com_drop: 0.15,
        tank: Some(BOOST_FULL),
        fuel: Spec::FUEL,
        buttons: Buttons::Boost,
        boost: Some(Boost::BOAT),
        aims: false,
        solids: &[],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        sweep: Spec::FLAT,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        skim: None,
        swim: None,
        // Its outline sweeps its floor up at both ends instead (`RAFT_PROFILE`).
        bow: 0.0,
        profile: RAFT_PROFILE,
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
    doors: NO_DOORS,
};
