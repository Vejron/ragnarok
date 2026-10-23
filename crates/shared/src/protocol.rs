//! Wire protocol: MessagePack arrays with a leading tag byte. Field names are never sent.
//!
//! The structs have named fields for the code's sake, but `rmp_serde::to_vec` writes a struct as an
//! array of its fields in declaration order, so **the order of the fields is the wire format**:
//! never reorder or insert one without bumping `PROTOCOL_VERSION` and the byte fixtures below.
//! See NETWORKING.md for the full table; `client/src/net/codec.ts` mirrors this file.

use serde::{Deserialize, Serialize};

/// Client → server tags.
pub const C_INPUT: u8 = 0;
pub const C_PING: u8 = 1;
/// Voice signaling, sent on a client-opened reliable stream rather than as a datagram.
pub const C_RTC: u8 = 2;

/// Server → client tags.
pub const S_WELCOME: u8 = 0;
pub const S_SNAPSHOT: u8 = 1;
pub const S_PONG: u8 = 2;
pub const S_LEAVE: u8 = 3;
pub const S_KILL: u8 = 4;
pub const S_SCORES: u8 = 5;
pub const S_DESTROYED: u8 = 6;
pub const S_DAMAGED: u8 = 7;
pub const S_HIT: u8 = 8;
pub const S_RTC: u8 = 9;
pub const S_CARVED: u8 = 10;
pub const S_PROFILES: u8 = 11;
pub const S_NOTICE: u8 = 12;

/// `Hit` roles: whose feedback it is.
/// You hurt something.
pub const HIT_DEALT: u8 = 0;
/// Something hurt you (or the vehicle you sit in).
pub const HIT_TAKEN: u8 = 1;

/// `Hit` kinds: what took the damage.
pub const HIT_BODY: u8 = 0;
pub const HIT_VEHICLE: u8 = 1;
/// A stickman or rooster that this hit finished.
pub const HIT_KILL: u8 = 2;
pub const HIT_ROOSTER: u8 = 3;

/// A `Hit` bearing for damage with no direction: a fall, a crash.
pub const BEARING_NONE: i16 = i16::MIN;

/// `Destroyed` entry kinds: what part of the static world fell.
pub const OBJ_TREE: u8 = 0;
pub const OBJ_BUILDING: u8 = 1;

pub use crate::consts::{KIND_DRONE, KIND_HELI, KIND_HEN, KIND_JEEP, KIND_LOITER, KIND_MORTAR, KIND_PLANE, KIND_ROADSTER, KIND_ROOSTER, KIND_SPORTS, KIND_STICKMAN, KIND_STRV, KIND_TANK, KIND_WALKER};

/// Whether `kind` is a drone: flown from a remote seat by whoever launched it (see the `Drones` mechanic).
pub fn is_drone(kind: u8) -> bool {
    crate::vehicles::vehicle_def(kind).drone.is_some()
}

/// Health of a fresh vehicle of `kind`. Zero on the wire means wrecked.
pub fn vehicle_hp(kind: u8) -> u8 {
    crate::vehicles::vehicle_def(kind).hp
}

/// Seats in a vehicle of `kind`. Seat 0 is the driver (a helicopter's or a plane's pilot); seat 1 of a tank, a
/// helicopter or a plane is the gunner.
pub fn seat_count(kind: u8) -> usize {
    crate::vehicles::vehicle_def(kind).seats
}

/// `replicated::Flags::flags` bits.
pub const FLAG_GROUNDED: u8 = 1 << 0;
/// Dead and waiting to respawn: lying where they fell, and neither hit nor hurt by anything.
pub const FLAG_DEAD: u8 = 1 << 1;
/// Holding crouch, so other clients can pose them low.
pub const FLAG_CROUCHING: u8 = 1 << 2;
/// Bits 3..=6 hold the item in the player's hands (`ITEM_*`), so others can draw it: up to 15 items.
pub const FLAG_HELD_SHIFT: u8 = 3;
pub const FLAG_HELD_MASK: u8 = 0b1111 << FLAG_HELD_SHIFT;
/// A rooster is mid-crow, so clients play its crow animation and sound.
pub const FLAG_CROWING: u8 = 1 << 7;

const _: () = assert!(
    crate::items::ITEM_COUNT as u16 <= (FLAG_HELD_MASK >> FLAG_HELD_SHIFT) as u16 + 1,
    "an item id no longer fits the held bits of the entity flags"
);

/// Weapons, as named in shot and kill events.
/// The AK5, and what an unarmed death (a fall) is credited to.
pub const WEAPON_RIFLE: u8 = 0;
pub const WEAPON_SHELL: u8 = 1;
pub const WEAPON_ROCKET: u8 = 2;
/// A vehicle going up. Only ever the weapon of a shot event or a kill.
pub const WEAPON_WRECK: u8 = 3;
pub const WEAPON_REVOLVER: u8 = 4;
pub const WEAPON_GRENADE: u8 = 5;
/// The helicopter's nose autocannon: a shell's burst, smaller.
pub const WEAPON_AUTOCANNON: u8 = 6;
pub const WEAPON_SNIPER: u8 = 7;
/// Shot events only, never a kill: vehicle `shooter` crashed into something at `x, y, z`.
pub const WEAPON_CRASH: u8 = 8;
/// Shot events only: vehicle `shooter` is scraping along something at `x, y, z`, throwing sparks.
pub const WEAPON_SCRAPE: u8 = 9;
/// A plane's machine guns, forward or rear. Shot events only for a round that hit something: the
/// client draws every round's tracer itself, from the gun's shot counter.
pub const WEAPON_MG: u8 = 10;
pub const WEAPON_BOMB: u8 = 11;
/// A vehicle running a stickman down. Only ever the weapon of a kill.
pub const WEAPON_ROADKILL: u8 = 12;
/// Ten pellets in one pull; this event is the first of them (muzzle flash and sound) and marks where it ended.
pub const WEAPON_SHOTGUN: u8 = 13;
/// Each further pellet of a multi-ray pull: only where it landed is drawn.
pub const WEAPON_PELLET: u8 = 14;
/// A flying bullet ended here (hit something, or the ground). Its shot event's `shooter` is the
/// bullet's id, as in `BulletSpawn`, not a player.
pub const WEAPON_BULLET: u8 = 15;
/// Shot events only: a shot glanced off vehicle `shooter` at `x, y, z`, for a spark and a ping.
pub const WEAPON_RICOCHET: u8 = 16;
/// Shot events only: the turret of tank `shooter` was shot off at `x, y, z`.
pub const WEAPON_TURRET: u8 = 17;
/// A mortar bomb going off.
pub const WEAPON_MORTAR: u8 = 18;
/// An FPV drone's charge going off; a shot-down one's pop too.
pub const WEAPON_DRONE: u8 = 19;
/// A hen's egg going off.
pub const WEAPON_EGG: u8 = 20;
/// A loitering drone's warhead going off.
pub const WEAPON_LOITER: u8 = 21;
/// Shot events only: a flare gun fired from `shooter`'s hand, at `x, y, z`.
pub const WEAPON_FLARE: u8 = 22;

/// How a stickman died, as sent in a `Kill`, so every client can stage the death to match.
pub const DEATH_BODY: u8 = 0;
pub const DEATH_HEAD: u8 = 1;
pub const DEATH_BLAST: u8 = 2;
pub const DEATH_ROADKILL: u8 = 3;
/// Far more damage than was left to take.
pub const DEATH_OVERKILL: u8 = 4;

/// `replicated::Projectile` kinds.
pub const PROJECTILE_ROCKET: u8 = 0;
pub const PROJECTILE_GRENADE: u8 = 1;
pub const PROJECTILE_SHELL: u8 = 2;
/// An autocannon round: flies like a shell, bursts smaller.
pub const PROJECTILE_CANNON: u8 = 3;
/// A sniper round: fast and nearly flat, it hurts whoever it meets and does not burst.
pub const PROJECTILE_BULLET: u8 = 4;
/// A bomb dropped from a plane: it falls from the plane's speed and bursts on the first thing it meets.
pub const PROJECTILE_BOMB: u8 = 5;
/// A mortar bomb: a high lob that falls steeply and bursts on the first thing it meets.
pub const PROJECTILE_MORTAR: u8 = 6;
/// A hen's egg: it falls and bounces like a grenade and bursts when its fuse runs out, or at once when it lands hard.
pub const PROJECTILE_EGG: u8 = 7;
/// A flare: it climbs, then hangs under its parachute, burning brightly as it drifts down, and burns
/// out where it lies. It hurts nobody.
pub const PROJECTILE_FLARE: u8 = 8;

/// A bullet leaving a gun: `[id, shooter, item, x, y, z, vx, vy, vz, tick]`, centimetres and cm/s.
/// Sent once (in two snapshots, in case one is lost) and not listed again: the path is
/// deterministic, so a client works out where the bullet is from `tick` on. It moves `v * dt` a
/// tick and then `v.y` loses `GRAVITY * BULLET_GRAVITY_SCALE * dt`, so after `n` ticks it is at
/// `p + v * n * dt - (0, g * dt^2 * n * (n + 1) / 2, 0)` with `g = GRAVITY * BULLET_GRAVITY_SCALE`.
/// It ends with a `ShotEvent` of `WEAPON_BULLET` carrying `id`, or when `item`'s range runs out.
/// `item` is the gun, so a client knows its range and look. `tick` is the server tick it was fired on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulletSpawn {
    pub id: u32,
    pub shooter: u32,
    pub item: u8,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub vx: i16,
    pub vy: i16,
    pub vz: i16,
    pub tick: u32,
}

/// One simulation tick of player intent. The client predicts with exactly these quantized values,
/// so client and server feed the simulation identical inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputFrame {
    /// Monotonic per-connection sequence number, starting at 1. One frame per client tick.
    pub seq: u32,
    /// Strafe axis, -127 (left) ..= 127 (right).
    pub strafe: i8,
    /// Forward axis, -127 (back) ..= 127 (forward).
    pub forward: i8,
    /// Facing in milliradians, `[-π, π)`. 0 faces -Z.
    pub yaw: i16,
    /// `BUTTON_*` bits.
    pub buttons: u16,
    /// Where the player looks up or down, milliradians (+ is up). Used by a tank gunner and the rifle.
    pub pitch: i16,
    /// The server tick this frame's picture showed, modulo 65536: the interpolated world the player was looking at when they pulled the trigger. The server rewinds other players to it.
    pub view: u16,
    /// The hotbar slot the player has selected, `0..HOTBAR_SLOTS`.
    pub slot: u8,
}

impl InputFrame {
    pub fn seq(&self) -> u32 {
        self.seq
    }
    pub fn buttons(&self) -> u16 {
        self.buttons
    }
    pub fn pitch(&self) -> i16 {
        self.pitch
    }
    pub fn view(&self) -> u16 {
        self.view
    }
    pub fn slot(&self) -> u8 {
        self.slot
    }
}

/// `[0, version, baseline, [frame, ...]]`: the newest `INPUT_REDUNDANCY` frames, oldest first.
/// `baseline` is the tick of the newest snapshot the client has rebuilt, or 0 for none: the server
/// diffs the next snapshot against it.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ClientInput(pub u8, pub u8, pub u32, pub Vec<InputFrame>);

/// `[1, nonce]`: latency probe, echoed as a `Pong`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Ping(pub u8, pub u32);

/// `[0, version, entity_id, tick_rate, snapshot_every, world_hash]`, sent once on a reliable stream.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Welcome {
    pub tag: u8,
    pub version: u8,
    pub id: u32,
    pub tick_rate: u8,
    pub snapshot_every: u8,
    pub world_hash: u32,
}

/// Where a seated player looks and what they hold, so others can draw them sitting: `[yaw, pitch,
/// held]`, milliradians in the world (yaw 0 faces -Z) and an `ITEM_*`. All zeros for an empty seat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rider {
    pub yaw: i16,
    pub pitch: i16,
    pub held: u8,
}

/// Indices into a vehicle's damage: the four sides of the body, then the wheels front left,
/// front right, rear left, rear right (a tank's and a helicopter's wheels are never hurt).
pub const DAMAGE_FRONT: usize = 0;
pub const DAMAGE_REAR: usize = 1;
pub const DAMAGE_LEFT: usize = 2;
pub const DAMAGE_RIGHT: usize = 3;
pub const DAMAGE_WHEELS: usize = 4;
/// A tank's slot 4 (its wheels are never hurt): `TURRET_GONE` once the turret has been shot off.
pub const DAMAGE_TURRET: usize = 4;
pub const TURRET_GONE: u8 = 255;
/// A wheel damaged this much has come off.
pub const WHEEL_GONE: u8 = 255;

/// `replicated::Turret::flags`: a field mortar is hitched to a car, so it is drawn folded up for travel.
pub const TURRET_STOWED: u8 = 1;

/// `replicated::Arms::flags`: the blip switch is cutting the engine, and either set of guns is jammed.
pub const ARMS_CUT: u8 = 1 << 0;
pub const ARMS_JAMMED: u8 = 1 << 1;
pub const ARMS_REAR_JAMMED: u8 = 1 << 2;

/// `replicated::Thrust::flags`: the rear (boost) nozzles and the lift (hop and hover) nozzles.
pub const THRUST_BOOST: u8 = 1 << 0;
pub const THRUST_LIFT: u8 = 1 << 1;
/// A drone's: its pilot is still flying it, rather than it falling with the link lost.
pub const THRUST_LINKED: u8 = 1 << 2;
/// A hen's: its wings are spread and still, holding its fall to a glide.
pub const THRUST_GLIDE: u8 = 1 << 3;
/// A hen's: neither foot is on the ground.
pub const THRUST_AIR: u8 = 1 << 4;

/// Something that just happened, for drawing: `[shooter, weapon, x, y, z]`, where `x, y, z` is
/// where it ended, in centimetres. A rifle shot is a tracer from the shooter's own position to
/// there; a rocket or a wreck is a burst at there (the shooter is 0 for a wreck with no culprit).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShotEvent {
    pub shooter: u32,
    pub weapon: u8,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// One hotbar slot: `[item, count, ammo]`. `item` is `ITEM_NONE` for an empty slot; `ammo` is the
/// rounds left in the magazine of a gun.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemSlot {
    pub item: u8,
    pub count: u8,
    pub ammo: u8,
}

/// `[1, server_tick, baseline, ack_input_seq, ride, [update, ...], [removed id, ...], [shot, ...],
/// [bullet spawn, ...]]`, sent as a datagram.
///
/// `baseline` is the tick of the snapshot the updates are relative to, or 0 when they are not (every
/// entity in range then comes whole). Each update is one entity's components
/// (`shared::replicated::Update`): stickmen, creatures, vehicles, projectiles and items on the ground
/// alike. `ack_input_seq` is the newest input of the *receiving* player that the state already
/// includes. `ride` is the id of the vehicle the receiving player stands on top of, or 0: it is carried
/// with that vehicle, so the client must predict relative to it. What only the receiving player is told
/// (their health and hotbar, the exact motion of what they drive) are `[owner]` components of their own
/// entity and their vehicle's, among the updates. `shots` are the effects
/// since the previous snapshot, capped at `MAX_SHOT_EVENTS`, and `bullets` the gun rounds the client
/// flies itself.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub tag: u8,
    pub tick: u32,
    pub baseline: u32,
    pub ack: u32,
    pub ride: u32,
    pub updates: Vec<crate::replicated::Update>,
    pub removed: Vec<u32>,
    pub shots: Vec<ShotEvent>,
    /// Bullets fired since a few snapshots ago, which the client then flies itself.
    pub bullets: Vec<BulletSpawn>,
}

/// A `Snapshot` as the server sends it, borrowing its lists: the same bytes, with nothing copied to
/// encode them.
#[derive(Serialize)]
pub struct SnapshotOut<'a> {
    pub tag: u8,
    pub tick: u32,
    pub baseline: u32,
    pub ack: u32,
    pub ride: u32,
    pub updates: &'a [crate::replicated::Update],
    pub removed: &'a [u32],
    pub shots: &'a [ShotEvent],
    pub bullets: &'a [BulletSpawn],
}

/// `[2, nonce]`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Pong(pub u8, pub u32);

/// `[3, entity_id]`, sent on a reliable stream when a player leaves.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Leave(pub u8, pub u32);

/// `[4, killer, victim, weapon, killer_kills, victim_deaths, style, yaw, power]`, sent on a reliable
/// stream to everyone. `killer` is 0 when the world did it (a fall); the totals are the new ones, so
/// clients keep no tally of their own. `style` is `DEATH_*`; `yaw` (milliradians, 0 faces -Z, as for
/// a look) is the way the body is thrown and `power` how hard, in units of 0.2 m/s.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Kill {
    pub tag: u8,
    pub killer: u32,
    pub victim: u32,
    pub weapon: u8,
    pub killer_kills: u16,
    pub victim_deaths: u16,
    pub style: u8,
    pub yaw: i16,
    pub power: u8,
}

/// `[5, [[id, kills, deaths], ...]]`, sent on a reliable stream to a player who just joined.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Scores(pub u8, pub Vec<(u32, u16, u16)>);

/// `[11, [[id, name, color, team, bot], ...]]`, sent on a reliable stream: to a player who just joined
/// with everyone's, and to everyone else with the joiner's (a bot's when it is added). `color` indexes
/// `PLAYER_COLOR_COUNT` colours. `team` is 0 for people, who are all against all; players on the same
/// other team are on one side. `bot` is 1 for a bot (see BOTS.md).
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Profiles(pub u8, pub Vec<ProfileEntry>);

/// One `Profiles` entry: `(id, name, color, team, bot)`.
pub type ProfileEntry = (u32, String, u8, u8, u8);

/// `Notice` kinds: what a scenario is telling everyone.
/// A message, shown for a few seconds.
pub const NOTICE_SAY: u8 = 0;
/// What to do now, shown until the next one; empty clears it. A joiner gets the current one.
pub const NOTICE_OBJECTIVE: u8 = 1;
/// The scenario is over: won, or lost. It starts again a little later.
pub const NOTICE_WON: u8 = 2;
pub const NOTICE_LOST: u8 = 3;

/// `[12, kind, text]`, sent on a reliable stream to everyone when a world's scenario has something to
/// say (see BOTS.md). `kind` is `NOTICE_*`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Notice(pub u8, pub u8, pub String);

/// `[6, [[kind, col, row, index, yaw], ...]]`, sent on a reliable stream: to everyone when
/// buildings or trees are destroyed, and to a player who just joined with all that already are.
/// `kind` is `OBJ_*`; `(col, row, index)` is the object's `ObjId`; `yaw` (milliradians, as
/// elsewhere) is the way a tree falls, or the direction a blast came from for a building.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Destroyed(pub u8, pub Vec<(u8, u16, u16, u32, i16, u8)>);

/// `[7, [[col, row, index, stage, marked, x, y, z], ...]]`, sent on a reliable stream: to everyone
/// when buildings are hurt but still stand, and to a player who just joined with every hurt one.
/// `(col, row, index)` is the building's `ObjId`; `stage` is `0..=DAMAGE_STAGES` (higher is worse,
/// the last is about to collapse). When `marked` is 1, a blast landed at `(x, y, z)` (cm, as in
/// `ShotEvent`) and leaves a scorch mark there; otherwise the position is zero.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Damaged(pub u8, pub Vec<DamagedEntry>);

/// One `Damaged` entry: `(col, row, index, stage, marked, x, y, z)`.
pub type DamagedEntry = (u16, u16, u32, u8, u8, i32, i32, i32);

/// `[10, live, [[col, row, index, x, y, z, [blasted, ...], [fell, ...]], ...]]`, sent on a reliable
/// stream: to everyone when a blast takes cells out of a standing building, and to a player who just
/// joined with every cell already gone (`live` 0, so it is state and not news). `(col, row, index)`
/// is the building's `ObjId`; `(x, y, z)` (cm) is where the blast landed; `blasted` are the cells it
/// broke and `fell` the cells it left unsupported (`carve::Shell::index`). A joiner's catch-up lists
/// every removed cell under `blasted`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Carved(pub u8, pub u8, pub Vec<CarvedEntry>);

/// One `Carved` entry.
pub type CarvedEntry = (u16, u16, u32, i32, i32, i32, Vec<u32>, Vec<u32>);

/// `[8, role, kind, amount, weapon, bearing, x, y, z, target]`, sent on a reliable stream to the one player it is
/// about, once per tick per `(role, kind)` with the amounts summed. `role` is `HIT_*`: dealt (you
/// hurt something) or taken (something hurt you or your vehicle); `kind` is `HIT_*` too; `amount`
/// is the health lost; `weapon` is `WEAPON_*`. For a taken hit `bearing` is the yaw (milliradians,
/// 0 faces -Z, as for a look) from you towards where it came from, or `BEARING_NONE`. `(x, y, z)`
/// (cm, as in `ShotEvent`) is about where the thing hit is, chest height for a stickman, and
/// `target` is its id, so clients can show a number and a flash on it.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hit {
    pub tag: u8,
    pub role: u8,
    pub kind: u8,
    pub amount: u8,
    pub weapon: u8,
    pub bearing: i16,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub target: u32,
}

/// `RTC_*` kinds: what an `Rtc` payload is.
pub const RTC_OFFER: u8 = 0;
pub const RTC_ANSWER: u8 = 1;
pub const RTC_ICE: u8 = 2;
/// Longest `Rtc` payload the server relays, bytes. A voice-only SDP is under 2 KiB.
pub const RTC_MAX_PAYLOAD: usize = 8192;

/// `[2 | 9, peer, kind, json]`: WebRTC signaling for proximity voice, relayed by the server and never
/// interpreted. Client → server (tag `C_RTC`, on a reliable stream) `peer` is the player it is for;
/// server → client (tag `S_RTC`, on the reliable stream) `peer` is the player it is from, which the
/// server fills in, so it cannot be forged. `kind` is `RTC_*` and `json` the session description or
/// ICE candidate as a JSON string. The voice itself goes peer to peer.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rtc(pub u8, pub u32, pub u8, pub String);

/// Reads the tag of a `[tag, ...]` message without decoding the rest. Every message is an array
/// (a fixarray header, or array16 once there are more than 15 fields, as a snapshot has) whose
/// first element is a positive fixint.
pub fn peek_tag(bytes: &[u8]) -> Option<u8> {
    match bytes {
        [0x91..=0x9f, tag @ 0x00..=0x7f, ..] | [0xdc, _, _, tag @ 0x00..=0x7f, ..] => Some(*tag),
        _ => None,
    }
}

pub fn encode<T: Serialize>(message: &T) -> Vec<u8> {
    rmp_serde::to_vec(message).expect("protocol messages always serialize")
}

pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Option<T> {
    rmp_serde::from_slice(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::*;
    use crate::items::*;

    // These fixtures are duplicated in `client/src/__tests__/codec.spec.ts`. If either side
    // changes, both tests must be updated together.

    pub const INPUT_FIXTURE: &[u8] = &[
        148, 0, 40, 205, 4, 206, 146, 152, 41, 127, 0, 205, 6, 35, 0, 205, 1, 44, 205, 238, 72, 0, 152, 42, 208, 129, 127, 209, 248, 98, 11, 209, 255, 106, 7, 3
    ];

    pub const SNAPSHOT_FIXTURE: &[u8] = &[
        153, 1, 205, 4, 210, 205, 4, 206, 42, 9, 148, 156, 1, 206, 0, 6, 0, 7, 25, 0, 0, 208, 136, 205, 2, 138, 0, 0, 73, 204, 128, 149, 147, 2, 1, 17, 147, 0, 0, 0, 147, 3, 2, 0, 147, 0, 0, 0, 147, 0, 0, 0, 156, 7, 205, 64, 15, 0, 85, 206, 0, 1, 134, 160, 0, 0, 0, 0, 3, 0, 0, 220, 0, 25, 10, 206, 0, 8, 76, 113, 205, 1, 44, 90, 209, 248, 48, 0, 205, 2, 188, 0, 32, 209, 253, 68, 204, 250, 3, 205, 4, 210, 1, 146, 1, 7, 204, 250, 2, 120, 251, 209, 246, 60, 30, 209, 254, 112, 15, 204, 250, 209, 253, 168, 147, 12, 206, 128, 0, 0, 0, 205, 16, 0, 146, 3, 11, 146, 149, 7, 0, 205, 5, 220, 204, 160, 209, 239, 152, 149, 1, 0, 236, 90, 205, 121, 24, 145, 154, 77, 7, 2, 120, 204, 165, 209, 244, 72, 205, 5, 220, 236, 209, 150, 136, 205, 4, 209,
    ];

    fn sample_input() -> ClientInput {
        ClientInput(
            C_INPUT,
            PROTOCOL_VERSION,
            1230,
            vec![
                InputFrame { seq: 41, strafe: 127, forward: 0, yaw: 1571, buttons: 0, pitch: 300, view: 61_000, slot: 0 },
                InputFrame { seq: 42, strafe: -127, forward: 127, yaw: -1950, buttons: BUTTON_JUMP | BUTTON_RUN | BUTTON_FIRE, pitch: -150, view: 7, slot: 3 },
            ],
        )
    }

    fn sample_snapshot() -> Snapshot {
        use crate::replicated::{self as wire, Replicated};
        let walker = |x, yaw, vy| Replicated {
            kind: Some(wire::Kind { kind: KIND_STICKMAN }),
            place: Some(wire::Place { x, y: 85, z: 100_000 }),
            heading: Some(wire::Heading { yaw }),
            motion: Some(wire::Motion { vy, knock_x: 0, knock_z: 0 }),
            flags: Some(wire::Flags { flags: FLAG_GROUNDED, stance: 0 }),
            ..Default::default()
        };
        let tank = Replicated {
            kind: Some(wire::Kind { kind: KIND_TANK }),
            place: Some(wire::Place { x: 300, y: 90, z: -2000 }),
            orientation: Some(wire::Orientation { qx: 0, qy: 700, qz: 0 }),
            steer: Some(wire::Steer { steer: 32 }),
            seats: Some(wire::Seats { riders: vec![1, 7] }),
            turret: Some(wire::Turret { yaw: -700, pitch: 250, shots: 3, range: 1234, flags: TURRET_STOWED }),
            health: Some(wire::Health { hp: 250 }),
            ..Default::default()
        };
        let hurt = Replicated { damage: Some(wire::Damage { parts: [0, 0, 40, 0, 0, 0, 255, 3] }), ..tank.clone() };
        let updates = vec![
            // A stickman moving: differences. A dead one new to the receiver, whole. A tank, whole, and one repaired.
            // The receiver themself, with what only they are told, and the tank they drive.
            Replicated { vitals: Some(wire::Vitals { hp: 73, progress: 128 }), hotbar: Some(wire::Hotbar { slots: [ItemSlot { item: ITEM_AK5, count: 1, ammo: 17 }, ItemSlot { item: ITEM_NONE, count: 0, ammo: 0 }, ItemSlot { item: ITEM_GRENADE, count: 2, ammo: 0 }, ItemSlot::default(), ItemSlot::default()] }), ..walker(25, -120, 650) }.update(1, Some(&walker(0, 0, 0))).unwrap(),
            Replicated { flags: Some(wire::Flags { flags: FLAG_GROUNDED | FLAG_DEAD, stance: 0 }), ..walker(0, 0, 0) }.update(7, None).unwrap(),
            Replicated { drive: Some(wire::Drive { vx: 120, vy: -5, vz: -2500, wx: 30, wy: -400, wz: 15, steer: 250, spin: -600 }), ..tank.clone() }.update(10, None).unwrap(),
            tank.update(12, Some(&hurt)).unwrap(),
        ];
        Snapshot { tag: S_SNAPSHOT, tick: 1234, baseline: 1230, ack: 42, ride: 9, updates, removed: vec![3, 11], shots: vec![ShotEvent { shooter: 7, weapon: WEAPON_RIFLE, x: 1500, y: 160, z: -4200 }, ShotEvent { shooter: 1, weapon: WEAPON_RIFLE, x: -20, y: 90, z: 31_000 }], bullets: vec![BulletSpawn { id: 77, shooter: 7, item: ITEM_AK5, x: 120, y: 165, z: -3000, vx: 1500, vy: -20, vz: -27000, tick: 1233 }] }
    }

    /// `[6, [[OBJ_TREE, 2, 3, 70000, -1571, 90]]]`, mirrored in `codec.spec.ts`.
    pub const DESTROYED_FIXTURE: &[u8] = &[146, 6, 145, 150, 0, 2, 3, 206, 0, 1, 17, 112, 209, 249, 221, 90];

    /// `[7, [[2, 3, 70000, 3, 1, 1000, 20, -1000]]]`, mirrored in `destruction.spec.ts`.
    pub const DAMAGED_FIXTURE: &[u8] = &[146, 7, 145, 152, 2, 3, 206, 0, 1, 17, 112, 3, 1, 205, 3, 232, 20, 209, 252, 24];

    #[test]
    fn damaged_message_matches_the_fixture() {
        let message = Damaged(S_DAMAGED, vec![(2, 3, 70_000, 3, 1, 1000, 20, -1000)]);
        assert_eq!(encode(&message), DAMAGED_FIXTURE);
        assert_eq!(decode::<Damaged>(DAMAGED_FIXTURE), Some(message));
        assert_eq!(peek_tag(DAMAGED_FIXTURE), Some(S_DAMAGED));
    }

    /// `[10, 1, [[2, 3, 70000, 1000, 20, -1000, [5, 300], [9]]]]`, mirrored in `destruction.spec.ts`.
    pub const CARVED_FIXTURE: &[u8] = &[147, 10, 1, 145, 152, 2, 3, 206, 0, 1, 17, 112, 205, 3, 232, 20, 209, 252, 24, 146, 5, 205, 1, 44, 145, 9];

    #[test]
    fn carved_message_matches_the_fixture() {
        let message = Carved(S_CARVED, 1, vec![(2, 3, 70_000, 1000, 20, -1000, vec![5, 300], vec![9])]);
        assert_eq!(encode(&message), CARVED_FIXTURE);
        assert_eq!(decode::<Carved>(CARVED_FIXTURE), Some(message));
        assert_eq!(peek_tag(CARVED_FIXTURE), Some(S_CARVED));
    }

    #[test]
    fn hit_round_trips() {
        let message = Hit { tag: S_HIT, role: HIT_TAKEN, kind: HIT_BODY, amount: 25, weapon: WEAPON_RIFLE, bearing: -1571, x: 100, y: 90, z: -250, target: 7 };
        assert_eq!(decode::<Hit>(&encode(&message)), Some(message));
        assert_eq!(peek_tag(&encode(&Hit { tag: S_HIT, role: HIT_DEALT, kind: HIT_KILL, amount: 100, weapon: WEAPON_SNIPER, bearing: BEARING_NONE, x: 0, y: 0, z: 0, target: 0 })), Some(S_HIT));
    }

    #[test]
    fn destroyed_matches_fixture() {
        let message = Destroyed(S_DESTROYED, vec![(OBJ_TREE, 2, 3, 70_000, -1571, 90)]);
        assert_eq!(encode(&message), DESTROYED_FIXTURE);
        assert_eq!(decode::<Destroyed>(DESTROYED_FIXTURE), Some(message));
        assert_eq!(peek_tag(DESTROYED_FIXTURE), Some(S_DESTROYED));
    }

    /// `[11, [[7, "Åsa", 3, 2, 1]]]`, mirrored in `codec.spec.ts`.
    pub const PROFILES_FIXTURE: &[u8] = &[146, 11, 145, 149, 7, 164, 195, 133, 115, 97, 3, 2, 1];

    #[test]
    fn profiles_match_fixture() {
        let message = Profiles(S_PROFILES, vec![(7, "Åsa".into(), 3, 2, 1)]);
        assert_eq!(encode(&message), PROFILES_FIXTURE);
        assert_eq!(decode::<Profiles>(PROFILES_FIXTURE), Some(message));
        assert_eq!(peek_tag(PROFILES_FIXTURE), Some(S_PROFILES));
    }

    /// `[12, 1, "Go"]`, mirrored in `codec.spec.ts`.
    pub const NOTICE_FIXTURE: &[u8] = &[147, 12, 1, 162, 71, 111];

    #[test]
    fn notice_matches_fixture() {
        let message = Notice(S_NOTICE, NOTICE_OBJECTIVE, "Go".into());
        assert_eq!(encode(&message), NOTICE_FIXTURE);
        assert_eq!(decode::<Notice>(NOTICE_FIXTURE), Some(message));
        assert_eq!(peek_tag(NOTICE_FIXTURE), Some(S_NOTICE));
    }

    #[test]
    fn rtc_matches_fixture() {
        // Copied byte for byte in `client/src/__tests__/codec.spec.ts`.
        assert_eq!(encode(&Rtc(S_RTC, 5, RTC_ICE, "x".into())), [148, 9, 5, 2, 161, 120]);
        assert_eq!(decode::<Rtc>(&[148, 2, 5, 2, 161, 120]), Some(Rtc(C_RTC, 5, RTC_ICE, "x".into())));
    }

    #[test]
    fn input_matches_fixture() {
        assert_eq!(encode(&sample_input()), INPUT_FIXTURE);
        assert_eq!(decode::<ClientInput>(INPUT_FIXTURE), Some(sample_input()));
        assert_eq!(peek_tag(INPUT_FIXTURE), Some(C_INPUT));
    }


    #[test]
    fn snapshot_matches_fixture() {
        assert_eq!(encode(&sample_snapshot()), SNAPSHOT_FIXTURE);
        assert_eq!(decode::<Snapshot>(SNAPSHOT_FIXTURE), Some(sample_snapshot()));
        assert_eq!(peek_tag(SNAPSHOT_FIXTURE), Some(S_SNAPSHOT));
        let Snapshot { tag, tick, baseline, ack, ride, updates, removed, shots, bullets } = sample_snapshot();
        let out = SnapshotOut { tag, tick, baseline, ack, ride, updates: &updates, removed: &removed, shots: &shots, bullets: &bullets };
        assert_eq!(encode(&out), SNAPSHOT_FIXTURE, "sent borrowed, it is the same bytes");
    }

    #[test]
    fn peek_rejects_garbage() {
        assert_eq!(peek_tag(&[]), None);
        assert_eq!(peek_tag(&[0x90]), None);
        assert_eq!(peek_tag(&[0x92, 0xcc, 0x01]), None);
    }
}
