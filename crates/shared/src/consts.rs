//! Simulation and network constants. `client/scripts/gen-consts.ts` generates the client's copy from this file: run `npm run gen:consts` in `client` after editing.

pub use crate::vehicles::drone::*;
pub use crate::vehicles::golf_cart::*;
pub use crate::vehicles::heli::*;
pub use crate::vehicles::hen::*;
pub use crate::vehicles::technical::*;
pub use crate::vehicles::tupp::*;
pub use crate::vehicles::jeep::*;
pub use crate::vehicles::loiter::*;
pub use crate::vehicles::missile::*;
pub use crate::vehicles::mortar::*;
pub use crate::vehicles::pak::*;
pub use crate::vehicles::plane::*;
pub use crate::vehicles::roadster::*;
pub use crate::vehicles::seaplane::*;
pub use crate::vehicles::ship::*;
pub use crate::vehicles::ship_gun::*;
pub use crate::vehicles::snowmobile::*;
pub use crate::vehicles::sports::*;
pub use crate::vehicles::strv::*;
pub use crate::vehicles::strf::*;
pub use crate::vehicles::submarine::*;
pub use crate::vehicles::tank::*;
pub use crate::vehicles::walker::*;

/// Wire schema version. Bumped on any incompatible protocol or world-format change.
pub const PROTOCOL_VERSION: u8 = 76;

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
/// The snowmobile: a mountain sled for two that tows a Pak or a mortar up slopes no car can climb.
pub const KIND_SNOWMOBILE: u8 = 17;
/// The pirate ship: sailed by the wind on open water from its helm, its deck walked, with six guns.
pub const KIND_SHIP: u8 = 18;
/// One of a pirate ship's guns, fixed to its deck and manned by one gunner.
pub const KIND_SHIP_GUN: u8 = 19;
/// Wildlife: a deer (a hind) and a stag, NPCs that graze in herds and bolt from people and gunfire.
/// Replicated like the rooster, with their own models.
pub const KIND_DEER: u8 = 20;
pub const KIND_STAG: u8 = 21;
/// The golf cart: two seats side by side under a roof, an electric motor, and a bag rack on the back.
pub const KIND_GOLF_CART: u8 = 22;
/// The technical: a pickup with a machine gun in its bed, a driver, a passenger and a gunner.
pub const KIND_TECHNICAL: u8 = 23;
/// A zombie: a soldier's body with no gun and no mind but hunger, that rises at night in a scenario's
/// zombie areas (ZOMBIES.md). Replicated like a soldier, drawn with the soldier's model in rags.
pub const KIND_ZOMBIE: u8 = 24;
/// The submarine: a captain who steers and dives it, and a weapons officer at the periscope who fires
/// guided missiles from its launch tubes.
pub const KIND_SUBMARINE: u8 = 25;
/// A submarine's guided missile, flown from its nose by the officer who fired it, as a drone is.
pub const KIND_MISSILE: u8 = 26;
/// The seaplane: a two-seat biplane on pontoons that carry small wheels, which flies from water or a road.
/// Armed as the triplane is.
pub const KIND_SEAPLANE: u8 = 27;
/// The Strf 90: a tracked infantry fighting vehicle with a 40 mm autocannon in its turret and two
/// troops in the back, who get out by the rear ramp.
pub const KIND_STRF: u8 = 28;

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
/// One press: the headlamp on or off (L). Like a jump it is not repeated.
pub const BUTTON_LAMP: u16 = 1 << 14;
/// Puts skis on or takes them off, in a winter world, nearly still (`SKI_CLIP_SPEED`). The last bit.
pub const BUTTON_SKIS: u16 = 1 << 15;

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

/// Swimming (SWIMMING.md). Depths, the float height and whether the eyes are under all follow the
/// waves on the water (`shared::waves`). Water this deep over the feet, metres, floats a soldier off them: wading turns
/// into swimming. Swimming stops once it is no deeper than `SWIM_LEAVE_DEPTH`, so the change does not
/// flicker on a bank; a swimmer floating at `SWIM_FLOAT` has exactly `SWIM_DEPTH` over the feet.
pub const SWIM_DEPTH: f32 = 1.3;
pub const SWIM_LEAVE_DEPTH: f32 = 1.1;
/// How far below the still surface a floating swimmer's capsule centre is, metres: the eye
/// (`EYE_HEIGHT` over the centre) is just out of the water.
pub const SWIM_FLOAT: f32 = 0.45;
/// Swimming speed, m/s, and with run held, which spends stamina as running does.
pub const SWIM_SPEED: f32 = 2.0;
pub const SWIM_FAST_SPEED: f32 = 3.2;
/// Space kicks a swimmer up at this speed, m/s, and crouch takes them down at `SWIM_SINK`.
pub const SWIM_RISE: f32 = 1.6;
pub const SWIM_SINK: f32 = 1.4;
/// A diver who is not swimming drifts up at this speed, m/s.
pub const SWIM_DRIFT: f32 = 0.5;
/// A swimmer this far below the float height, metres, is under: looking up or down steers where they
/// swim. Above it the water floats them back up unless they are diving, so nobody hangs just under the
/// surface with their eyes in and out of it.
pub const SWIM_UNDER: f32 = 0.9;
/// At the surface, swimming forward while looking down by more than this, radians (52°, near the feet),
/// dives: far past where the camera looks while swimming along, so only a swimmer who means it goes under.
pub const SWIM_DIVE_PITCH: f32 = 0.9;
/// A dive from the surface goes down at least this fast, m/s, until the swimmer is under (`SWIM_UNDER`).
pub const SWIM_DUCK: f32 = 1.5;
/// How hard the water pulls a swimmer back to the float height, m/s for each metre off it.
pub const SWIM_BUOYANCY: f32 = 4.0;
/// How fast a swimmer's vertical speed comes round to what they want, m/s², plus `SWIM_DRAG` m/s²
/// for every m/s it is off: a plunge from a height is braked hard, a stroke changes gently.
pub const SWIM_ACCEL: f32 = 6.0;
pub const SWIM_DRAG: f32 = 3.0;
/// Each tick in the water knockback keeps this fraction of itself.
pub const SWIM_KNOCK_KEEP: f32 = 0.9;
/// A swimmer climbs out over a bank or a jetty's edge this high, metres, and up slopes this steep, degrees.
pub const SWIM_CLIMB: f32 = 0.6;
pub const SWIM_CLIMB_DEG: f32 = 75.0;
/// Air held on one breath, ticks (30 s at `TICK_RATE`), and air regained every tick with the eyes out
/// of the water (empty to full in 5 s).
pub const BREATH_FULL: u16 = 1800;
pub const BREATH_REFILL: u16 = 6;
/// Out of air, a diver drowns: this much off the torso every `DROWN_EVERY` ticks (15 hp a second, so
/// full health lasts 10 s) until they come up for air.
pub const DROWN_DAMAGE: u8 = 15;
pub const DROWN_EVERY: u16 = 60;

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
/// Hands up: a bot that has given in.
pub const STANCE_SURRENDER: u8 = 4;
/// Swimming in deep water with the head out (`Character::swimming`). The gun is put away.
pub const STANCE_SWIM: u8 = 5;
/// Swimming under the water, the eyes below its surface: holding the breath (`BREATH_FULL`).
pub const STANCE_DIVE: u8 = 6;
/// A zombie climbing out of the ground (`ZOMBIE_RISE_TICKS`): it neither moves nor claws until it is up.
pub const STANCE_RISE: u8 = 7;
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
/// Share of walking speed lost per unit of grade (height over distance) going uphill, and the least
/// that can leave. A 10% climb costs 15%; the steepest is a slog. Going down costs nothing, and gives
/// nothing: any faster and a runner leaves the ground.
pub const SLOPE_CLIMB_DRAG: f32 = 1.5;
pub const SLOPE_SCALE_MIN: f32 = 0.4;
/// Share of walking speed stony ground takes at its stoniest (`Tile::stones`), for cobbles and for a
/// boulder field's blocks: ISOM's grades come out near slow running 0.85, walk 0.6 and fight 0.4.
pub const STONY_DRAG: f32 = 0.6;
pub const BOULDER_DRAG: f32 = 0.66;
/// Further than this above the terrain, metres, a soldier is on a roof or a bridge, and its slope is not theirs.
pub const SLOPE_MAX_CLEARANCE: f32 = 2.0;

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
/// Where the camera behind a mounted gun sits: this far back from the gun's pivot along the look, and
/// this far above that, looking along the look. The middle of the screen is that ray, and the server
/// points the gun at what the ray meets (`MountDef::eye`), so a round goes where the reticle is.
/// A gunner's: a tank's, a technical's, a helicopter's nose gun, a plane's rear gun, a Pak's, a ship's gun.
pub const GUNNER_BACK: f32 = 6.5;
pub const GUNNER_UP: f32 = 2.6;
/// Over the shoulder of the gunner on a tank's roof.
pub const ROOF_AIM_BACK: f32 = 3.5;
pub const ROOF_AIM_UP: f32 = 1.2;
/// Behind a walker's guns or a tupp's eyes, high enough to see over the cab.
pub const WALKER_AIM_BACK: f32 = 12.0;
pub const WALKER_AIM_UP: f32 = 5.0;
/// Behind a Strv S's gun trunnion.
pub const STRV_AIM_BACK: f32 = 9.0;
pub const STRV_AIM_UP: f32 = 3.2;
/// How far the camera's ray is followed to find what a mounted gun's reticle is on, metres, when the
/// gun's own reach does not say.
pub const MOUNT_AIM_RANGE: f32 = 1500.0;
/// A gun sight's eye sits this far above the gun's pivot, on the mantlet, and looks along the look.
pub const SIGHT_EYE_UP: f32 = 0.5;
/// How quickly, seconds, a mounted gun's lay eases to a new distance when the reticle moves onto
/// something nearer or further: long enough that the barrel glides, short enough to be laid before a
/// shot is lined up. Eased in proportion (a log scale), so 10 m to 300 m takes as long as 300 m to 10 m.
pub const MOUNT_AIM_EASE: f32 = 0.25;
/// Out to this far, metres, a mounted gun's barrel is raised for the shell's fall and the shell lands
/// where the reticle is; further off it falls short of it, and the gunner aims high to make up the rest.
pub const MOUNT_ZERO_RANGE: f32 = 100.0;
/// The reticle camera is lifted to stay this far above the ground, metres, still looking along the look.
pub const MOUNT_EYE_CLEARANCE: f32 = 0.3;
/// A gun fired with aim held scatters over this fraction of its hip-fire cone.
pub const AIM_SPREAD_SCALE: f32 = 0.25;
/// ...and over this fraction of the bloom held fire has added to it (`items::Bloom`).
pub const AIM_BLOOM_SCALE: f32 = 0.6;
/// How fast that bloom shrinks again once the trigger is let go, radians a second.
pub const BLOOM_RECOVER: f32 = 0.12;
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
/// Skis on: `carry` is a mask of what is worn.
pub const CARRY_SKIS: u8 = 2;
/// The headlamp is on.
pub const CARRY_LAMP: u8 = 4;
/// Poling hard: on skis on the ground, pushing with run held.
pub const CARRY_POLING: u8 = 8;

/// Pull of gravity on a skier's slope, m/s². Real, unlike the game's heavier `GRAVITY` for flight.
pub const SKI_GRAVITY: f32 = 9.81;
/// Friction on a groomed track and on loose snow.
pub const SKI_MU_TRACK: f32 = 0.03;
pub const SKI_MU_SNOW: f32 = 0.08;
/// Friction in a groove of a track: the best going there is.
pub const SKI_MU_GROOVE: f32 = 0.022;
/// How far each classic groove lies from a track's centre line, metres.
pub const SKI_GROOVE_OFFSET: f32 = 0.8;
/// Cosine of how far off a groove skis can point or go and stay in it, or drop into it (28°).
pub const SKI_GROOVE_HOLD: f32 = 0.883;
/// How hard a groove draws skis to its line: m/s sideways per metre off it, and the most.
pub const SKI_GROOVE_PULL: f32 = 7.0;
pub const SKI_GROOVE_PULL_MAX: f32 = 2.5;
/// Air drag per (m/s)², upright and in a tuck.
pub const SKI_DRAG: f32 = 0.005;
pub const SKI_DRAG_TUCK: f32 = 0.0025;
/// What the poles put out, W/kg: over speed, it is the push.
pub const SKI_POWER: f32 = 3.0;
/// What the legs alone put out, W/kg: a gun in hand leaves no hand for the poles.
pub const SKI_KICK_POWER: f32 = 1.0;
/// What run held adds to the push from a standstill, m/s². It fades away to nothing at `SKI_BOOST_TOP`.
pub const SKI_BOOST: f32 = 3.5;
/// The speed past which poling hard gives no more, m/s: 43 km/h.
pub const SKI_BOOST_TOP: f32 = 12.0;
/// The speed a push is worked out from below this: a standing start does not push forever harder.
pub const SKI_PUSH_MIN_SPEED: f32 = 0.6;
/// Climbing steeper than this along the skis is a herringbone: stepping up on the inner edges, radians (12°).
pub const SKI_HERRING_FROM: f32 = 0.21;
/// The steepest a herringbone gets up, radians: 30° at a walk, 42° with run held.
pub const SKI_HERRING_SLOPE: f32 = 0.524;
pub const SKI_HERRING_SLOPE_RUN: f32 = 0.733;
/// The pace it keeps on easy ground, m/s, walking and with run held. It falls by `SKI_HERRING_SLOWING` at the limit.
pub const SKI_HERRING_PACE: f32 = 1.5;
pub const SKI_HERRING_PACE_RUN: f32 = 2.4;
pub const SKI_HERRING_SLOWING: f32 = 0.6;
/// How fast a herringbone gets up to its pace, m/s².
pub const SKI_HERRING_ACCEL: f32 = 5.0;
/// Skiing hard spends stamina this many times slower than running.
pub const SKI_STAMINA_SHARE: u16 = 3;
pub const SKI_MAX_SPEED: f32 = 25.0;
/// Fastest a skier slides backwards.
pub const SKI_SLIP_SPEED: f32 = 4.0;
pub const SKI_JUMP_SPEED: f32 = 4.5;
/// Gravity on a skier in the air, m/s²: lighter than `GRAVITY`, the game's heavy one for runners.
pub const SKI_AIR_GRAVITY: f32 = 13.0;
/// Over a crest the ground ahead is felt this far off, seconds of travel, and from this speed, m/s.
pub const SKI_CREST_LOOK: f32 = 0.12;
pub const SKI_CREST_SPEED: f32 = 6.0;
/// How much the ground's climb rate can fall over that look before the legs cannot follow it, m/s.
pub const SKI_CREST_DROP: f32 = 1.7;
/// Slowing a full snowplow adds, m/s², and how much better it holds a turn.
pub const SKI_PLOW_BRAKE: f32 = 3.5;
pub const SKI_PLOW_GRIP: f32 = 1.6;
/// Most sideways acceleration the edges give, m/s²: how fast the velocity follows the skis.
pub const SKI_GRIP: f32 = 7.0;
/// Below this the skis are stepped round and the velocity goes where they point, m/s.
pub const SKI_SHUFFLE_SPEED: f32 = 1.5;
/// How far strafing turns the skis from the look (the tangent of 45°).
pub const SKI_STRAFE_TURN: f32 = 1.0;
/// Too tight: speed × the angle between skis and travel past this (m/s × radians) strains the edges.
pub const SKI_EDGE: f32 = 11.0;
/// No speed under this catches an edge, m/s.
pub const SKI_FALL_MIN_SPEED: f32 = 5.0;
/// Ticks of strain that are a fall, and how fast it eases off.
pub const SKI_STRAIN_FALL: u8 = 12;
pub const SKI_STRAIN_EASE: u8 = 2;
/// Hardest landing the legs take, m/s across the ground, and how much more in a tuck.
pub const SKI_LANDING: f32 = 8.0;
pub const SKI_LANDING_TUCK: f32 = 1.3;
/// Landing above this speed with the skis more than this off the way they are going is a fall.
pub const SKI_LANDING_SKEW_SPEED: f32 = 6.0;
pub const SKI_LANDING_SKEW: f32 = 0.524;
/// Above this speed, being stopped short by something is a crash, m/s.
pub const SKI_CRASH_SPEED: f32 = 8.0;
/// Share of the wanted move that counts as stopped short.
pub const SKI_CRASH_MOVED: f32 = 0.4;
/// How far a fall bounces the skier up, m/s.
pub const SKI_FALL_HOP: f32 = 2.5;
/// Share of the speed a fall keeps each tick on the ground: a tumble, not a stop.
pub const SKI_TUMBLE_KEEP: f32 = 0.975;
/// Snap to the ground on skis, metres. Under the walkers': at speed a crest falls away faster than
/// this a tick, and the skier flies.
pub const SKI_SNAP_DISTANCE: f32 = 0.12;
/// Fastest skis go on or off, m/s.
pub const SKI_CLIP_SPEED: f32 = 2.0;

/// Combat. Every player starts a life with `PLAYER_MAX_HP` and dies at zero.
pub const PLAYER_MAX_HP: u8 = 150;
/// Ticks a dead player waits before respawning.
pub const RESPAWN_TICKS: u32 = 3 * TICK_RATE;
/// Guns are hitscans fired from the eye along the look direction; their stats are in `items`.
/// Height of a soldier's eye above its capsule centre, where a rifle shot leaves.
pub const EYE_HEIGHT: f32 = 0.6;
/// How far a crouch lowers the eye. The capsule keeps its size, so collision is unchanged.
pub const CROUCH_EYE_DROP: f32 = 0.3;
/// How far a crouch lowers the top of the hurtbox, with the head zone and the hips: as far as the eye.
pub const CROUCH_DROP: f32 = CROUCH_EYE_DROP;
/// Lying down, the hurtbox is a capsule on its side along the heading, the head at the front: this far
/// from the centre to the centre of either end, and this thick. 1.7 m long and under 0.8 m high.
pub const PRONE_HURT_HALF_LENGTH: f32 = 0.47;
pub const PRONE_HURT_RADIUS: f32 = 0.38;
/// A whole day and night, seconds, and where in it (0 midnight, 0.5 noon) tick 0 falls.
pub const DAY_SECONDS: f32 = 1800.0;
pub const DAY_START_PHASE: f32 = 0.36;
/// A prone soldier's eye is 0.2 m above its feet, so this is relative to the capsule centre.
pub const PRONE_EYE_HEIGHT: f32 = 0.2 - PRONE_CENTRE;

/// The on-foot camera sits behind a pivot this far to the right of the capsule centre and this far
/// above it. The server rebuilds the camera ray from it, so a shot lands where the crosshair is.
/// Mirrors `ON_FOOT_SHOULDER` and `ON_FOOT_HEIGHT` in the client.
pub const ON_FOOT_SHOULDER: f32 = 0.6;
pub const ON_FOOT_HEIGHT: f32 = 0.6;

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

/// Deer, placed in herds by the map editor (`animals` in area.json). They graze about their herd's
/// home, turn to look at someone who comes near and bolt from one who comes nearer, from gunfire and
/// blasts, and from being hurt; a herd bolts together. The stag lets people come closer.
/// How far from home a grazing herd drifts, metres.
pub const DEER_HERD_RADIUS: f32 = 14.0;
/// Walking speed and galloping (fleeing) speed, m/s, and how fast it turns walking and galloping, rad/s.
pub const DEER_WALK_SPEED: f32 = 1.3;
pub const DEER_GALLOP_SPEED: f32 = 9.0;
pub const DEER_TURN_RATE: f32 = 2.5;
pub const DEER_FLEE_TURN_RATE: f32 = 5.0;
/// How far it runs from what scared it, metres.
pub const DEER_FLEE_DISTANCE: f32 = 70.0;
/// Someone this close looks up from grazing, metres: walking, running, and crouched or prone and
/// still. Someone in a vehicle is noticed at the running distance plus a half.
pub const DEER_NOTICE_WALK: f32 = 24.0;
pub const DEER_NOTICE_RUN: f32 = 36.0;
pub const DEER_NOTICE_SNEAK: f32 = 8.0;
/// Of the noticing distance: someone nearer than this share of it makes it bolt.
pub const DEER_BOLT_SHARE: f32 = 0.6;
/// The stag's noticing distances are the hind's times this.
pub const STAG_NERVE: f32 = 0.7;
pub const DEER_HP: u8 = 60;
pub const STAG_HP: u8 = 90;
/// A killed deer is back at its herd's home after this long.
pub const DEER_RESPAWN_TICKS: u32 = 90 * TICK_RATE;
/// Hit capsule: centre this high above its hooves, metres. Hurtboxes stand upright, so it is a fat one
/// that takes in the body from chest to rump, and the head held up.
pub const DEER_CAPSULE_CENTRE: f32 = 0.9;
pub const DEER_CAPSULE_HALF_HEIGHT: f32 = 0.25;
pub const DEER_CAPSULE_RADIUS: f32 = 0.55;
pub const STAG_CAPSULE_CENTRE: f32 = 1.05;
pub const STAG_CAPSULE_HALF_HEIGHT: f32 = 0.3;
pub const STAG_CAPSULE_RADIUS: f32 = 0.6;

/// Zombies (ZOMBIES.md): soldiers' bodies that rise in hordes at night and hunt the living by smell.
/// They walk as soldiers do, so they lose arms and legs as soldiers do, and crawl on with no legs;
/// only the head or the torso kills one.
/// How far a zombie smells the living, metres: it needs no line of sight.
pub const ZOMBIE_SMELL_RANGE: f32 = 80.0;
/// Its shambling pace, slowest and fastest, as a forward axis value of 127: each zombie has its own.
pub const ZOMBIE_PACE: [i8; 2] = [55, 95];
/// Within this of its prey, metres, it lunges at full walking pace.
pub const ZOMBIE_LUNGE: f32 = 5.0;
/// One zombie in this many runs instead of shambling.
pub const ZOMBIE_RUNNER_ONE_IN: u32 = 8;
/// It claws at someone within this of its body's centre, metres.
pub const ZOMBIE_CLAW_REACH: f32 = 1.4;
/// A claw: ticks of reaching back before it lands, and of recovering after.
pub const ZOMBIE_CLAW_WINDUP: u32 = 24;
pub const ZOMBIE_CLAW_RECOVER: u32 = 30;
/// The harm a claw does, and a bite (a zombie with no arms left bites instead).
pub const ZOMBIE_CLAW_DAMAGE: u8 = 22;
pub const ZOMBIE_BITE_DAMAGE: u8 = 14;
/// Ticks a zombie takes to claw its way out of the ground (6 s at `TICK_RATE`): a hand first, then the
/// head, then it heaves itself out and stands. It does nothing else meanwhile, but can be shot.
pub const ZOMBIE_RISE_TICKS: u32 = 360;
/// The zombies of a horde come up one by one over this many ticks (10 s), each at a moment of its own.
pub const ZOMBIE_RISE_STAGGER: u32 = 600;
/// The most zombies up at once in a room, whatever a scenario asks for.
pub const MAX_ZOMBIES: usize = 96;
/// The zombies' side: they never hurt each other, and everyone else is their prey. Their profiles say
/// so, for the client to keep them off the scoreboard.
pub const TEAM_ZOMBIES: u8 = 255;

/// Golf (`ITEM_DRIVER`, `ITEM_PUTTER`, `PROJECTILE_GOLF_BALL`). A ball falls at this share of
/// `GRAVITY`, which is twice the earth's: a ball wants the real thing to carry like one.
pub const GOLF_GRAVITY_SCALE: f32 = 0.5;
pub const GOLF_BALL_RADIUS: f32 = 0.05;
/// A full swing's speed off the club, m/s, and how far above level it leaves, radians: the driver's
/// own, and with aim held, a high pitch that stops quickly.
pub const GOLF_DRIVER_SPEED: f32 = 70.0;
pub const GOLF_DRIVER_LOFT: f32 = 0.21;
pub const GOLF_DRIVER_HIGH_LOFT: f32 = 0.75;
pub const GOLF_PUTTER_SPEED: f32 = 6.0;
/// The swing meter (`golf::Meter`), counted in input frames, one a tick. Held fire fills it to a full
/// swing in `GOLF_CHARGE_TICKS`, and on into an overswing to `GOLF_OVERSWING`, where it lets go by
/// itself. Let go, a marker runs back across it, a full bar in `GOLF_RETURN_FRAMES`, to the line.
pub const GOLF_CHARGE_TICKS: u32 = TICK_RATE * 6 / 5;
pub const GOLF_OVERSWING: f32 = 1.1;
pub const GOLF_RETURN_FRAMES: u32 = TICK_RATE * 9 / 10;
/// Fire pressed again this many frames either side of the line strikes the ball dead straight; further
/// off it curves, early to the left and late to the right, fully `GOLF_CURVE_FRAMES` beyond that; more
/// than `GOLF_MISS_FRAMES` off (or never) is a mishit, topped, with `GOLF_MISHIT_POWER` of the power.
pub const GOLF_PURE_FRAMES: u32 = 1;
pub const GOLF_CURVE_FRAMES: u32 = 6;
pub const GOLF_MISS_FRAMES: u32 = 9;
pub const GOLF_MISHIT_POWER: f32 = 0.4;
/// Frames after a strike before the meter can be started again.
pub const GOLF_REST_FRAMES: u32 = TICK_RATE;
/// A fully curved ball's sideways pull in the air, m/s² for each m/s it flies at; a putt has no air to
/// curve in, so it is pulled or pushed off line by this much, radians.
pub const GOLF_SPIN: f32 = 0.12;
pub const GOLF_PUTT_PULL: f32 = 0.1;
/// A ball this far (metres, along the ground) from the golfer's feet, ahead of them, is in reach.
pub const GOLF_REACH: f32 = 1.6;
/// How much of its speed into a surface a ball keeps bouncing off it, and of its speed along it.
pub const GOLF_BOUNCE: f32 = 0.35;
pub const GOLF_SKID: f32 = 0.75;
/// Rolling friction, of the ball's weight: on a green, a fairway or tee, a bunker, and anywhere else.
pub const GOLF_ROLL_GREEN: f32 = 0.065;
pub const GOLF_ROLL_FAIRWAY: f32 = 0.16;
pub const GOLF_ROLL_BUNKER: f32 = 2.0;
pub const GOLF_ROLL_ROUGH: f32 = 0.4;
/// Slower than this (m/s) on a slope gentler than its friction holds, a rolling ball stops.
pub const GOLF_REST_SPEED: f32 = 0.08;
/// A ball within this of the cup's middle (metres) going slower than `GOLF_CUP_SPEED` (m/s) drops in.
pub const GOLF_CUP_RADIUS: f32 = 0.16;
pub const GOLF_CUP_SPEED: f32 = 1.8;
/// A ball nobody has struck for this long is gone; another waits on its tee.
pub const GOLF_BALL_TTL_TICKS: u32 = 300 * TICK_RATE;
/// Golf balls in the world at once.
pub const MAX_GOLF_BALLS: usize = 32;

/// Speed of a ski lift's cable and its chairs, m/s: quicker than a real chair lift, so a ride up a
/// hill takes minutes, not a quarter of an hour (LIFTS.md).
pub const LIFT_SPEED: f32 = 4.0;
/// Distance between chairs along the cable, metres, about: a lift's loop holds a whole number of them.
pub const LIFT_CHAIR_SPACING: f32 = 40.0;
/// Distance between the cable going up and the one coming down, metres: the bull wheel's diameter.
pub const LIFT_GAUGE: f32 = 4.4;
/// How far a chair's seat hangs under the cable, metres.
pub const LIFT_CHAIR_DROP: f32 = 2.6;
/// Height of the cable over the ground at a station, metres: the seat is at a rider's knees there.
pub const LIFT_STATION_CABLE: f32 = 3.1;
/// Height of the cable over the ground at a pylon, metres, at the least, and at the most.
pub const LIFT_PYLON_CABLE: f32 = 9.0;
pub const LIFT_PYLON_MAX: f32 = 30.0;
/// Height a chair's seat keeps over the ground between the pylons, metres, at the least, once it is
/// clear of the stations.
pub const LIFT_CLEARANCE: f32 = 2.5;
/// Longest span between two pylons, metres: a longer one gets pylons put in.
pub const LIFT_SPAN: f32 = 220.0;
/// Width of one rider's place on a chair, metres.
pub const LIFT_SEAT_PITCH: f32 = 0.6;
/// Riders a chair carries when OpenStreetMap does not say, and the most it carries.
pub const LIFT_SEATS: u8 = 2;
pub const LIFT_MAX_SEATS: u8 = 4;
/// A soldier this near a chair's seat (metres, across the ground), where the seat is no higher
/// than `LIFT_BOARD_HEIGHT` (metres) over the ground, can sit down on it: only at the stations.
pub const LIFT_BOARD_RADIUS: f32 = 2.2;
pub const LIFT_BOARD_HEIGHT: f32 = 1.3;
/// Segments of the half circle a chair goes round a station's bull wheel on.
pub const LIFT_WHEEL_SEGMENTS: usize = 8;
/// Height of a riding soldier's capsule centre over the chair's seat, metres: the legs hang below it,
/// and the eye (crouched) is a sitting one's.
pub const LIFT_RIDER_CENTRE: f32 = 0.45;
