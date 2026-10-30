//! The helicopter: everything the game knows about it.

use super::*;

/// The helicopter: a hull flown by a pilot (seat 0) with a gunner (seat 1) on the nose gun. There are no
/// wheels; the rotor is a thrust along the hull's up axis, and the pilot leans the hull to go
/// anywhere. Same body axes as the car.
pub const HELI_SEATS: usize = 2;
pub const HELI_HALF_EXTENTS: [f32; 3] = [1.0, 0.8, 2.4];
/// How far the rotor disc leans at full stick: tangent of the angle, forward and sideways.
pub const HELI_TILT: f32 = 0.5;
pub const HELI_BANK: f32 = 0.35;
/// Yaw rate at full stick, rad/s.
pub const HELI_YAW_RATE: f32 = 1.4;
/// Vertical speed the climb input asks for and the descent input asks for, m/s. With neither
/// held the rotor holds altitude, whatever speed it had. `HELI_VERT_GAIN` (per second) turns the
/// speed error into an acceleration, which never exceeds `HELI_VERT_ACCEL`, m/s².
pub const HELI_CLIMB_SPEED: f32 = 6.0;
pub const HELI_SINK_SPEED: f32 = 4.0;
pub const HELI_VERT_GAIN: f32 = 1.5;
pub const HELI_VERT_ACCEL: f32 = 5.0;
/// Climbing fades out over the last 20 m below this height above the terrain, and above it the rotor
/// pushes down. Every flier's ceiling is measured from the terrain under it, not from y = 0.
pub const HELI_CEILING: f32 = 120.0;
/// How firmly the hull levels out (rad/s of correction per unit of lean error), and how fast the
/// angular velocity follows it, per second.
pub const HELI_LEVEL_GAIN: f32 = 4.0;
pub const HELI_RESPONSE: f32 = 8.0;
/// The nose gun: a four-barrel autocannon on a chin mount. `HELI_GUN_PIVOT` is the mount in the hull
/// frame and `HELI_GUN_LENGTH` how far the muzzle is from it. The gunner's look direction aims it, within
/// `HELI_GUN_YAW_LIMIT` either side of the nose and `HELI_GUN_PITCH_MIN..=HELI_GUN_PITCH_MAX` (radians, the
/// hull's frame), at `HELI_GUN_SLEW` rad/s. It fires a shell (`SHELL_SPEED`, same fall and lifetime) every
/// `HELI_GUN_COOLDOWN_TICKS`, three a second, scattered by up to `HELI_GUN_SPREAD` radians. It bursts
/// like a tank shell on a smaller scale: a third of the radius and a third of the damage.
pub const HELI_GUN_PIVOT: [f32; 3] = [0.0, -0.6, -2.0];
pub const HELI_GUN_LENGTH: f32 = 1.3;
pub const HELI_GUN_YAW_LIMIT: f32 = 1.0;
pub const HELI_GUN_PITCH_MIN: f32 = -0.75;
pub const HELI_GUN_PITCH_MAX: f32 = 0.2;
pub const HELI_GUN_SLEW: f32 = 3.0;
pub const HELI_GUN_COOLDOWN_TICKS: u32 = TICK_RATE / 3;
pub const HELI_GUN_SPREAD: f32 = 0.01;
/// The rocket pods under the stub wings, fired by the pilot (seat 0) on the fire button. `HELI_POD_PIVOTS`
/// are the four tube mouths in the hull frame, used in turn, one rocket every `HELI_ROCKET_COOLDOWN_TICKS`.
/// A rocket leaves at `HELI_ROCKET_LAUNCH_SPEED` over the helicopter's own velocity, along the nose, and
/// is driven at `ROCKET_ACCEL` m/s² up to `ROCKET_MAX_SPEED`: it flies straight, no gravity.
pub const HELI_POD_PIVOTS: [[f32; 3]; 4] = [[-1.0, -0.39, -0.3], [1.0, -0.39, -0.3], [-1.75, -0.525, -0.3], [1.75, -0.525, -0.3]];
pub const HELI_ROCKET_COOLDOWN_TICKS: u32 = TICK_RATE / 5;
pub const HELI_ROCKET_LAUNCH_SPEED: f32 = 20.0;
/// The pilot raised behind and above the gunner in the nose.
pub const HELI_SEAT_HEADS: [[f32; 3]; 2] = [[0.0, 0.45, -0.45], [0.0, 0.1, -1.55]];

/// The turbine over the cabin, which turns the rotor, and the fuel under its floor; the crew behind the glass are hit as anyone is.
const ARMOR: Armor = Armor {
    plates: ALLOY.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.55, 0.5], [0.4, 0.22, 0.6], 110),
        Module::new(Part::Fuel, [0.0, -0.45, 0.5], [0.5, 0.2, 0.5], 90),
    ],
    canvas: false,
};

pub(super) const HELI: VehicleDef = VehicleDef {
    kind: KIND_HELI,
    name: "Helicopter",
    hp: 100,
    seats: HELI_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Glazed,
    seat_heads: &HELI_SEAT_HEADS,
    crew_guns: false,
    body: ROTOR,
    spec: Spec {
        half_extents: HELI_HALF_EXTENTS,
        // 3.8 m³ of hull is about 1.1 tonnes. Nothing below uses the wheel fields.
        density: 300.0,
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
        lift: Lift::Rotor,
        // Air drag is what limits the speed: the rotor's lean gives about 4.9 m/s², so about 25 m/s.
        linear_damping: 0.2,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[],
        tongue: 0.0,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
};

/// The gunner's nose autocannon, whose rounds are shells in miniature, and the pilot's rocket pods.
const MOUNTS: &[MountDef] = &[
    MountDef {
        seat: 1,
        aim: Aim::Mount { yaw_limit: Some(HELI_GUN_YAW_LIMIT), pitch: (HELI_GUN_PITCH_MIN, HELI_GUN_PITCH_MAX), slew: HELI_GUN_SLEW },
        muzzle: Muzzle::One(HELI_GUN_PIVOT),
        fire: Fire::Shell { kind: PROJECTILE_CANNON, alt: PROJECTILE_CANNON, speed: SHELL_SPEED, length: HELI_GUN_LENGTH, ttl: SHELL_TTL_TICKS },
        trigger: BUTTON_FIRE,
        cooldown: HELI_GUN_COOLDOWN_TICKS,
        spread: HELI_GUN_SPREAD,
        recoil: 0.0,
        heat: 0.0,
        ammo: None,
        needs: &[],
        show: Show::Turret,
    },
    MountDef {
        seat: 0,
        aim: Aim::Fixed,
        muzzle: Muzzle::Turns(&HELI_POD_PIVOTS),
        fire: Fire::Rocket { speed: HELI_ROCKET_LAUNCH_SPEED, ttl: ROCKET_TTL_TICKS },
        trigger: BUTTON_FIRE,
        cooldown: HELI_ROCKET_COOLDOWN_TICKS,
        spread: 0.0,
        recoil: 0.0,
        heat: 0.0,
        ammo: None,
        needs: &[],
        show: Show::Hidden,
    },
];
