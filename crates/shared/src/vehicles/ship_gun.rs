//! A pirate ship's gun: everything the game knows about it.

use super::*;

/// A ship's gun (`KIND_SHIP_GUN`): a long gun on a wooden truck carriage, fixed to its ship's deck
/// (`Fitting`, `SHIP_GUN_SLOTS`) and carried with it, so nothing pushes or drives it. One gunner lays it
/// by the look, a little either way along the broadside and up and down, and fires round shot
/// (`PROJECTILE_BALL`): slower than a shell, so it arcs, and bursting small where it lands. Its
/// muzzle points down its own -Z, over the side.
pub const SHIP_GUN_SEATS: usize = 1;
pub const SHIP_GUN_HALF_EXTENTS: [f32; 3] = [0.3, 0.3, 0.5];
/// The trunnions, over the carriage's front, and the barrel's length from them.
pub const SHIP_GUN_PIVOT: [f32; 3] = [0.0, 0.25, -0.12];
pub const SHIP_GUN_BARREL_LENGTH: f32 = 1.4;
/// Traverse either side of square to the ship, and elevation, radians.
pub const SHIP_GUN_TRAVERSE: f32 = 0.5;
pub const SHIP_GUN_PITCH_MIN: f32 = -0.12;
pub const SHIP_GUN_PITCH_MAX: f32 = 0.35;
/// How fast it is laid onto the look, rad/s: handspikes and tackle, slow.
pub const SHIP_GUN_SLEW: f32 = 0.8;
/// Ticks between shots: swabbed, loaded and run out again.
pub const SHIP_GUN_COOLDOWN_TICKS: u32 = 4 * TICK_RATE;
/// How hard it kicks back, m/s, which its ship takes at the gun: a broadside rolls it.
pub const SHIP_GUN_RECOIL: f32 = 1.5;
/// The round shot's muzzle speed, m/s.
pub const BALL_SPEED: f32 = 90.0;
/// Its small burst where it lands, beside the harm of the iron itself to whatever it goes through
/// (radius metres, damage, knock m/s), and the hole it knocks in the ground (radius, depth, metres).
pub const BALL_BLAST_RADIUS: f32 = 2.5;
pub const BALL_BLAST_DAMAGE: f32 = 55.0;
pub const BALL_BLAST_KNOCK: f32 = 7.0;
pub const CRATER_BALL_RADIUS: f32 = 1.3;
pub const CRATER_BALL_DEPTH: f32 = 0.3;
/// The gunner stands behind the breech, from the pivot, with the gun pointing ahead.
pub const SHIP_GUN_SEAT_HEAD: [f32; 3] = [0.25, 0.5, 0.85];
/// Where the gun is manned from, behind its breech, and where its gunner steps back onto the deck.
pub const SHIP_GUN_BOARD: [f32; 3] = [0.0, 0.2, 0.6];
pub const SHIP_GUN_EXIT: [f32; 3] = [0.0, STAND_CENTRE + 0.05 - SHIP_GUN_HALF_EXTENTS[1], SHIP_GUN_HALF_EXTENTS[2] + CAPSULE_RADIUS + 0.3];

/// Oak and iron: a bullet stops in it. The breech can be smashed.
const ARMOR: Armor = Armor {
    plates: &[Plate::hull("carriage", Zone::Front, 6.0).all_round().glances(80.0)],
    modules: &[Module::new(Part::Breech, [0.0, 0.33, 0.2], [0.1, 0.1, 0.2], 60)],
    canvas: false,
};

pub(super) const SHIP_GUN: VehicleDef = VehicleDef {
    kind: KIND_SHIP_GUN,
    name: "Ship's gun",
    hp: 100,
    seats: SHIP_GUN_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Open,
    seat_heads: &[],
    crew_guns: false,
    body: Body { drop_y: 0.0, ..CAR },
    spec: Spec {
        half_extents: SHIP_GUN_HALF_EXTENTS,
        // Its ship carries it: what it weighs, about 400 kg, only matters to what its recoil does to the
        // ship, which one gun rolls by a couple of degrees and a whole broadside by several.
        density: 1700.0,
        wheel_radius: 0.0,
        track_half: 0.0,
        axles: &[],
        hard_point_y: 0.0,
        suspension_rest: 0.0,
        max_suspension_force: 0.0,
        engine_force: 0.0,
        max_speed: 0.0,
        steer_lock: 0.0,
        turn_rate: 0.0,
        lift: Lift::Mounted,
        linear_damping: 0.0,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        // The barrel and breech over the carriage, a little taller than they are: a soldier's capsule
        // rides up and over anything lower than about 0.7 m, which would make the gun a stair over the side.
        solids: &[Solid { shape: Shape::Cuboid([0.18, 0.25, 0.45]), at: [0.0, 0.3, 0.05], role: Role::Part }],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        skim: None,
        bow: 0.0,
        springs: Springs::FIRM,
        airframe: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: Some(MountSeat { seat: 0, pivot: SHIP_GUN_PIVOT, head: SHIP_GUN_SEAT_HEAD }),
    board: Some(SHIP_GUN_BOARD),
    exit: Some(SHIP_GUN_EXIT),
    fittings: &[],
};

/// The gun, laid by the look within its port.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: 0,
    aim: Aim::Mount { yaw_limit: Some(SHIP_GUN_TRAVERSE), pitch: (SHIP_GUN_PITCH_MIN, SHIP_GUN_PITCH_MAX), slew: SHIP_GUN_SLEW },
    muzzle: Muzzle::One(SHIP_GUN_PIVOT),
    fire: Fire::Shell { kind: PROJECTILE_BALL, alt: PROJECTILE_BALL, speed: BALL_SPEED, length: SHIP_GUN_BARREL_LENGTH, ttl: SHELL_TTL_TICKS },
    trigger: BUTTON_FIRE,
    cooldown: SHIP_GUN_COOLDOWN_TICKS,
    spread: 0.0,
    recoil: SHIP_GUN_RECOIL,
    heat: 0.0,
    ammo: None,
    needs: &[Need::Breech],
    show: Show::Turret,
    eye: Some(Eye { back: GUNNER_BACK, up: GUNNER_UP }),
    sight: true,
}];
