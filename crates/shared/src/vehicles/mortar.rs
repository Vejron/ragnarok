//! The field mortar: everything the game knows about it.

use super::*;

/// The field mortar (`KIND_MORTAR`): a towed, unpowered two-wheeler with one gunner, who aims its bearing
/// with the look and its range with the dial. It is towed by a car's rear tow point through a spherical joint.
pub const MORTAR_SEATS: usize = 1;
pub const MORTAR_HALF_EXTENTS: [f32; 3] = [1.0, 0.5, 1.5];
pub const MORTAR_WHEEL_RADIUS: f32 = 0.45;
pub const MORTAR_TRACK_HALF: f32 = 0.85;
/// Where the mount sits in the hull frame, and the barrel's length from it.
pub const MORTAR_GUN_PIVOT: [f32; 3] = [0.0, 0.7, -0.2];
pub const MORTAR_BARREL_LENGTH: f32 = 3.2;
/// Elevation travel, radians: a high lob at the bottom, nearly vertical at the top. The gunner never sets
/// it by hand: the range dial does (`lob_pitch` in gunnery).
pub const MORTAR_PITCH_MIN: f32 = 0.785;
pub const MORTAR_PITCH_MAX: f32 = 1.484;
/// The range dial, metres: W and S walk it at `MORTAR_RANGE_RATE` while `BUTTON_RANGE_UP` /
/// `BUTTON_RANGE_DOWN` are held. The longest lob is 245 m (`GRAVITY` and the muzzle speed at 45 degrees),
/// the shortest the high lob reaches is about 42 m, so the dial stops at 50.
pub const MORTAR_RANGE_START: f32 = 120.0;
pub const MORTAR_RANGE_MIN: f32 = 50.0;
pub const MORTAR_RANGE_MAX: f32 = 245.0;
pub const MORTAR_RANGE_RATE: f32 = 60.0;
/// How fast the mount follows the gunner's look, rad/s. Quick enough that the barrel stays on the crosshair.
pub const MORTAR_TRAVERSE_SLEW: f32 = 3.0;
/// Muzzle speed of a mortar bomb, m/s. Under `GRAVITY` it reaches about 245 m at 45 degrees and 40 m at 85.
pub const MORTAR_SHELL_SPEED: f32 = 70.0;
/// A reload is long: the gunner waits between bombs.
pub const MORTAR_COOLDOWN_TICKS: u32 = 7 * TICK_RATE;
pub const MORTAR_SHELL_TTL_TICKS: u32 = 10 * TICK_RATE;
/// Speed the mortar is shoved back at when it fires, m/s, along the barrel's horizontal heading.
pub const MORTAR_RECOIL: f32 = 3.0;
/// Blast of a mortar bomb: wider and harder than a tank shell.
pub const MORTAR_RADIUS: f32 = 11.0;
pub const MORTAR_DAMAGE: f32 = 260.0;
pub const MORTAR_KNOCK: f32 = 30.0;
/// The mortar's gunner rides its mount, so this is in the mount's frame: `MORTAR_GUN_PIVOT` turned by the gun's yaw.
pub const MORTAR_SEAT_HEAD: [f32; 3] = [0.6, 0.45, 0.6];

/// The field mortar: a towed gun on two wheels. Nothing drives it, so it has no engine; its wheels only
/// roll it, and the brake holds it once it is unhitched.
pub(super) const MORTAR: VehicleDef = VehicleDef {
    kind: KIND_MORTAR,
    name: "Field mortar",
    hp: 150,
    seats: MORTAR_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Open,
    seat_heads: &[],
    crew_guns: false,
    body: TOWED,
    spec: Spec {
        half_extents: MORTAR_HALF_EXTENTS,
        // 6 m³ of steel, about 900 kg: a jeep can pull it, a sports car struggles.
        density: 150.0,
        wheel_radius: MORTAR_WHEEL_RADIUS,
        track_half: MORTAR_TRACK_HALF,
        // Two axles, one fore and one aft, so it stands on four wheels and does not teeter on a pair at its middle.
        axles: &[-0.9, 0.9],
        hard_point_y: -0.25,
        suspension_rest: 0.3,
        max_suspension_force: 30_000.0,
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
        solids: &[],
        tongue: -(MORTAR_HALF_EXTENTS[2] + TOW_BACK),
    },
    armor: ALLOY,
    drone: None,
    mount_seat: Some(MountSeat { seat: 0, pivot: MORTAR_GUN_PIVOT, head: MORTAR_SEAT_HEAD }),
};

/// The tube, which a towed mortar cannot fire: it is being pulled.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: 0,
    aim: Aim::Lob { slew: MORTAR_TRAVERSE_SLEW },
    muzzle: Muzzle::One(MORTAR_GUN_PIVOT),
    fire: Fire::Shell { kind: PROJECTILE_MORTAR, alt: PROJECTILE_MORTAR, speed: MORTAR_SHELL_SPEED, length: MORTAR_BARREL_LENGTH, ttl: MORTAR_SHELL_TTL_TICKS },
    trigger: BUTTON_FIRE,
    cooldown: MORTAR_COOLDOWN_TICKS,
    spread: 0.0,
    recoil: MORTAR_RECOIL,
    heat: 0.0,
    ammo: None,
    needs: &[Need::Untowed],
    show: Show::Turret,
}];
