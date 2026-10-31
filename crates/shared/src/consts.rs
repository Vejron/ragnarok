//! Simulation and network constants. `client/scripts/gen-consts.ts` generates the client's copy from this file: run `npm run gen:consts` in `client` after editing.

pub use crate::vehicles::drone::*;
pub use crate::vehicles::heli::*;
pub use crate::vehicles::hen::*;
pub use crate::vehicles::tupp::*;
pub use crate::vehicles::jeep::*;
pub use crate::vehicles::loiter::*;
pub use crate::vehicles::mortar::*;
pub use crate::vehicles::pak::*;
pub use crate::vehicles::plane::*;
pub use crate::vehicles::roadster::*;
pub use crate::vehicles::sports::*;
pub use crate::vehicles::strv::*;
pub use crate::vehicles::tank::*;
pub use crate::vehicles::walker::*;

/// Wire schema version. Bumped on any incompatible protocol or world-format change.
pub const PROTOCOL_VERSION: u8 = 60;

/// Entity kinds, the `kind` component: which model a client draws. A vehicle kind's definition is
/// `crate::vehicles`.
pub const KIND_SOLDIER: u8 = 0;
/// A Hedemora rooster, an NPC. Replicated like a soldier, with its own model.
pub const KIND_ROOSTER: u8 = 5;
/// Vehicle kinds: replicated with an `orientation`.
pub const KIND_JEEP: u8 = 1;
pub const KIND_TANK: u8 = 2;
pub const KIND_HELI: u8 = 3;
pub const KIND_SPORTS: u8 = 4;
/// The triplane. 5 is the rooster's.
pub const KIND_PLANE: u8 = 6;
/// The two-legged walker. 5 is the rooster's.
pub const KIND_WALKER: u8 = 7;
/// The turretless Strv S tank, one seat.
pub const KIND_STRV: u8 = 8;
/// The towed field mortar, one seat for its gunner.
pub const KIND_MORTAR: u8 = 9;
/// The Tesla Roadster, a sports car with cold-gas thrusters: boost, hop and hover. Two seats.
pub const KIND_ROADSTER: u8 = 10;
/// An FPV drone, flown from afar by the player who launched it. Gone once it blows up or its pilot lets go.
pub const KIND_DRONE: u8 = 11;
/// The hen: a giant robot hen on the walker's legs that jumps, flaps and glides, and lays egg bombs. One seat.
pub const KIND_HEN: u8 = 12;
/// A loitering drone: a Shahed-sized delta wing with a big warhead, flown from afar like an FPV drone
/// but launched high above and behind its pilot, and always cruising forward.
pub const KIND_LOITER: u8 = 13;
/// The Pak 38: a towed anti-tank gun with a shield and one gunner, who lays it like a tank's gun.
pub const KIND_PAK: u8 = 14;
/// The Intinor Compact Backpack lying on the ground (see `BACKPACK_*`). Replicated as a place and this kind.
pub const KIND_BACKPACK: u8 = 15;
/// The tupp: a giant robot Hedemora rooster on the hen's legs and wings, with laser eyes. One seat.
pub const KIND_TUPP: u8 = 16;

/// Fixed simulation rate, in ticks per second.
pub const TICK_RATE: u32 = 60;
/// Seconds per simulation tick.
pub const TICK_DT: f32 = 1.0 / TICK_RATE as f32;
/// A snapshot is sent every this many ticks (60 / 2 = 30 Hz).
pub const SNAPSHOT_EVERY: u32 = 2;

/// Players admitted to the single room. Interest management and the packet budget (`MAX_PACKET`)
/// keep a snapshot within one datagram however many of them are in view.
pub const MAX_PLAYERS: usize = 64;
/// Outfit colours a player can pick from (`client/src/profile.ts` holds the colours themselves).
pub const PLAYER_COLOR_COUNT: u8 = 12;
/// Spawn points a player may choose, used in turn each time they spawn.
pub const MAX_SPAWNS: usize = 5;
/// Longest player name, in characters.
pub const MAX_NAME_CHARS: usize = 16;
/// Datagrams above this size are dropped unread. Stays under a typical QUIC datagram MTU.
pub const MAX_PACKET: usize = 1200;
/// A player is sent the entities, vehicles and effects within this many metres of their soldier
/// (measured on the ground plane), and keeps them until they are `INTEREST_HYSTERESIS` further out,
/// so something at the edge does not flicker in and out.
pub const INTEREST_RADIUS: f32 = 300.0;
pub const INTEREST_HYSTERESIS: f32 = 40.0;
/// Snapshots the server remembers per player to diff against, and the client remembers to
/// rebuild from. The client keeps at least as many as the server, so any baseline the server
/// still offers is one the client still has. At 30 Hz, 32 is about a second.
pub const BASELINE_WINDOW: usize = 32;
pub const BASELINE_WINDOW_CLIENT: usize = 64;
/// Each input datagram repeats this many of the most recent frames, so a lost packet costs nothing.
pub const INPUT_REDUNDANCY: usize = 4;

/// Input buttons, as bits of `InputFrame::buttons`.
pub const BUTTON_JUMP: u16 = 1 << 0;
pub const BUTTON_RUN: u16 = 1 << 1;
pub const BUTTON_USE: u16 = 1 << 2;
/// Gunner's trigger. Held to keep firing; the gun's reload paces it.
pub const BUTTON_FIRE: u16 = 1 << 3;
/// Aim held (right mouse): looks down the scope of a gun that has one, and tightens any gun's spread.
pub const BUTTON_ALT: u16 = 1 << 4;
/// Held to crouch: slower, and a lower eye.
pub const BUTTON_CROUCH: u16 = 1 << 5;
/// One press, sent with run and crouch held: starts a slide. Like a jump it is not repeated.
pub const BUTTON_SLIDE: u16 = 1 << 6;
/// One press: reload the selected gun. Like a jump it is not repeated.
pub const BUTTON_RELOAD: u16 = 1 << 7;
/// Held to lie down: slower, and a low eye. Lets go to stand again, if there is room overhead.
pub const BUTTON_PRONE: u16 = 1 << 8;
/// One press: hitch a free mortar to the car in front of you, or unhitch the one it tows. Like a jump it is not repeated.
pub const BUTTON_HITCH: u16 = 1 << 9;
/// Held by a mortar's gunner to walk the range dial out and in (W and S).
pub const BUTTON_RANGE_UP: u16 = 1 << 10;
pub const BUTTON_RANGE_DOWN: u16 = 1 << 11;
/// One press: open or cut the parachute while in the air. Like a jump it is not repeated.
pub const BUTTON_CHUTE: u16 = 1 << 12;
/// One press, at a gun that has two kinds of round: load the other one (Q). Like a jump it is not repeated.
pub const BUTTON_AMMO: u16 = 1 << 13;

/// Soldier movement, in metres and seconds.
pub const WALK_SPEED: f32 = 4.0;
pub const RUN_SPEED: f32 = 7.5;
/// Crouching overrides running.
pub const CROUCH_SPEED: f32 = 2.0;
/// Lying down overrides both: crawling.
pub const PRONE_SPEED: f32 = 1.0;
/// A slide is a burst of knockback along the run direction that bleeds off slowly while crouch is held.
pub const SLIDE_SPEED: f32 = 12.0;
pub const SLIDE_KEEP: f32 = 0.96;
/// A new slide can start once the last has slowed below this, in m/s.
pub const SLIDE_RETRIGGER: f32 = 1.0;
/// Above this much leftover velocity, a grounded crouching player is sliding.
pub const SLIDE_MIN_SPEED: f32 = 2.0;
/// A rested jump's take-off speed, m/s: 1.6 times the height of a winded one (`JUMP_SPEED_WINDED`), as height goes by its square.
pub const JUMP_SPEED: f32 = 8.22;
pub const JUMP_SPEED_WINDED: f32 = 6.5;
/// Stamina on foot, in whole points so the server and the prediction agree on it exactly. Running on the
/// ground spends `STAMINA_RUN_DRAIN` a tick (a full bar lasts 5 s) and a jump `STAMINA_JUMP_COST`;
/// otherwise, with a foot down, `STAMINA_REFILL` a tick comes back. In the air it neither drains nor fills.
pub const STAMINA_FULL: u16 = 6000;
pub const STAMINA_RUN_DRAIN: u16 = 20;
pub const STAMINA_JUMP_COST: u16 = 900;
pub const STAMINA_REFILL: u16 = 15;
/// Once it runs out the player is winded: running is walking and a jump is a `JUMP_SPEED_WINDED` one,
/// until this much has come back.
pub const STAMINA_RECOVER: u16 = 1800;
pub const GRAVITY: f32 = 20.0;
/// Knockback (a car hitting a soldier) is a horizontal velocity added to the walking velocity.
/// Each tick it keeps this fraction of itself: quickly on the ground, barely in the air.
pub const KNOCK_KEEP_GROUND: f32 = 0.92;
pub const KNOCK_KEEP_AIR: f32 = 0.99;
/// Below this speed (m/s) knockback is dropped, so it always dies out exactly.
pub const KNOCK_MIN_SPEED: f32 = 0.05;
/// Fastest knockback the wire can carry and the server will give, m/s.
pub const KNOCK_MAX_SPEED: f32 = 20.0;
/// Terminal fall speed, so `vy` always fits its i16 cm/s wire field.
pub const MAX_FALL_SPEED: f32 = 50.0;
/// Parachute. An open canopy settles the fall at this speed, m/s; a faster fall is braked at `CHUTE_DRAG` m/s².
pub const CHUTE_DESCENT: f32 = 5.0;
pub const CHUTE_DRAG: f32 = 40.0;
/// Glide speed under a canopy: full forward, backward (braking) and sideways, m/s.
pub const CHUTE_FORWARD: f32 = 15.0;
pub const CHUTE_BACK: f32 = 3.0;
pub const CHUTE_SIDE: f32 = 8.0;
/// How much of the gap to the wanted glide velocity is closed each tick: a canopy turns lazily.
pub const CHUTE_STEER: f32 = 0.03;
/// Respawns drop in from this far above the ground, metres, with the canopy open.
pub const SPAWN_ALTITUDE: f32 = 100.0;

/// Soldier collision capsule. Total height is `2 * (half_height + radius)` = 1.7 m, with the
/// body origin at the capsule centre.
pub const CAPSULE_HALF_HEIGHT: f32 = 0.5;
pub const CAPSULE_RADIUS: f32 = 0.35;
/// Lying down, the body is a ball of this radius (0.6 m across) resting on the ground. Its bottom is
/// where the standing capsule's is, so going prone drops the centre by `STAND_CENTRE - PRONE_CENTRE`.
pub const PRONE_RADIUS: f32 = 0.3;
/// Capsule-centre height above the feet, standing and prone.
pub const STAND_CENTRE: f32 = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS;
pub const PRONE_CENTRE: f32 = PRONE_RADIUS;
/// Stance, as `replicated::Flags::stance` on the wire. Crouching is a held button, not a stance.
pub const STANCE_STANDING: u8 = 0;
pub const STANCE_PRONE: u8 = 1;
/// Hanging under an open parachute.
pub const STANCE_CHUTE: u8 = 2;
/// Knocked down by a blast: thrown, then lying helpless until `Character::down` runs out (KNOCKDOWN.md).
pub const STANCE_DOWN: u8 = 3;
/// A blast that throws a soldier at least this hard, m/s, knocks them down.
pub const KNOCKDOWN_POWER: f32 = 7.0;
/// Ticks they lie there once landed: this many at `KNOCKDOWN_POWER`, rising evenly to
/// `KNOCKDOWN_MAX_TICKS` at `KNOCK_MAX_SPEED`.
pub const KNOCKDOWN_MIN_TICKS: u8 = 48;
pub const KNOCKDOWN_MAX_TICKS: u8 = 150;
/// A round that hits above this height over the capsule centre hit the head.
pub const HEAD_ZONE_Y: f32 = 0.45;
/// Body-part health. A soldier's torso has `PLAYER_MAX_HP`, its head `HEAD_HP` and each limb `LIMB_HP`.
/// Only the torso or the head running out kills; a limb that runs out comes off, and what the round had
/// left over is lost. A sniper round to the head kills, as do three revolver rounds or seven AK5 rounds.
pub const HEAD_HP: u8 = 75;
pub const LIMB_HP: u8 = 60;

/// Limbs. A round below this height over the capsule centre hit a leg (the hips are about 0.75 m up),
/// and one between there and the head this far or more to the side of the body's middle (the outer
/// edge of the chest), an arm.
pub const HIP_ZONE_Y: f32 = -0.1;
pub const ARM_ZONE_X: f32 = 0.25;
/// How far out a round finds a soldier from the middle of their body: wider than the 0.35 m they
/// move with, for the arms as they are drawn.
pub const HURT_RADIUS: f32 = 0.43;
/// One leg: each step is a hop up at this speed, m/s, at this share of walking speed; no running or sliding.
pub const HOP_SPEED: f32 = 2.6;
pub const HOP_SCALE: f32 = 0.55;
/// One leg: a jump's take-off speed.
pub const HOP_JUMP_SPEED: f32 = 4.5;
/// No legs: always lying down, crawling at this share of `PRONE_SPEED`.
pub const LEGLESS_CRAWL_SCALE: f32 = 0.7;
/// One arm: a gun's spread grows by this, radians (aiming no longer steadies it), and a reload takes this many times as long.
pub const ONE_ARM_SPREAD: f32 = 0.035;
pub const ONE_ARM_RELOAD: u32 = 2;
/// One arm: how far the aim wanders, radians, as the client sways the look it sends.
pub const ONE_ARM_SWAY: f32 = 0.03;

/// Character controller tuning. The client builds its Rapier controller from the same values.
pub const CONTROLLER_OFFSET: f32 = 0.02;
pub const MAX_SLOPE_CLIMB_DEG: f32 = 50.0;
pub const AUTOSTEP_HEIGHT: f32 = 0.35;
pub const AUTOSTEP_MIN_WIDTH: f32 = 0.2;
/// How far below its feet a standing character is pulled back onto the ground when it walks off
/// a slope, metres. Without it, running downhill is a series of small falls.
pub const SNAP_DISTANCE: f32 = 0.3;

/// Anything that falls below this is respawned (e.g. a player squeezed through a gap in the mesh).
pub const KILL_Y: f32 = -50.0;

/// Vehicles. Chassis and wheel geometry are shared with the client, which draws the car from them.
/// Body-local axes: +X right, +Y up, -Z forward (yaw 0 faces -Z, like a soldier).
/// Seats in the roomiest car (the jeep); bounds the seat list of any vehicle.
pub const CAR_SEATS: usize = 4;
/// Vehicles in the world; bounds the vehicle list in a snapshot. The map editor's placed vehicles and the
/// drones players launch share it, so it leaves room past the editor's full count for drones to launch into.
pub const MAX_VEHICLES: usize = 512;
/// A player can enter a vehicle whose centre is within this distance of their capsule centre.
pub const ENTER_RADIUS: f32 = 4.0;

/// Gravity acting on vehicle bodies. Soldiers use `GRAVITY`; the ray-cast suspension is tuned
/// around the real-world value.
pub const CAR_GRAVITY: f32 = 9.81;

/// Highest relief a world may have, metres from its lowest dry ground (y = 0) to its highest. The
/// importer refuses more; the boundary walls reach above it, and fliers find the terrain under them by
/// a ray from just over it.
pub const MAX_RELIEF: f32 = 1000.0;

pub const ROCKET_ACCEL: f32 = 110.0;
pub const ROCKET_MAX_SPEED: f32 = 170.0;
/// The blast of an autocannon round: a third of a tank shell's radius, a third of its damage.
pub const CANNON_RADIUS: f32 = EXPLOSION_RADIUS / 3.0;
pub const CANNON_DAMAGE: f32 = EXPLOSION_DAMAGE / 3.0;
pub const CANNON_KNOCK: f32 = EXPLOSION_KNOCK / 2.0;

pub const GUN_COOL: f32 = 1.0;
pub const GUN_UNJAM: f32 = 77.0;
pub const BOMB_RADIUS: f32 = 6.0;
pub const BOMB_DAMAGE: f32 = 110.0;
pub const BOMB_KNOCK: f32 = 16.0;

/// Craters (CRATERS.md): anything explosive that bursts on the ground dents it in a smooth bowl
/// `radius` metres across the way out and `depth` metres deep at the middle. The lighter blasts dig
/// about 0.8 of their blast radius, as the tank shell does.
pub const CRATER_SHELL_RADIUS: f32 = 5.6;
pub const CRATER_SHELL_DEPTH: f32 = 1.0;
pub const CRATER_MORTAR_RADIUS: f32 = 8.8;
pub const CRATER_MORTAR_DEPTH: f32 = 1.6;
pub const CRATER_BOMB_RADIUS: f32 = 7.0;
pub const CRATER_BOMB_DEPTH: f32 = 2.2;
pub const CRATER_ROCKET_RADIUS: f32 = 4.0;
pub const CRATER_ROCKET_DEPTH: f32 = 0.7;
pub const CRATER_GRENADE_RADIUS: f32 = 3.5;
pub const CRATER_GRENADE_DEPTH: f32 = 0.5;
pub const CRATER_CANNON_RADIUS: f32 = 2.0;
pub const CRATER_CANNON_DEPTH: f32 = 0.3;
pub const CRATER_EGG_RADIUS: f32 = 5.0;
pub const CRATER_EGG_DEPTH: f32 = 0.9;
/// A shell digs only if it bursts within this far above the terrain, metres: not on a roof or in the air.
pub const CRATER_REACH: f32 = 1.5;
/// However many craters land on a place, the ground goes no lower than this under what it was, metres.
pub const CRATER_MAX_DEPTH: f32 = 4.0;
/// The most craters whose marks a joiner is sent (the oldest are dropped first). Their heights are sent whole, in `Dug`.
pub const CRATER_MAX_KEPT: usize = 2000;
/// A crater's scorch fades out over this long, seconds; then it is forgotten. Its hole stays.
pub const CRATER_FADE_SECONDS: u32 = 30 * 60;

/// The tank gun.
pub const SHELL_COOLDOWN_TICKS: u32 = 90;
/// Speed a shell leaves the muzzle at, m/s. It falls under `GRAVITY` from there, so at 100 m it has
/// dropped about 5 m and at 200 m about 20.
pub const SHELL_SPEED: f32 = 140.0;
/// A sniper round flies at twice a tank shell's speed, m/s, and falls under a quarter of gravity, so
/// at 200 m (0.7 s) it has dropped about 1 m.
pub const BULLET_SPEED: f32 = 2.0 * SHELL_SPEED;
pub const BULLET_GRAVITY_SCALE: f32 = 0.25;
/// A gun fired with aim held scatters over this fraction of its hip-fire cone.
pub const AIM_SPREAD_SCALE: f32 = 0.25;
/// How long a shell flies before it bursts where it is.
pub const SHELL_TTL_TICKS: u32 = 6 * TICK_RATE;
/// Speed a fired shell takes from the tank, m/s, backwards along the barrel.
pub const SHELL_RECOIL: f32 = 1.6;

/// The tow point sits this far behind a car's rear face, metres.
pub const TOW_BACK: f32 = 0.25;
/// How close a mortar's tongue must be to a car's tow point to hitch, metres.
pub const HITCH_REACH: f32 = 2.5;
/// A hitch or an unhitch needs the car at a near standstill, m/s.
pub const HITCH_MAX_SPEED: f32 = 1.5;
pub const EXPLOSION_RADIUS: f32 = 7.0;
/// Knockback speed at the centre of a blast, m/s; it fades to zero at the edge.
pub const EXPLOSION_KNOCK: f32 = 18.0;

/// The backpack mode (BACKPACK.md): a world whose scenario places one Intinor Compact Backpack. Whoever
/// carries it walks `BACKPACK_SPEED` times as fast on foot and heals `BACKPACK_REGEN` health a second.
/// Its modems give it away: everyone's map shows a circle with it somewhere inside, `BACKPACK_WIDE`
/// metres across while it is new to its carrier (or lying about), narrowing to `BACKPACK_NARROW` over
/// `BACKPACK_NARROW_SECONDS` in one carrier's hands.
pub const BACKPACK_SPEED: f32 = 2.0;
pub const BACKPACK_REGEN: f32 = 5.0;
pub const BACKPACK_WIDE: f32 = 150.0;
pub const BACKPACK_NARROW: f32 = 20.0;
pub const BACKPACK_NARROW_SECONDS: f32 = 300.0;
/// How near a soldier on foot must walk to take it, metres across the ground and up or down.
pub const BACKPACK_REACH: f32 = 1.5;
pub const BACKPACK_REACH_Y: f32 = 2.5;
/// Seconds it may lie untouched before it goes back where the scenario put it.
pub const BACKPACK_RETURN_SECONDS: f32 = 60.0;
/// The `carry` component's value for the backpack.
pub const CARRY_BACKPACK: u8 = 1;

/// Combat. Every player starts a life with `PLAYER_MAX_HP` and dies at zero.
pub const PLAYER_MAX_HP: u8 = 150;
/// Ticks a dead player waits before respawning.
pub const RESPAWN_TICKS: u32 = 3 * TICK_RATE;
/// Guns are hitscans fired from the eye along the look direction; their stats are in `items`.
/// Height of a soldier's eye above its capsule centre, where a rifle shot leaves.
pub const EYE_HEIGHT: f32 = 0.6;
/// How far a crouch lowers the eye. The capsule keeps its size, so collision is unchanged.
pub const CROUCH_EYE_DROP: f32 = 0.3;
/// A prone soldier's eye is 0.2 m above its feet, so this is relative to the capsule centre.
pub const PRONE_EYE_HEIGHT: f32 = 0.2 - PRONE_CENTRE;

/// The on-foot camera sits behind a pivot this far to the right of the capsule centre and this far
/// above it. The server rebuilds the camera ray from it, so a shot lands where the crosshair is.
/// Mirrors `ON_FOOT_SHOULDER` and `ON_FOOT_HEIGHT` in the client.
pub const ON_FOOT_SHOULDER: f32 = 0.6;
pub const ON_FOOT_HEIGHT: f32 = 0.85;

/// Building with bricks (`ITEM_BRICKS`, `crate::construct`). Fire puts a cube where the crosshair ray
/// from the on-foot camera pivot meets the world, on the near side of the face it meets; aim (`BUTTON_ALT`)
/// with fire digs the placed cube there out instead. Neither reaches further than `BUILD_REACH` metres
/// from the eye to the cube's middle.
pub const BUILD_REACH: f32 = 6.0;
/// Ticks between two cubes placed or dug by one player while fire is held.
pub const BUILD_COOLDOWN_TICKS: u32 = TICK_RATE / 6;
/// Most cubes placed in the whole world at once: past it bricks place nothing until some are dug or
/// blasted. It bounds the memory, the catch-up a joiner gets and what is drawn.
pub const MAX_BUILT: usize = 200_000;
/// A piece of cubes cut off from what held it is looked through no further than this many cubes; a
/// bigger one is taken as held, so one blast never costs a long tick.
pub const BUILD_SUPPORT_SEARCH: usize = 4096;

/// The seated capsule's centre is this far below the head point, and it spans `SEATED_HALF_HEIGHT`
/// above and below that, plus `SEATED_RADIUS`.
pub const SEATED_CENTRE_DROP: f32 = 0.3;
pub const SEATED_HALF_HEIGHT: f32 = 0.25;
pub const SEATED_RADIUS: f32 = 0.25;
/// A round that hits a seated body above this far below its head point hit the head.
pub const SEATED_HEAD_ZONE: f32 = 0.12;
/// A passenger who can shoot looks over their own shoulder: the camera's pivot is this far to the
/// right of their head point and this far above it, and the camera sits `PASSENGER_DISTANCE` behind.
/// The server rebuilds the crosshair ray from it, as on foot.
pub const PASSENGER_SHOULDER: f32 = 0.6;
pub const PASSENGER_HEIGHT: f32 = 0.5;
pub const PASSENGER_DISTANCE: f32 = 3.0;

/// Eye height above the capsule centre for a player holding `buttons`, lying down or not.
pub fn eye_height(buttons: u16, prone: bool) -> f32 {
    if prone {
        PRONE_EYE_HEIGHT
    } else if buttons & BUTTON_CROUCH != 0 {
        EYE_HEIGHT - CROUCH_EYE_DROP
    } else {
        EYE_HEIGHT
    }
}
/// A hurt building is drawn in this many worse states. Stage `n` begins when its health falls to
/// `DAMAGE_STAGE_HEALTH[n - 1]` of full (percent); the last is about to collapse.
pub const DAMAGE_STAGES: u8 = 3;
pub const DAMAGE_STAGE_HEALTH: [u32; DAMAGE_STAGES as usize] = [66, 33, 15];
/// A blast hurting a building by at least this much leaves a scorch mark, and is announced.
pub const DAMAGE_MARK_MIN: u16 = 5;

/// Damage at the centre of an explosion; it falls off linearly to zero at `EXPLOSION_RADIUS`.
/// More than a full life, so a shell that lands within about 1.4 m kills outright.
pub const EXPLOSION_DAMAGE: f32 = 125.0;
/// An armour-piercing shell's small bursting charge, which goes off behind the plate it gets through.
pub const AP_BLAST_RADIUS: f32 = 3.0;
pub const AP_BLAST_DAMAGE: f32 = 70.0;
pub const AP_BLAST_KNOCK: f32 = 8.0;
pub const CRATER_AP_RADIUS: f32 = 1.6;
pub const CRATER_AP_DEPTH: f32 = 0.3;
/// Most shot events one snapshot carries; the rest are cosmetic and dropped. A shotgun pull is ten.
pub const MAX_SHOT_EVENTS: usize = 24;
/// Most hull marks (`S_MARKS`) a player is sent with one snapshot.
pub const MAX_HULL_MARKS: usize = 32;
/// Most bullet spawns one snapshot carries (nearest first); they are cheap, and are sent twice.
pub const MAX_BULLET_SPAWNS: usize = 32;

/// The rocket launcher: a projectile that flies straight and bursts on the first thing it meets.
pub const ROCKET_SPEED: f32 = 45.0;
pub const ROCKET_TTL_TICKS: u32 = 4 * TICK_RATE;
/// How far in front of the eye a rocket starts.
pub const ROCKET_MUZZLE: f32 = 1.0;
pub const ROCKET_BLAST_RADIUS: f32 = 5.0;
pub const ROCKET_DAMAGE: f32 = 110.0;
/// Knockback speed at the centre of a rocket's blast, m/s.
pub const ROCKET_KNOCK: f32 = 16.0;
/// Rockets in flight at once, which bounds the projectile list in a snapshot.
pub const MAX_ROCKETS: usize = 8;

/// How long a destroyed vehicle burns before it is put back at its home, and the blast it goes up in.
pub const WRECK_TICKS: u32 = 10 * TICK_RATE;
/// Angular speed (rad/s) a wreck is thrown into, about a horizontal axis: about one turn in the air.
pub const WRECK_SPIN: f32 = 6.5;
pub const WRECK_BLAST_RADIUS: f32 = 8.0;
pub const WRECK_BLAST_DAMAGE: f32 = 100.0;
pub const WRECK_BLAST_KNOCK: f32 = 14.0;

/// How far back the server will rewind other players to meet a shooter's view, in ticks (250 ms).
/// A shot never reaches further into the past than this, so a laggy shooter has a cap on how far
/// ahead of themselves they can lead their targets.
pub const MAX_REWIND_TICKS: u32 = 15;

/// The Hedemora rooster NPC. It meanders around `ROOSTER_HOME` (latitude, longitude), stops to crow
/// now and then, and drops a golden egg when killed. If the home is outside the world it lives at the first spawn.
pub const ROOSTER_HOME: (f64, f64) = (63.840784, 20.326410);
/// How far from home it wanders, metres.
pub const ROOSTER_WANDER_RADIUS: f32 = 8.0;
/// Walking speed, m/s, and how fast it turns, rad/s.
pub const ROOSTER_SPEED: f32 = 1.2;
pub const ROOSTER_TURN_RATE: f32 = 4.0;
pub const ROOSTER_HP: u8 = 30;
/// A killed rooster is back at home after this long.
pub const ROOSTER_RESPAWN_TICKS: u32 = 60 * TICK_RATE;
/// Length of a crow; the client's `crow` clip is the same length.
pub const ROOSTER_CROW_TICKS: u32 = 100;
/// Hit capsule: centre this high above its feet, metres.
pub const ROOSTER_CAPSULE_CENTRE: f32 = 0.4;
pub const ROOSTER_CAPSULE_HALF_HEIGHT: f32 = 0.1;
pub const ROOSTER_CAPSULE_RADIUS: f32 = 0.3;
