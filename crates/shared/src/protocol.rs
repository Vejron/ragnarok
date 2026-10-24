//! Wire protocol: MessagePack arrays with a leading tag byte. Field names are never sent.
//!
//! The structs have named fields for the code's sake, but `rmp_serde::to_vec` writes a struct as an
//! array of its fields in declaration order, so **the order of the fields is the wire format**:
//! never reorder or insert one without bumping `PROTOCOL_VERSION`.
//!
//! Every message is listed once, in the `messages!` invocation below. The client's codec is generated
//! from it (`client/src/net/messages.generated.ts`, refreshed with `UPDATE_PARITY=1 cargo test -p
//! shared`), with the bytes of every message as Rust encodes them (`messages.fixture.json`), so a new
//! message is one entry there. See NETWORKING.md for the full table.

use crate::replicated::Update;
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
pub const S_BUILT: u8 = 13;

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
/// Falling masonry: cells of a building that lost their hold, landing on someone. Only ever the weapon of a kill.
pub const WEAPON_RUBBLE: u8 = 23;

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

/// A message's field: its name, its Rust type and its unit (`UNITS`), or `""` for a count, a set of
/// bits, an id or text, which have none.
pub type Field = (&'static str, &'static str, &'static str);

/// What a field's integer counts, and so how the client turns it into its own units:
///
/// - `cm`: centimetres (or cm/s), metres on the client.
/// - `mrad`: milliradians, radians on the client.
/// - `byte`: 0..=255 for 0..=1.
/// - `fifth`: fifths of a metre a second, m/s on the client.
pub const UNITS: &[&str] = &["cm", "mrad", "byte", "fifth"];

/// One message, or one struct of a message's lists, as `messages!` lists it: its name, its tags (the
/// constants that name it on the wire, `C_*` to the server and `S_*` from it; none for a struct) and its
/// fields after the tag, in wire order.
#[derive(Clone, Copy, Debug)]
pub struct Message {
    pub name: &'static str,
    pub tags: &'static [(&'static str, u8)],
    pub fields: &'static [Field],
}

/// One message's struct: a leading `tag` when it has tags.
macro_rules! message {
    ([] $(#[$meta:meta])* $Name:ident { $( $(#[$fmeta:meta])* $f:ident : $t:ty ),* }) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct $Name { $( $(#[$fmeta])* pub $f: $t, )* }
    };
    ([$($tag:ident),+] $(#[$meta:meta])* $Name:ident { $( $(#[$fmeta:meta])* $f:ident : $t:ty ),* }) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct $Name {
            /// Which message it is: one of its tags.
            pub tag: u8,
            $( $(#[$fmeta])* pub $f: $t, )*
        }
    };
}

/// Every message and the structs of their lists, and the `MESSAGES` schema the client's codec is
/// generated from (`client/src/net/messages.generated.ts`). Each is a MessagePack array of its fields
/// in the order listed, after its tag.
macro_rules! messages {
    ($( $(#[$meta:meta])* [$($tag:ident),*] $Name:ident { $( $(#[$fmeta:meta])* $f:ident : $t:ty $(as $unit:ident)? ),* $(,)? } )*) => {
        $( message!([$($tag),*] $(#[$meta])* $Name { $( $(#[$fmeta])* $f : $t ),* }); )*

        /// Every message and struct, in the order `messages!` lists them.
        pub const MESSAGES: &[Message] = &[$(
            Message { name: stringify!($Name), tags: &[$( (stringify!($tag), $tag) ),*], fields: &[$( (stringify!($f), stringify!($t), stringify!($($unit)?)) ),*] }
        ),*];
    };
}

messages! {
    /// A bullet leaving a gun, in centimetres and cm/s. Sent once (in two snapshots, in case one is
    /// lost) and not listed again: the path is deterministic, so a client works out where the bullet is
    /// from `tick` on. It moves `v * dt` a tick and then `v.y` loses `GRAVITY * BULLET_GRAVITY_SCALE *
    /// dt`, so after `n` ticks it is at `p + v * n * dt - (0, g * dt^2 * n * (n + 1) / 2, 0)` with `g =
    /// GRAVITY * BULLET_GRAVITY_SCALE`. It ends with a `ShotEvent` of `WEAPON_BULLET` carrying `id`, or
    /// when `item`'s range runs out. `item` is the gun, so a client knows its range and look. `tick` is
    /// the server tick it was fired on.
    #[derive(Copy, Eq)]
    [] BulletSpawn { id: u32, shooter: u32, item: u8, x: i32 as cm, y: i32 as cm, z: i32 as cm, vx: i16 as cm, vy: i16 as cm, vz: i16 as cm, tick: u32 }

    /// One simulation tick of player intent. The client predicts with exactly these quantized values,
    /// so client and server feed the simulation identical inputs.
    #[derive(Copy, Default, Eq)]
    [] InputFrame {
        /// Monotonic per-connection sequence number, starting at 1. One frame per client tick.
        seq: u32,
        /// Strafe axis, -127 (left) ..= 127 (right).
        strafe: i8,
        /// Forward axis, -127 (back) ..= 127 (forward).
        forward: i8,
        /// Facing in milliradians, `[-π, π)`. 0 faces -Z.
        yaw: i16,
        /// `BUTTON_*` bits.
        buttons: u16,
        /// Where the player looks up or down, milliradians (+ is up). Used by a tank gunner and the rifle.
        pitch: i16,
        /// The server tick this frame's picture showed, modulo 65536: the interpolated world the player was looking at when they pulled the trigger. The server rewinds other players to it.
        view: u16,
        /// The hotbar slot the player has selected, `0..HOTBAR_SLOTS`.
        slot: u8,
    }

    /// The newest `INPUT_REDUNDANCY` frames, oldest first. `baseline` is the tick of the newest snapshot
    /// the client has rebuilt, or 0 for none: the server diffs the next snapshot against it.
    [C_INPUT] ClientInput { version: u8, baseline: u32, frames: Vec<InputFrame> }

    /// A latency probe, echoed as a `Pong`.
    [C_PING] Ping { nonce: u32 }

    /// Sent once on a reliable stream.
    [S_WELCOME] Welcome { version: u8, id: u32, tick_rate: u8, snapshot_every: u8, world_hash: u32 }

    /// Something that just happened, for drawing; `x, y, z` is where it ended. A rifle shot is a tracer
    /// from the shooter's own position to there; a rocket or a wreck is a burst at there (the shooter is
    /// 0 for a wreck with no culprit).
    #[derive(Copy, Eq)]
    [] ShotEvent { shooter: u32, weapon: u8, x: i32 as cm, y: i32 as cm, z: i32 as cm }

    /// Sent as a datagram. `baseline` is the tick of the snapshot the updates are relative to, or 0
    /// when they are not (every entity in range then comes whole). Each update is one entity's
    /// components (`shared::replicated::Update`): stickmen, creatures, vehicles, projectiles and items
    /// on the ground alike. `ack` is the newest input of the *receiving* player that the state already
    /// includes. `ride` is the id of the vehicle the receiving player stands on top of, or 0: it is
    /// carried with that vehicle, so the client must predict relative to it. What only the receiving
    /// player is told (their health and hotbar, the exact motion of what they drive) are `[owner]`
    /// components of their own entity and their vehicle's, among the updates. `shots` are the effects
    /// since the previous snapshot, capped at `MAX_SHOT_EVENTS`, and `bullets` the gun rounds fired
    /// since a few snapshots ago, which the client then flies itself.
    [S_SNAPSHOT] Snapshot { tick: u32, baseline: u32, ack: u32, ride: u32, updates: Vec<Update>, removed: Vec<u32>, shots: Vec<ShotEvent>, bullets: Vec<BulletSpawn> }

    [S_PONG] Pong { nonce: u32 }

    /// Sent on a reliable stream when a player leaves.
    [S_LEAVE] Leave { id: u32 }

    /// Sent on a reliable stream to everyone. `killer` is 0 when the world did it (a fall); the totals
    /// are the new ones, so clients keep no tally of their own. `style` is `DEATH_*`; `yaw` (0 faces
    /// -Z, as for a look) is the way the body is thrown and `power` how hard.
    #[derive(Eq)]
    [S_KILL] Kill { killer: u32, victim: u32, weapon: u8, killer_kills: u16, victim_deaths: u16, style: u8, yaw: i16 as mrad, power: u8 as fifth }

    #[derive(Eq)]
    [] Score { id: u32, kills: u16, deaths: u16 }

    /// Sent on a reliable stream to a player who just joined.
    #[derive(Eq)]
    [S_SCORES] Scores { scores: Vec<Score> }

    /// What a player chose. `color` indexes `PLAYER_COLOR_COUNT` colours. `team` is 0 for people, who
    /// are all against all; players on the same other team are on one side. `bot` is a bot (see BOTS.md).
    #[derive(Eq)]
    [] ProfileEntry { id: u32, name: String, color: u8, team: u8, bot: bool }

    /// Sent on a reliable stream: to a player who just joined with everyone's, and to everyone else with
    /// the joiner's (a bot's when it is added).
    #[derive(Eq)]
    [S_PROFILES] Profiles { players: Vec<ProfileEntry> }

    /// Sent on a reliable stream to everyone when a world's scenario has something to say (see
    /// BOTS.md). `kind` is `NOTICE_*`.
    #[derive(Eq)]
    [S_NOTICE] Notice { kind: u8, text: String }

    /// A building or tree that is gone. `kind` is `OBJ_*`; `(col, row, index)` is the object's `ObjId`;
    /// `yaw` is the way a tree falls, or the direction a blast came from for a building; `force` how
    /// violently it went (0 for a joiner's catch-up).
    #[derive(Eq)]
    [] DestroyedObject { kind: u8, col: u16, row: u16, index: u32, yaw: i16 as mrad, force: u8 as byte }

    /// Sent on a reliable stream: to everyone when buildings or trees are destroyed, and to a player who
    /// just joined with all that already are.
    #[derive(Eq)]
    [S_DESTROYED] Destroyed { objects: Vec<DestroyedObject> }

    /// A building that is hurt but stands. `(col, row, index)` is its `ObjId`; `stage` is
    /// `0..=DAMAGE_STAGES` (higher is worse, the last is about to collapse); `mark` is where a blast
    /// landed and left a scorch mark, if one did.
    #[derive(Eq)]
    [] DamagedBuilding { col: u16, row: u16, index: u32, stage: u8, mark: Option<[i32; 3]> as cm }

    /// Sent on a reliable stream: to everyone when buildings are hurt but still stand, and to a player
    /// who just joined with every hurt one.
    #[derive(Eq)]
    [S_DAMAGED] Damaged { buildings: Vec<DamagedBuilding> }

    /// Cells a blast took out of a standing building. `(col, row, index)` is the building's `ObjId`;
    /// `point` is where the blast landed; `blasted` are the cells it broke and `fell` the cells it left
    /// unsupported (`carve::Shell::index`).
    #[derive(Eq)]
    [] CarvedBuilding { col: u16, row: u16, index: u32, point: [i32; 3] as cm, blasted: Vec<u32>, fell: Vec<u32> }

    /// Sent on a reliable stream: to everyone when a blast takes cells out of a standing building, and
    /// to a player who just joined with every cell already gone (`live` false, so it is state and not
    /// news). A joiner's catch-up lists every removed cell under `blasted`.
    #[derive(Eq)]
    [S_CARVED] Carved { live: bool, buildings: Vec<CarvedBuilding> }

    /// A cube placed with bricks: its place on the world lattice (`construct::Pos`, so its corner is
    /// `CELL` centimetres times it), its colour, the builder's (`PLAYER_COLOR_COUNT` colours), and its
    /// material (`construct::MATERIALS`).
    #[derive(Copy, Eq)]
    [] BuiltCube { x: i32, y: i32, z: i32, color: u8, material: u8 }

    /// A place on the world lattice of cubes (`construct::Pos`).
    #[derive(Copy, Eq)]
    [] CubePos { x: i32, y: i32, z: i32 }

    /// Sent on a reliable stream: to everyone when cubes are placed, dug out, blasted or fall, and to
    /// a player who just joined with every cube there is (`live` false, so it is state and not news).
    /// `placed` are new cubes; `removed` the cubes dug out or blasted, and `fell` those left with
    /// nothing holding them up, which come down; both as lattice places (`construct::Pos`).
    #[derive(Eq)]
    [S_BUILT] Built { live: bool, placed: Vec<BuiltCube>, removed: Vec<CubePos>, fell: Vec<CubePos> }

    /// Sent on a reliable stream to the one player it is about, once per tick per `(role, kind)` with
    /// the amounts summed. `role` is `HIT_*`: dealt (you hurt something) or taken (something hurt you
    /// or your vehicle); `kind` is `HIT_*` too; `amount` is the health lost; `weapon` is `WEAPON_*`. For
    /// a taken hit `bearing` is the yaw (0 faces -Z, as for a look) from you towards where it came
    /// from, if it came from anywhere. `(x, y, z)` is about where the thing hit is, chest height for a
    /// stickman, and `target` is its id, so clients can show a number and a flash on it.
    #[derive(Eq)]
    [S_HIT] Hit { role: u8, kind: u8, amount: u8, weapon: u8, bearing: Option<i16> as mrad, x: i32 as cm, y: i32 as cm, z: i32 as cm, target: u32 }

    /// WebRTC signaling for proximity voice, relayed by the server and never interpreted. Client →
    /// server (`C_RTC`, on a reliable stream) `peer` is the player it is for; server → client (`S_RTC`,
    /// on the reliable stream) `peer` is the player it is from, which the server fills in, so it cannot
    /// be forged. `kind` is `RTC_*` and `json` the session description or ICE candidate as a JSON
    /// string. The voice itself goes peer to peer.
    #[derive(Eq)]
    [C_RTC, S_RTC] Rtc { peer: u32, kind: u8, json: String }
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

/// One hotbar slot: `[item, count, ammo]`. `item` is `ITEM_NONE` for an empty slot; `ammo` is the
/// rounds left in the magazine of a gun.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemSlot {
    pub item: u8,
    pub count: u8,
    pub ammo: u8,
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
    pub updates: &'a [Update],
    pub removed: &'a [u32],
    pub shots: &'a [ShotEvent],
    pub bullets: &'a [BulletSpawn],
}

/// `Notice` kinds: what a scenario is telling everyone.
/// A message, shown for a few seconds.
pub const NOTICE_SAY: u8 = 0;
/// What to do now, shown until the next one; empty clears it. A joiner gets the current one.
pub const NOTICE_OBJECTIVE: u8 = 1;
/// The scenario is over: won, or lost. It starts again a little later.
pub const NOTICE_WON: u8 = 2;
pub const NOTICE_LOST: u8 = 3;

/// `RTC_*` kinds: what an `Rtc` payload is.
pub const RTC_OFFER: u8 = 0;
pub const RTC_ANSWER: u8 = 1;
pub const RTC_ICE: u8 = 2;
/// Longest `Rtc` payload the server relays, bytes. A voice-only SDP is under 2 KiB.
pub const RTC_MAX_PAYLOAD: usize = 8192;

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
mod tests;
