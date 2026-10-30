//! The loitering drone: everything the game knows about it.

use super::*;

/// The loitering drone: a delta wing about 2.5 m across and 3.5 m long, with a pusher prop. It flies as
/// a simplified, nimble plane: always at `LOITER_SPEED` along its nose, with no throttle. The wings and
/// motor close any gap between its velocity and that at `LOITER_GRIP` per second, never harder than
/// `LOITER_MAX_ACCEL` m/s², so it goes where its nose points, and they hold it up whatever its attitude.
/// The nose is steered as a plane's is: the mouse sets where to go and it banks into the turn and pulls
/// (see `autopilot`), and W/S pitch and A/D roll by hand while held. It turns at most `LOITER_PITCH_RATE`,
/// `LOITER_YAW_RATE` and `LOITER_ROLL_RATE` rad/s about its own axes; its angular velocity follows the
/// controls at `LOITER_RESPONSE` per second.
pub const LOITER_SEATS: usize = 1;
pub const LOITER_HALF_EXTENTS: [f32; 3] = [1.25, 0.25, 1.75];
pub const LOITER_SPEED: f32 = 35.0;
pub const LOITER_GRIP: f32 = 10.0;
pub const LOITER_MAX_ACCEL: f32 = 80.0;
pub const LOITER_PITCH_RATE: f32 = 2.0;
pub const LOITER_ROLL_RATE: f32 = 4.0;
pub const LOITER_YAW_RATE: f32 = 0.8;
pub const LOITER_RESPONSE: f32 = 8.0;
/// The most the autopilot banks it into a turn, radians (about 57 degrees): less than the plane, so a
/// hard reversal stays a turn rather than a roll on to its back.
pub const LOITER_MAX_BANK: f32 = 1.0;
/// Climbing fades out over the last 20 m below this height, and above it it is pushed down.
pub const LOITER_CEILING: f32 = 500.0;
/// Air drag on the body, which is all that slows one whose motor is off.
pub const LOITER_DAMPING: f32 = 0.05;

/// It cannot hover, so it starts out far up and back, already cruising; shot down, it pops about as
/// big as a tank shell's burst.
const LOITERING: DroneDef = DroneDef {
    energy_ticks: LOITER_ENERGY_TICKS,
    range: LOITER_RANGE,
    blast_radius: LOITER_BLAST_RADIUS,
    damage: LOITER_DAMAGE,
    knock: LOITER_KNOCK,
    pop_radius: LOITER_POP_RADIUS,
    pop_damage: LOITER_POP_DAMAGE,
    impact_dv: LOITER_IMPACT_DV,
    proximity: LOITER_PROXIMITY,
    fall_ticks: LOITER_FALL_TICKS,
    weapon: WEAPON_LOITER,
    pop_shown_as: WEAPON_SHELL,
    launch: Launch::Behind { back: LOITER_LAUNCH_BACK, up: LOITER_LAUNCH_UP, speed: LOITER_SPEED },
};

/// Takes a few rifle rounds to bring down.
pub(super) const LOITER: VehicleDef = VehicleDef {
    kind: KIND_LOITER,
    name: "Loitering drone",
    hp: 40,
    seats: LOITER_SEATS,
    mounts: &[],
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: ROTOR,
    spec: Spec {
        half_extents: LOITER_HALF_EXTENTS,
        // A box 2.5 x 0.5 x 3.5 m around the wing, about 200 kg. Nothing below uses the wheel fields.
        density: 45.0,
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
        lift: Lift::Loiter,
        linear_damping: LOITER_DAMPING,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: true,
        solids: &[],
        tongue: 0.0,
    },
    armor: SHEET,
    drone: Some(LOITERING),
    mount_seat: None,
};
