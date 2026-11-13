//! The submarine's guided missile: everything the game knows about it.

use super::*;

/// The guided missile (`KIND_MISSILE`): a slim rocket about 2 m long with cruciform fins, fired
/// straight up out of a submarine's launch tube (`SUB_TUBE_AT`) and flown from its nose by the weapons
/// officer who fired it, as a loitering drone is. Its motor always drives it at `MISSILE_SPEED` along its
/// nose, closing any gap at `MISSILE_GRIP` per second and never harder than `MISSILE_MAX_ACCEL`; the
/// mouse sets where it goes, and it points its nose there with pitch and yaw and keeps its wings level. It leaves the tube slowly and
/// nose up, and its motor speeds it up along its nose by no more than `MISSILE_SPEED_UP` m/s²; its fins
/// bite with the square of its share of `MISSILE_FIN_SPEED`, so it climbs almost straight up at first and
/// pitches over towards the look as it gathers speed. At full speed it turns at most `MISSILE_PITCH_RATE`,
/// `MISSILE_YAW_RATE` and `MISSILE_ROLL_RATE` rad/s about its own axes, its angular velocity following
/// at `MISSILE_RESPONSE` per second.
pub const MISSILE_SEATS: usize = 1;
pub const MISSILE_HALF_EXTENTS: [f32; 3] = [0.2, 0.2, 1.0];
pub const MISSILE_SPEED: f32 = 70.0;
pub const MISSILE_GRIP: f32 = 4.0;
pub const MISSILE_MAX_ACCEL: f32 = 90.0;
pub const MISSILE_SPEED_UP: f32 = 18.0;
pub const MISSILE_FIN_SPEED: f32 = 65.0;
pub const MISSILE_PITCH_RATE: f32 = 1.8;
pub const MISSILE_ROLL_RATE: f32 = 5.0;
pub const MISSILE_YAW_RATE: f32 = 1.2;
pub const MISSILE_RESPONSE: f32 = 10.0;
/// The most it banks into a turn, radians (about 70 degrees).
pub const MISSILE_MAX_BANK: f32 = 1.2;
/// How steeply its flier may ask it to climb or dive, radians.
pub const MISSILE_MAX_PITCH: f32 = 1.4;
/// Climbing fades out over the last 20 m below this height, and above it it is pushed down.
pub const MISSILE_CEILING: f32 = 400.0;
/// Air drag on the body, which is all that slows one whose motor has burnt out.
pub const MISSILE_DAMPING: f32 = 0.02;
/// How long its motor burns, ticks, and how far from its flier's body it can be flown, metres.
pub const MISSILE_ENERGY_TICKS: u32 = 20 * TICK_RATE;
pub const MISSILE_RANGE: f32 = 1500.0;
/// Its warhead: smaller than a loitering drone's, bigger than a rocket's.
pub const MISSILE_BLAST_RADIUS: f32 = 8.0;
pub const MISSILE_DAMAGE: f32 = 250.0;
pub const MISSILE_KNOCK: f32 = 20.0;
pub const MISSILE_CRATER: [f32; 2] = [6.0, 1.2];
/// Shot down, its warhead only partly goes off.
pub const MISSILE_POP_RADIUS: f32 = 3.0;
pub const MISSILE_POP_DAMAGE: f32 = 40.0;
pub const MISSILE_POP_CRATER: [f32; 2] = [2.4, 0.4];
/// A contact that takes this much speed (m/s) out of it sets it off, as does coming this close to a
/// soldier (metres); out of fuel or its link lost, it falls this many ticks at most before it goes off.
pub const MISSILE_IMPACT_DV: f32 = 3.0;
pub const MISSILE_PROXIMITY: f32 = 1.5;
pub const MISSILE_FALL_TICKS: u32 = 3 * TICK_RATE;

/// Fired from a tube, not from a soldier's hand: the submarine places it.
const GUIDED: DroneDef = DroneDef {
    energy_ticks: MISSILE_ENERGY_TICKS,
    range: MISSILE_RANGE,
    blast_radius: MISSILE_BLAST_RADIUS,
    damage: MISSILE_DAMAGE,
    knock: MISSILE_KNOCK,
    pop_radius: MISSILE_POP_RADIUS,
    pop_damage: MISSILE_POP_DAMAGE,
    crater: MISSILE_CRATER,
    pop_crater: MISSILE_POP_CRATER,
    impact_dv: MISSILE_IMPACT_DV,
    proximity: MISSILE_PROXIMITY,
    fall_ticks: MISSILE_FALL_TICKS,
    weapon: WEAPON_MISSILE,
    pop_shown_as: WEAPON_ROCKET,
    launch: Launch::Tube,
};

/// A rifle round or two brings it down.
pub(super) const MISSILE: VehicleDef = VehicleDef {
    kind: KIND_MISSILE,
    name: "Guided missile",
    hp: 20,
    seats: MISSILE_SEATS,
    mounts: &[],
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: ROTOR,
    spec: Spec {
        half_extents: MISSILE_HALF_EXTENTS,
        // About 100 kg. Nothing below uses the wheel fields.
        density: 160.0,
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
        lift: Lift::Missile,
        linear_damping: MISSILE_DAMPING,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: true,
        solids: &[],
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
    armor: SHEET,
    drone: Some(GUIDED),
    mount_seat: None,
    board: None,
    exit: None,
    fittings: &[],
};
