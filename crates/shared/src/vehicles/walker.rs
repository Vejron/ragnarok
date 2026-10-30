//! The walker: everything the game knows about it.

use super::*;

/// The walker (`KIND_WALKER`): a two-legged assault walker, one driver in the head. Its body is a single
/// box held up by two spring legs that are ray casts to the ground, and kept upright by a torque that a
/// big enough blow overpowers. Mirrors `WALKER_*` in `sim/consts.ts`.
pub const WALKER_SEATS: usize = 1;
pub const WALKER_HALF_EXTENTS: [f32; 3] = [1.0, 1.2, 1.8];
/// Distance of each foot from the middle of the hull, sideways, metres, and how long a leg is from the
/// bottom of the hull to the ground when it stands.
pub const WALKER_FOOT_HALF: f32 = 1.7;
pub const WALKER_LEG_LENGTH: f32 = 4.2;
/// The legs are solid, so a car or a shell can hit them: a box on each hip, this wide and deep (half
/// extents, metres), ending this far above the ground when it stands so the feet's own springs carry it.
pub const WALKER_LEG_HALF_WIDTH: f32 = 0.25;
pub const WALKER_LEG_HALF_DEPTH: f32 = 0.3;
pub const WALKER_LEG_CLEARANCE: f32 = 0.5;
/// Top speed forwards, backwards (a share of it), m/s, and the acceleration towards it, m/s².
pub const WALKER_MAX_SPEED: f32 = 9.0;
pub const WALKER_REVERSE_SHARE: f32 = 0.5;
pub const WALKER_ACCEL: f32 = 4.5;
/// Deceleration, m/s², when it slows or reverses: it plants its feet and leans back.
pub const WALKER_BRAKE: f32 = 6.0;
/// Planted feet do not slide sideways: the push against it, per m/s of sideways speed, and its cap, m/s².
pub const WALKER_GRIP_GAIN: f32 = 8.0;
pub const WALKER_GRIP: f32 = 8.0;
/// Yaw rate at full stick, rad/s, and the angular acceleration towards it, rad/s².
pub const WALKER_TURN_RATE: f32 = 1.7;
pub const WALKER_TURN_ACCEL: f32 = 8.0;
/// Leg spring and damper, per kilogram of the whole body for the pair: the spring settles at
/// `CAR_GRAVITY / WALKER_LEG_SPRING` of compression.
pub const WALKER_LEG_SPRING: f32 = 80.0;
pub const WALKER_LEG_DAMPING: f32 = 14.0;
/// How hard the hull is pulled upright, rad/s² per radian of tilt, and the damping on its tilt rate, per
/// second. The pull never exceeds `WALKER_BALANCE_MAX` rad/s²: a blow that tips it further than that
/// can undo goes over.
pub const WALKER_BALANCE_GAIN: f32 = 10.0;
pub const WALKER_BALANCE_DAMP: f32 = 4.0;
pub const WALKER_BALANCE_MAX: f32 = 3.0;
/// Pitching (forwards and backwards) is the axis a walker can fight best: its feet are in line with the
/// tilt, so the pull is up to this, and it also takes a catching step. The step is a push towards where
/// the top is falling, by `WALKER_CATCH_GAIN` m/s² per unit of lean from where it should be and
/// `WALKER_CATCH_DAMP` per rad/s of fall, at most `WALKER_CATCH_MAX`.
pub const WALKER_BALANCE_MAX_PITCH: f32 = 6.0;
pub const WALKER_CATCH_GAIN: f32 = 20.0;
pub const WALKER_CATCH_DAMP: f32 = 3.0;
pub const WALKER_CATCH_MAX: f32 = 8.0;
/// Lean (up's horizontal part along the heading beyond its walking lean) and fall rate (rad/s) it
/// tolerates before it takes a step, so a walk's own lean and its pushes are left alone.
pub const WALKER_CATCH_LEAN_MARGIN: f32 = 0.15;
pub const WALKER_CATCH_FALL_MARGIN: f32 = 0.5;
/// Extra lean into its heading at top speed, as a tilt: the walker walks a little forward of its feet.
pub const WALKER_CRUISE_LEAN: f32 = 0.08;
/// How much of the tilt that would match its push it leans by: the push goes through the middle of
/// the hull, so the lean is for show and for how it handles.
pub const WALKER_ACCEL_LEAN: f32 = 0.5;
/// A blast pushes a walker at its shoulders, this far above the middle of the hull, so it tips it.
pub const WALKER_BLAST_LEVER: f32 = 3.0;
/// Below this `up_y` it has fallen: the legs no longer push or drive, and only a weak pull
/// (`WALKER_GETUP_SHARE` of the full one) stands it up again.
pub const WALKER_FALLEN_UP: f32 = 0.6;
pub const WALKER_GETUP_SHARE: f32 = 0.25;
/// The chin guns: the driver aims them with the look, and the upper body turns to point them, up to
/// `WALKER_TORSO_LIMIT` either side of the way the legs face (and `WALKER_GUN_PITCH_MIN..=_MAX` up and
/// down, radians, hull frame), at `WALKER_GUN_SLEW` rad/s. `WALKER_GUN_PIVOT` is where the upper body turns
/// in the hull frame and `WALKER_GUN_LENGTH` how far the muzzle is from it. A round is a cannon shell, as
/// in a helicopter's nose gun, fired every `WALKER_GUN_COOLDOWN_TICKS` and scattered by up to `WALKER_GUN_SPREAD`.
pub const WALKER_TORSO_LIMIT: f32 = 0.52;
pub const WALKER_GUN_PITCH_MIN: f32 = -0.5;
pub const WALKER_GUN_PITCH_MAX: f32 = 0.4;
pub const WALKER_GUN_SLEW: f32 = 2.5;
pub const WALKER_GUN_PIVOT: [f32; 3] = [0.0, 0.1, 0.0];
pub const WALKER_GUN_LENGTH: f32 = 3.7;
/// The two barrels hang `WALKER_GUN_BARREL_DROP` below the pivot, `WALKER_GUN_BARREL_X` either side of
/// it, and fire in turn.
pub const WALKER_GUN_BARREL_X: f32 = 0.3;
pub const WALKER_GUN_BARREL_DROP: f32 = 1.0;
pub const WALKER_GUN_COOLDOWN_TICKS: u32 = TICK_RATE / 4;
pub const WALKER_GUN_SPREAD: f32 = 0.01;

/// A walker's two legs under a hull `hy` high. They hang from the bottom of the hull and stop short of
/// the ground, so a car or a shell hits them but the springs, not the boxes, carry the walker.
/// Its pilot sealed in the cab up front, and the fuel at the back.
const ARMOR: Armor = Armor {
    plates: ALLOY.plates,
    modules: &[
        Module::crew(0, [0.0, 0.4, -0.9], [0.4, 0.45, 0.45]),
        Module::new(Part::Fuel, [0.0, -0.3, 1.0], [0.5, 0.3, 0.4], 100),
    ],
    canvas: false,
};

pub(super) const fn legs(hy: f32) -> [Solid; 2] {
    let half = (WALKER_LEG_LENGTH - WALKER_LEG_CLEARANCE) / 2.0;
    let leg = Shape::Cuboid([WALKER_LEG_HALF_WIDTH, half, WALKER_LEG_HALF_DEPTH]);
    [
        Solid { shape: leg, at: [-WALKER_FOOT_HALF, -hy - half, 0.0], role: Role::Part },
        Solid { shape: leg, at: [WALKER_FOOT_HALF, -hy - half, 0.0], role: Role::Part },
    ]
}

const LEG_SOLIDS: [Solid; 2] = legs(WALKER_HALF_EXTENTS[1]);

pub(super) const WALKER: VehicleDef = VehicleDef {
    kind: KIND_WALKER,
    name: "Walker",
    hp: 200,
    seats: WALKER_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: LEGS,
    spec: Spec {
        half_extents: WALKER_HALF_EXTENTS,
        // 22.5 m³ of hull at this density is about 5.6 tonnes. Nothing below uses the wheel fields; the legs
        // are the `WALKER_*` constants.
        density: 250.0,
        wheel_radius: 0.0,
        track_half: 0.0,
        axles: &[],
        hard_point_y: 0.0,
        suspension_rest: 0.0,
        max_suspension_force: 0.0,
        engine_force: 0.0,
        max_speed: WALKER_MAX_SPEED,
        steer_lock: 0.0,
        turn_rate: WALKER_TURN_RATE,
        lift: Lift::Walker,
        linear_damping: 0.05,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &LEG_SOLIDS,
        tongue: 0.0,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
};

/// The driver's chin guns: the upper body turns within `WALKER_TORSO_LIMIT` of the legs.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: 0,
    aim: Aim::Mount { yaw_limit: Some(WALKER_TORSO_LIMIT), pitch: (WALKER_GUN_PITCH_MIN, WALKER_GUN_PITCH_MAX), slew: WALKER_GUN_SLEW },
    muzzle: Muzzle::Twin { pivot: WALKER_GUN_PIVOT, x: WALKER_GUN_BARREL_X, drop: WALKER_GUN_BARREL_DROP },
    fire: Fire::Shell { kind: PROJECTILE_CANNON, alt: PROJECTILE_CANNON, speed: SHELL_SPEED, length: WALKER_GUN_LENGTH, ttl: SHELL_TTL_TICKS },
    trigger: BUTTON_FIRE,
    cooldown: WALKER_GUN_COOLDOWN_TICKS,
    spread: WALKER_GUN_SPREAD,
    recoil: 0.0,
    heat: 0.0,
    ammo: None,
    needs: &[Need::Upright],
    show: Show::Turret,
}];
