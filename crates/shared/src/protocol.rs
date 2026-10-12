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

/// Entity kinds.
pub const KIND_STICKMAN: u8 = 0;
/// A Hedemora rooster, an NPC. Appears in `EntityState`, like a stickman, with its own model.
pub const KIND_ROOSTER: u8 = 5;
/// Vehicle kinds. These appear in `VehicleState`, not in `EntityState`.
pub const KIND_JEEP: u8 = 1;
pub const KIND_TANK: u8 = 2;
pub const KIND_HELI: u8 = 3;
pub const KIND_SPORTS: u8 = 4;
/// The triplane. 5 is the rooster's.
pub const KIND_PLANE: u8 = 6;

/// Health of a fresh vehicle of `kind`. Zero on the wire means wrecked.
pub fn vehicle_hp(kind: u8) -> u8 {
    crate::vehicles::vehicle_def(kind).hp
}

/// Seats in a vehicle of `kind`. Seat 0 is the driver (a helicopter's or a plane's pilot); seat 1 of a tank, a
/// helicopter or a plane is the gunner.
pub fn seat_count(kind: u8) -> usize {
    crate::vehicles::vehicle_def(kind).seats
}

/// `EntityState::flags` bits.
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

/// How a stickman died, as sent in a `Kill`, so every client can stage the death to match.
pub const DEATH_BODY: u8 = 0;
pub const DEATH_HEAD: u8 = 1;
pub const DEATH_BLAST: u8 = 2;
pub const DEATH_ROADKILL: u8 = 3;
/// Far more damage than was left to take.
pub const DEATH_OVERKILL: u8 = 4;

/// `ProjectileState` kinds.
pub const PROJECTILE_ROCKET: u8 = 0;
pub const PROJECTILE_GRENADE: u8 = 1;
pub const PROJECTILE_SHELL: u8 = 2;
/// An autocannon round: flies like a shell, bursts smaller.
pub const PROJECTILE_CANNON: u8 = 3;
/// A sniper round: fast and nearly flat, it hurts whoever it meets and does not burst.
pub const PROJECTILE_BULLET: u8 = 4;
/// A bomb dropped from a plane: it falls from the plane's speed and bursts on the first thing it meets.
pub const PROJECTILE_BOMB: u8 = 5;

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
    pub buttons: u8,
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
    pub fn buttons(&self) -> u8 {
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

/// One entity in a snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityState {
    /// Entity id.
    pub id: u32,
    /// `KIND_*`.
    pub kind: u8,
    /// Position of the body origin, centimetres.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Facing, milliradians.
    pub yaw: i16,
    /// Vertical velocity, cm/s. Needed to resume a predicted jump from the authoritative state.
    pub vy: i16,
    /// `FLAG_*` bits.
    pub flags: u8,
    /// Knockback velocity (a car hit them), cm/s along X and Z. Needed to resume a predicted slide from the authoritative state.
    pub knock_x: i16,
    pub knock_z: i16,
}

/// One vehicle in a snapshot. Players sitting in a vehicle are not listed as entities: the vehicle's
/// seats say who is in it, and the client draws them from that.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VehicleState {
    /// Vehicle id. Shares the id space with players.
    pub id: u32,
    /// `KIND_*`.
    pub kind: u8,
    /// Position of the chassis centre, centimetres.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Chassis orientation as the `x, y, z` of a unit quaternion scaled by 32767, with `w >= 0`.
    pub qx: i16,
    pub qy: i16,
    pub qz: i16,
    /// Front wheel steering as a fraction of its kind's `*_MAX_STEER`, -127..=127 (+ is right).
    pub steer: i8,
    /// Player id in each seat, or 0 for an empty one. Seat 0 is the driver.
    pub seats: Vec<u32>,
    /// The gun, for vehicles that have one; `nil` otherwise.
    pub turret: Option<Turret>,
    /// Health; 0 is a burning wreck nobody can board.
    pub hp: u8,
    /// Crash damage, `nil` while there is none: `[front, rear, left, right, wheel FL, FR, RL, RR]`, each 0..=255. A wheel at 255 has come off. See `DAMAGE_*`.
    pub damage: Option<[u8; 8]>,
    /// A plane's pilot weapons and engine, `nil` for anything else.
    pub arms: Option<Arms>,
}

/// What changed in a standing entity since the baseline: `[id, dx, dy, dz, dyaw, dvy, flags,
/// dknock_x, dknock_z]`. Every field but `id` and `flags` is the new value minus the old one.
/// Entities that did not change at all are not listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDelta {
    pub id: u32,
    pub dx: i32,
    pub dy: i32,
    pub dz: i32,
    pub dyaw: i32,
    pub dvy: i32,
    /// `FLAG_*` bits, the new value.
    pub flags: u8,
    pub dknock_x: i32,
    pub dknock_z: i32,
}

/// What changed in a vehicle since the baseline: `[id, dx, dy, dz, dqx, dqy, dqz, dsteer, seats,
/// turret, hp, damage, arms]`. The first eight are differences; `seats`, `turret`, `damage` and `arms`
/// are `nil` when unchanged (a repaired vehicle's `damage` is all zeros), and `hp` is the new value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VehicleDelta {
    pub id: u32,
    pub dx: i32,
    pub dy: i32,
    pub dz: i32,
    pub dqx: i32,
    pub dqy: i32,
    pub dqz: i32,
    pub dsteer: i32,
    pub seats: Option<Vec<u32>>,
    pub turret: Option<Turret>,
    pub hp: u8,
    pub damage: Option<[u8; 8]>,
    pub arms: Option<Arms>,
}

/// Indices into a vehicle's damage: the four sides of the body, then the wheels front left,
/// front right, rear left, rear right (a tank's and a helicopter's wheels are never hurt).
pub const DAMAGE_FRONT: usize = 0;
pub const DAMAGE_REAR: usize = 1;
pub const DAMAGE_LEFT: usize = 2;
pub const DAMAGE_RIGHT: usize = 3;
pub const DAMAGE_WHEELS: usize = 4;
/// A wheel damaged this much has come off.
pub const WHEEL_GONE: u8 = 255;

/// A vehicle's gun, as it is drawn: the turret and barrel angles in the hull frame, and the last
/// shot. `shots` counts shots modulo 256, so a client sees a new shot as a change in the counter;
/// `range` is how far that shell flew before it hit something, in decimetres.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turret {
    /// Turret yaw relative to the hull, milliradians (+ turns left).
    pub yaw: i16,
    /// Barrel elevation, milliradians (+ is up).
    pub pitch: i16,
    pub shots: u8,
    pub range: u16,
}

/// A plane's weapons and engine, as they are drawn and shown on the crew's gauges:
/// `[shots, heat, rear_heat, bombs, flags]`. `shots` counts the forward guns' rounds modulo 256, as
/// `Turret::shots` does the rear gun's (the left gun fires the even ones); the heats are 0..=255 and
/// jam the guns at 255; `bombs` are left on the rack; `flags` are `ARMS_*`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arms {
    pub shots: u8,
    pub heat: u8,
    pub rear_heat: u8,
    pub bombs: u8,
    pub flags: u8,
}

/// `Arms` flags: the blip switch is cutting the engine, and either set of guns is jammed.
pub const ARMS_CUT: u8 = 1 << 0;
pub const ARMS_JAMMED: u8 = 1 << 1;
pub const ARMS_REAR_JAMMED: u8 = 1 << 2;

/// The motion a driver needs to predict their own vehicle, which the vehicle's pose does not
/// carry. Sent only to the player in the driver seat: `[vx, vy, vz, wx, wy, wz, steer, spin]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveState {
    /// Linear velocity, cm/s.
    pub vx: i16,
    pub vy: i16,
    pub vz: i16,
    /// Angular velocity about the world axes, milliradians per second.
    pub wx: i16,
    pub wy: i16,
    pub wz: i16,
    /// Front wheel angle, milliradians, + is right.
    pub steer: i16,
    /// Yaw rate a tank has ramped to, milliradians per second, + is clockwise.
    pub spin: i16,
}

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

/// A rocket or grenade in flight: `[id, kind, x, y, z, vx, vy, vz]`, centimetres and cm/s. A client
/// extrapolates nothing: it interpolates these like any entity and aims the rocket along `v`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectileState {
    pub id: u32,
    pub kind: u8,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub vx: i16,
    pub vy: i16,
    pub vz: i16,
}

/// One hotbar slot: `[item, count, ammo]`. `item` is `ITEM_NONE` for an empty slot; `ammo` is the
/// rounds left in the magazine of a gun.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemSlot {
    pub item: u8,
    pub count: u8,
    pub ammo: u8,
}

/// What only the receiving player is told about themselves: `[hp, progress, [slot, ...]]`.
/// `hp` is their health, `0` is dead. `progress` is how far a reload or a bandage has got, 0..=255,
/// or 0 when neither is going on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfState {
    pub hp: u8,
    pub progress: u8,
    pub slots: Vec<ItemSlot>,
}

/// An item lying on the ground: `[id, item, x, y, z]`, centimetres. Static until taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickupState {
    pub id: u32,
    pub item: u8,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// `[1, server_tick, baseline, ack_input_seq, ride, [new entity, ...], [entity delta, ...],
/// [new vehicle, ...], [vehicle delta, ...], [removed id, ...], drive, self, [shot, ...],
/// [projectile, ...], [pickup, ...], [bullet spawn, ...]]`, sent as a datagram.
/// `baseline` is the tick of the snapshot the lists are relative to, or 0 when they are not
/// (everything in range is then listed as new). `ack_input_seq` is the newest input of the
/// *receiving* player that the state already includes. `ride` is the id of the vehicle the
/// receiving player stands on top of, or 0. It is carried with that vehicle, so the client must
/// predict relative to it. `drive` is `nil` unless the receiving player is driving. `self` is the
/// receiving player's own health and hotbar. `shots` are the gunshots since the previous
/// snapshot, newest last, capped at `MAX_SHOT_EVENTS`. `pickups` are the items on the ground in range.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub tag: u8,
    pub tick: u32,
    pub baseline: u32,
    pub ack: u32,
    pub ride: u32,
    pub new_entities: Vec<EntityState>,
    pub entities: Vec<EntityDelta>,
    pub new_vehicles: Vec<VehicleState>,
    pub vehicles: Vec<VehicleDelta>,
    pub removed: Vec<u32>,
    pub drive: Option<DriveState>,
    pub me: SelfState,
    pub shots: Vec<ShotEvent>,
    pub rockets: Vec<ProjectileState>,
    pub pickups: Vec<PickupState>,
    /// Bullets fired since a few snapshots ago, which the client then flies itself.
    pub bullets: Vec<BulletSpawn>,
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

/// `[6, [[kind, col, row, index, yaw], ...]]`, sent on a reliable stream: to everyone when
/// buildings or trees are destroyed, and to a player who just joined with all that already are.
/// `kind` is `OBJ_*`; `(col, row, index)` is the object's `ObjId`; `yaw` (milliradians, as
/// elsewhere) is the way a tree falls, or the direction a blast came from for a building.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Destroyed(pub u8, pub Vec<(u8, u16, u16, u32, i16)>);

/// `[7, [[col, row, index, stage, marked, x, y, z], ...]]`, sent on a reliable stream: to everyone
/// when buildings are hurt but still stand, and to a player who just joined with every hurt one.
/// `(col, row, index)` is the building's `ObjId`; `stage` is `0..=DAMAGE_STAGES` (higher is worse,
/// the last is about to collapse). When `marked` is 1, a blast landed at `(x, y, z)` (cm, as in
/// `ShotEvent`) and leaves a scorch mark there; otherwise the position is zero.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Damaged(pub u8, pub Vec<DamagedEntry>);

/// One `Damaged` entry: `(col, row, index, stage, marked, x, y, z)`.
pub type DamagedEntry = (u16, u16, u32, u8, u8, i32, i32, i32);

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
        148, 0, 22, 205, 4, 206, 146, 152, 41, 127, 0, 205, 6, 35, 0, 205, 1, 44, 205, 238, 72, 0, 152, 42, 208, 129, 127, 209, 248, 98, 11, 209, 255, 106, 7, 3,
    ];

    pub const SNAPSHOT_FIXTURE: &[u8] = &[
        220, 0, 16, 1, 205, 4, 210, 205, 4, 206, 42, 9, 145, 154, 7, 0, 0, 85, 206, 0, 1, 134, 160, 0, 0, 3, 0, 0, 145, 153, 1, 25, 253, 40, 208, 136, 205, 2, 138, 1, 209, 254, 212, 12, 146, 158, 10, 2, 205, 1, 44, 90, 209, 248, 48, 0, 205, 2, 188, 0, 32, 146, 1, 7, 148, 209, 253, 68, 204, 250, 3, 205, 4, 210, 0, 192, 192, 158, 12, 6, 209, 254, 12, 205, 1, 44, 205, 3, 32, 10, 236, 30, 0, 146, 4, 0, 148, 205, 11, 184, 204, 200, 9, 0, 70, 192, 149, 17, 204, 200, 0, 3, 3, 145, 157, 9, 100, 0, 208, 211, 3, 254, 0, 4, 148, 1, 0, 7, 0, 192, 100, 152, 0, 0, 40, 0, 0, 0, 204, 255, 3, 192, 146, 3, 11, 152, 120, 251, 209, 246, 60, 30, 209, 254, 112, 15, 204, 250, 209, 253, 168, 147, 73, 204, 128, 147, 147, 2, 1, 17, 147, 0, 0, 0, 147, 3, 2, 0, 146, 149, 7, 0, 205, 5, 220, 204, 160, 209, 239, 152, 149, 1, 0, 236, 90, 205, 121, 24, 145, 152, 40, 1, 205, 4, 176, 204, 150, 209, 252, 224, 205, 11, 184, 0, 209, 246, 60, 145, 149, 60, 4, 209, 254, 212, 5, 205, 4, 226, 145, 154, 77, 7, 2, 120, 204, 165, 209, 244, 72, 205, 5, 220, 236, 209, 150, 136, 205, 4, 209
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
        Snapshot { tag: S_SNAPSHOT, tick: 1234, baseline: 1230, ack: 42, ride: 9, new_entities: vec![EntityState { id: 7, kind: KIND_STICKMAN, x: 0, y: 85, z: 100_000, yaw: 0, vy: 0, flags: FLAG_GROUNDED | FLAG_DEAD, knock_x: 0, knock_z: 0 }], entities: vec![EntityDelta { id: 1, dx: 25, dy: -3, dz: 40, dyaw: -120, dvy: 650, flags: FLAG_GROUNDED, dknock_x: -300, dknock_z: 12 }], new_vehicles: vec![VehicleState { id: 10, kind: KIND_TANK, x: 300, y: 90, z: -2000, qx: 0, qy: 700, qz: 0, steer: 32, seats: vec![1, 7], turret: Some(Turret { yaw: -700, pitch: 250, shots: 3, range: 1234 }), hp: 0, damage: None, arms: None }, VehicleState { id: 12, kind: KIND_PLANE, x: -500, y: 300, z: 800, qx: 10, qy: -20, qz: 30, steer: 0, seats: vec![4, 0], turret: Some(Turret { yaw: 3000, pitch: 200, shots: 9, range: 0 }), hp: 70, damage: None, arms: Some(Arms { shots: 17, heat: 200, rear_heat: 0, bombs: 3, flags: ARMS_CUT | ARMS_JAMMED }) }], vehicles: vec![VehicleDelta { id: 9, dx: 100, dy: 0, dz: -45, dqx: 3, dqy: -2, dqz: 0, dsteer: 4, seats: Some(vec![1, 0, 7, 0]), turret: None, hp: 100, damage: Some([0, 0, 40, 0, 0, 0, 255, 3]), arms: None }], removed: vec![3, 11], drive: Some(DriveState { vx: 120, vy: -5, vz: -2500, wx: 30, wy: -400, wz: 15, steer: 250, spin: -600 }), me: SelfState { hp: 73, progress: 128, slots: vec![ItemSlot { item: ITEM_AK5, count: 1, ammo: 17 }, ItemSlot { item: ITEM_NONE, count: 0, ammo: 0 }, ItemSlot { item: ITEM_GRENADE, count: 2, ammo: 0 }] }, shots: vec![ShotEvent { shooter: 7, weapon: WEAPON_RIFLE, x: 1500, y: 160, z: -4200 }, ShotEvent { shooter: 1, weapon: WEAPON_RIFLE, x: -20, y: 90, z: 31_000 }], rockets: vec![ProjectileState { id: 40, kind: PROJECTILE_GRENADE, x: 1200, y: 150, z: -800, vx: 3000, vy: 0, vz: -2500 }], pickups: vec![PickupState { id: 60, item: ITEM_BANDAGE, x: -300, y: 5, z: 1250 }], bullets: vec![BulletSpawn { id: 77, shooter: 7, item: ITEM_AK5, x: 120, y: 165, z: -3000, vx: 1500, vy: -20, vz: -27000, tick: 1233 }] }
    }

    /// `[6, [[OBJ_TREE, 2, 3, 70000, -1571]]]`, mirrored in `codec.spec.ts`.
    pub const DESTROYED_FIXTURE: &[u8] = &[146, 6, 145, 149, 0, 2, 3, 206, 0, 1, 17, 112, 209, 249, 221];

    /// `[7, [[2, 3, 70000, 3, 1, 1000, 20, -1000]]]`, mirrored in `destruction.spec.ts`.
    pub const DAMAGED_FIXTURE: &[u8] = &[146, 7, 145, 152, 2, 3, 206, 0, 1, 17, 112, 3, 1, 205, 3, 232, 20, 209, 252, 24];

    #[test]
    fn damaged_message_matches_the_fixture() {
        let message = Damaged(S_DAMAGED, vec![(2, 3, 70_000, 3, 1, 1000, 20, -1000)]);
        assert_eq!(encode(&message), DAMAGED_FIXTURE);
        assert_eq!(decode::<Damaged>(DAMAGED_FIXTURE), Some(message));
        assert_eq!(peek_tag(DAMAGED_FIXTURE), Some(S_DAMAGED));
    }

    #[test]
    fn hit_round_trips() {
        let message = Hit { tag: S_HIT, role: HIT_TAKEN, kind: HIT_BODY, amount: 25, weapon: WEAPON_RIFLE, bearing: -1571, x: 100, y: 90, z: -250, target: 7 };
        assert_eq!(decode::<Hit>(&encode(&message)), Some(message));
        assert_eq!(peek_tag(&encode(&Hit { tag: S_HIT, role: HIT_DEALT, kind: HIT_KILL, amount: 100, weapon: WEAPON_SNIPER, bearing: BEARING_NONE, x: 0, y: 0, z: 0, target: 0 })), Some(S_HIT));
    }

    #[test]
    fn destroyed_matches_fixture() {
        let message = Destroyed(S_DESTROYED, vec![(OBJ_TREE, 2, 3, 70_000, -1571)]);
        assert_eq!(encode(&message), DESTROYED_FIXTURE);
        assert_eq!(decode::<Destroyed>(DESTROYED_FIXTURE), Some(message));
        assert_eq!(peek_tag(DESTROYED_FIXTURE), Some(S_DESTROYED));
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
    }

    #[test]
    fn peek_rejects_garbage() {
        assert_eq!(peek_tag(&[]), None);
        assert_eq!(peek_tag(&[0x90]), None);
        assert_eq!(peek_tag(&[0x92, 0xcc, 0x01]), None);
    }
}
