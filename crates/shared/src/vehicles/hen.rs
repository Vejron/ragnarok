//! The hen: everything the game knows about it.

use super::*;

/// The hen (`KIND_HEN`): a giant robot hen on the walker's legs, one driver in a cockpit on its back. It
/// walks and balances exactly as the walker does (the `WALKER_*` legs, spring and balance), and adds
/// wings: a leap off the ground (Space), flapping that holds a slow climb while Space stays held in the
/// air, and a glide that holds its fall to a slow sink once the flapping has run out. Jumping and
/// flapping drain the wings' stamina, which comes back while it stands with Space let go.
pub const HEN_SEATS: usize = 1;
pub const HEN_HALF_EXTENTS: [f32; 3] = WALKER_HALF_EXTENTS;
/// The wings' stamina, as a share of full.
pub const HEN_STAMINA: f32 = 1.0;
/// Upward speed a leap adds, m/s (about a 6 m leap), plus this much along its heading at full stick
/// (half of it backwards), and the stamina it costs.
pub const HEN_JUMP_SPEED: f32 = 11.0;
pub const HEN_JUMP_FORWARD: f32 = 6.0;
pub const HEN_JUMP_COST: f32 = 0.2;
/// Flapping, once the leap has topped out, holds this upward speed, m/s, chasing it this hard (per
/// second) by at most this much on top of gravity's worth, m/s², and uses this share of the stamina
/// per second: about three seconds of it.
pub const HEN_FLAP_CLIMB: f32 = 1.5;
pub const HEN_FLAP_GAIN: f32 = 3.0;
pub const HEN_FLAP_ACCEL: f32 = 6.0;
pub const HEN_FLAP_DRAIN: f32 = 0.33;
/// Held in the air, the wings carry it forward towards this speed at full stick, m/s, by at most
/// this much, m/s².
pub const HEN_GLIDE_SPEED: f32 = 12.0;
pub const HEN_GLIDE_ACCEL: f32 = 4.0;
/// With no stamina left the wings spread and hold its fall to this sink, m/s, chasing it this hard,
/// per second.
pub const HEN_GLIDE_SINK: f32 = 4.0;
pub const HEN_GLIDE_GAIN: f32 = 3.0;
/// In the air the wings keep it level, leaning it this far (radians, roughly) with the stick, levelling
/// it this hard and this fast (per second), and turn it at this yaw rate, rad/s.
pub const HEN_AIR_TILT: f32 = 0.2;
pub const HEN_AIR_LEVEL: f32 = 4.0;
pub const HEN_AIR_RESPONSE: f32 = 6.0;
pub const HEN_AIR_YAW: f32 = 1.2;
/// Stamina comes back at this share per second while a foot is down and Space is let go.
pub const HEN_STAMINA_REFILL: f32 = 0.35;
/// Eggs: a clutch of `HEN_EGGS`, laid one at most every `HEN_EGG_COOLDOWN_TICKS` from `HEN_VENT` (hull
/// frame, under the tail) with the hull's velocity and `HEN_EGG_TOSS` m/s backwards, and one growing back
/// every `HEN_EGG_REGROW_TICKS`. An egg falls and bounces like a grenade (`HEN_EGG_RADIUS`, keeping
/// `HEN_EGG_BOUNCE` of its speed), bursts when `HEN_EGG_FUSE_TICKS` run out, and at once when it meets
/// something at more than `HEN_EGG_SMASH_SPEED` m/s, which a laid egg never does but a dropped one does.
pub const HEN_EGGS: u8 = 6;
pub const HEN_EGG_COOLDOWN_TICKS: u32 = TICK_RATE / 2;
pub const HEN_EGG_REGROW_TICKS: u32 = TICK_RATE * 4;
pub const HEN_VENT: [f32; 3] = [0.0, -1.0, 2.0];
pub const HEN_EGG_TOSS: f32 = 3.0;
pub const HEN_EGG_RADIUS: f32 = 0.35;
pub const HEN_EGG_BOUNCE: f32 = 0.3;
pub const HEN_EGG_FUSE_TICKS: u32 = TICK_RATE * 2;
pub const HEN_EGG_SMASH_SPEED: f32 = 16.0;
/// An egg's blast, as a plane's bomb.
pub const HEN_EGG_BLAST_RADIUS: f32 = 6.0;
pub const HEN_EGG_DAMAGE: f32 = 110.0;
pub const HEN_EGG_KNOCK: f32 = 16.0;

const LEG_SOLIDS: [Solid; 2] = super::walker::legs(HEN_HALF_EXTENTS[1]);

/// The walker's legs and hull, with wings and eggs in place of guns.
/// Its pilot sealed in the cab up front, and the fuel at the back.
const ARMOR: Armor = Armor {
    plates: ALLOY.plates,
    modules: &[
        Module::crew(0, [0.0, 0.4, -0.9], [0.4, 0.45, 0.45]),
        Module::new(Part::Fuel, [0.0, -0.3, 1.0], [0.5, 0.3, 0.4], 100),
    ],
    canvas: false,
};

pub(super) const HEN: VehicleDef = VehicleDef {
    kind: KIND_HEN,
    name: "Hen",
    hp: 180,
    seats: HEN_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: LEGS,
    // A lighter hull than the walker's, so it gets off the ground more easily.
    spec: Spec {
        half_extents: HEN_HALF_EXTENTS,
        // About 4.5 tonnes. Nothing below uses the wheel fields; the legs are the `WALKER_*` constants.
        density: 200.0,
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
        tank: Some(HEN_STAMINA),
        buttons: Buttons::Leap,
        aims: false,
        solids: &LEG_SOLIDS,
        tongue: 0.0,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
};

/// The clutch: an egg from under the tail, with the hull's velocity and a little backwards.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: 0,
    aim: Aim::Fixed,
    muzzle: Muzzle::One(HEN_VENT),
    fire: Fire::Drop { kind: PROJECTILE_EGG, toss: [0.0, 0.0, HEN_EGG_TOSS], ttl: HEN_EGG_FUSE_TICKS },
    trigger: BUTTON_FIRE,
    cooldown: HEN_EGG_COOLDOWN_TICKS,
    spread: 0.0,
    recoil: 0.0,
    heat: 0.0,
    ammo: Some(Ammo { full: HEN_EGGS, refill: Refill::Regrow(HEN_EGG_REGROW_TICKS) }),
    needs: &[Need::Upright],
    show: Show::Rack,
}];
