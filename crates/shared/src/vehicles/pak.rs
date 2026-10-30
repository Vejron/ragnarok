//! The Pak 38: everything the game knows about it.

use super::*;

/// The Pak 38 (`KIND_PAK`): a towed 5 cm anti-tank gun on two wheels with split trails and a shield,
/// one gunner. Its muzzle points down the hull's -Z, the trails run back to +Z, and it is towed by the
/// trails' eye (`PAK_TONGUE_Z`), so it travels muzzle-last. It fires the tank's shell, laid by the look
/// within its traverse and elevation, as a hull-mounted gun (`Aim::Mount`).
pub const PAK_SEATS: usize = 1;
pub const PAK_HALF_EXTENTS: [f32; 3] = [0.8, 0.35, 1.4];
pub const PAK_WHEEL_RADIUS: f32 = 0.4;
pub const PAK_TRACK_HALF: f32 = 0.8;
/// The road wheels' axle, and the trails' spades it rests on at the back (the physics' second axle).
pub const PAK_AXLES_Z: [f32; 2] = [-0.7, 1.25];
/// The towing eye where the closed trails meet, behind the hull box.
pub const PAK_TONGUE_Z: f32 = PAK_HALF_EXTENTS[2] + TOW_BACK;
/// The trunnions, over the axle, and the barrel's length from them.
pub const PAK_GUN_PIVOT: [f32; 3] = [0.0, 0.25, -0.7];
pub const PAK_BARREL_LENGTH: f32 = 3.0;
/// Traverse either side of the trails' centre line, and elevation, radians: about 65 degrees in all,
/// and -8 to +27 degrees.
pub const PAK_TRAVERSE: f32 = 0.57;
pub const PAK_PITCH_MIN: f32 = -0.14;
pub const PAK_PITCH_MAX: f32 = 0.47;
/// How fast the gun is laid onto the look, rad/s: hand wheels, a little quicker than a tank's turret.
pub const PAK_SLEW: f32 = 1.2;
/// Speed it is shoved back at when it fires, m/s: its spades are dug in.
pub const PAK_RECOIL: f32 = 1.0;
/// The shield across the front, in the hull frame: it stops what would hit the gunner from ahead.
pub const PAK_SHIELD_HALF: [f32; 3] = [0.85, 0.4, 0.03];
pub const PAK_SHIELD_CENTRE: [f32; 3] = [0.0, 0.15, -0.95];
/// The Pak 38's gunner kneels behind the shield, left of the breech, at the sight: in the mount's frame.
pub const PAK_SEAT_HEAD: [f32; 3] = [-0.4, 0.2, 0.6];

/// The shield stops a rifle bullet and turns a shot that comes in at a slant, but a gap for the sight is
/// left in it; the trails and the breech behind it are bare.
const ARMOR: Armor = Armor {
    plates: &[
        Plate { face: None, ..Plate::slit("sight gap", Zone::Front, Box3::new([0.25, 0.3, -0.95], [0.05, 0.04, 0.06])).on(0) },
        Plate::hull("shield", Zone::Front, 8.0).on(0).all_round().sloped(11.0).glances(70.0),
        Plate::hull("carriage", Zone::Front, 3.0).all_round().glances(82.0),
    ],
    // Its crew are in the open, hit as anyone is; only the breech behind the shield is a part.
    modules: &[Module::new(Part::Breech, [0.0, 0.25, -0.4], [0.12, 0.12, 0.25], 60)],
    canvas: false,
};

/// The Pak 38: a towed anti-tank gun. Nothing drives it, as with the mortar, but its gunner lays it like a
/// tank's gun: the look trains and elevates it within its carriage's travel, and it fires the tank's shell.
pub(super) const PAK: VehicleDef = VehicleDef {
    kind: KIND_PAK,
    name: "Pak 38",
    hp: 140,
    seats: PAK_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Open,
    seat_heads: &[],
    crew_guns: false,
    body: TOWED,
    spec: Spec {
        half_extents: PAK_HALF_EXTENTS,
        // 3.1 m³ of box, about 1000 kg: a jeep pulls it, as it does the mortar.
        density: 320.0,
        wheel_radius: PAK_WHEEL_RADIUS,
        track_half: PAK_TRACK_HALF,
        // The road wheels, and the trails' spades behind: it stands on both, as the real gun does.
        axles: &PAK_AXLES_Z,
        hard_point_y: -0.1,
        suspension_rest: 0.2,
        max_suspension_force: 35_000.0,
        engine_force: 0.0,
        max_speed: 1.0,
        steer_lock: 0.0,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[Solid { shape: Shape::Cuboid(PAK_SHIELD_HALF), at: PAK_SHIELD_CENTRE, role: Role::Part }],
        tongue: PAK_TONGUE_Z,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: Some(MountSeat { pivot: PAK_GUN_PIVOT, head: PAK_SEAT_HEAD }),
};

/// The gun, laid by the look within its traverse, which a towed Pak cannot fire.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: 0,
    aim: Aim::Mount { yaw_limit: Some(PAK_TRAVERSE), pitch: (PAK_PITCH_MIN, PAK_PITCH_MAX), slew: PAK_SLEW },
    muzzle: Muzzle::One(PAK_GUN_PIVOT),
    fire: Fire::Shell { kind: PROJECTILE_SHELL, alt: PROJECTILE_HE, speed: SHELL_SPEED, length: PAK_BARREL_LENGTH, ttl: SHELL_TTL_TICKS },
    trigger: BUTTON_FIRE,
    cooldown: SHELL_COOLDOWN_TICKS,
    spread: 0.0,
    recoil: PAK_RECOIL,
    heat: 0.0,
    ammo: None,
    needs: &[Need::Untowed, Need::Breech],
    show: Show::Turret,
}];
