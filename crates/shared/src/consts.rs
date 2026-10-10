//! Simulation and network constants. Mirrored in `client/src/sim/consts.ts`.

/// Wire schema version. Bumped on any incompatible protocol or world-format change.
pub const PROTOCOL_VERSION: u8 = 12;

/// Fixed simulation rate, in ticks per second.
pub const TICK_RATE: u32 = 60;
/// Seconds per simulation tick.
pub const TICK_DT: f32 = 1.0 / TICK_RATE as f32;
/// A snapshot is sent every this many ticks (60 / 2 = 30 Hz).
pub const SNAPSHOT_EVERY: u32 = 2;

/// Players admitted to the single room. Interest management and the packet budget (`MAX_PACKET`)
/// keep a snapshot within one datagram however many of them are in view.
pub const MAX_PLAYERS: usize = 64;
/// Datagrams above this size are dropped unread. Stays under a typical QUIC datagram MTU.
pub const MAX_PACKET: usize = 1200;
/// A player is sent the entities, vehicles and effects within this many metres of their stickman
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
pub const BUTTON_JUMP: u8 = 1 << 0;
pub const BUTTON_RUN: u8 = 1 << 1;
pub const BUTTON_USE: u8 = 1 << 2;
/// Gunner's trigger. Held to keep firing; the gun's reload paces it.
pub const BUTTON_FIRE: u8 = 1 << 3;
/// Secondary fire. Nothing uses it on foot now that weapons live in the hotbar.
pub const BUTTON_ALT: u8 = 1 << 4;
/// Held to crouch: slower, and a lower eye.
pub const BUTTON_CROUCH: u8 = 1 << 5;
/// One press, sent with run and crouch held: starts a slide. Like a jump it is not repeated.
pub const BUTTON_SLIDE: u8 = 1 << 6;
/// One press: reload the selected gun. Like a jump it is not repeated.
pub const BUTTON_RELOAD: u8 = 1 << 7;

/// Stickman movement, in metres and seconds.
pub const WALK_SPEED: f32 = 4.0;
pub const RUN_SPEED: f32 = 7.5;
/// Crouching overrides running.
pub const CROUCH_SPEED: f32 = 2.0;
/// A slide is a burst of knockback along the run direction that bleeds off slowly while crouch is held.
pub const SLIDE_SPEED: f32 = 12.0;
pub const SLIDE_KEEP: f32 = 0.96;
/// A new slide can start once the last has slowed below this, in m/s.
pub const SLIDE_RETRIGGER: f32 = 1.0;
/// Above this much leftover velocity, a grounded crouching player is sliding.
pub const SLIDE_MIN_SPEED: f32 = 2.0;
pub const JUMP_SPEED: f32 = 6.5;
pub const GRAVITY: f32 = 20.0;
/// Knockback (a car hitting a stickman) is a horizontal velocity added to the walking velocity.
/// Each tick it keeps this fraction of itself: quickly on the ground, barely in the air.
pub const KNOCK_KEEP_GROUND: f32 = 0.92;
pub const KNOCK_KEEP_AIR: f32 = 0.99;
/// Below this speed (m/s) knockback is dropped, so it always dies out exactly.
pub const KNOCK_MIN_SPEED: f32 = 0.05;
/// Fastest knockback the wire can carry and the server will give, m/s.
pub const KNOCK_MAX_SPEED: f32 = 20.0;
/// Terminal fall speed, so `vy` always fits its i16 cm/s wire field.
pub const MAX_FALL_SPEED: f32 = 50.0;

/// Stickman collision capsule. Total height is `2 * (half_height + radius)` = 1.7 m, with the
/// body origin at the capsule centre.
pub const CAPSULE_HALF_HEIGHT: f32 = 0.5;
pub const CAPSULE_RADIUS: f32 = 0.35;

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
/// Body-local axes: +X right, +Y up, -Z forward (yaw 0 faces -Z, like a stickman).
pub const CAR_SEATS: usize = 4;
/// Vehicles in the world; bounds the vehicle list in a snapshot.
pub const MAX_VEHICLES: usize = 9;
/// A player can enter a vehicle whose centre is within this distance of their capsule centre.
pub const ENTER_RADIUS: f32 = 4.0;

/// Half extents of the chassis box, metres.
pub const CAR_HALF_EXTENTS: [f32; 3] = [0.9, 0.4, 2.1];
pub const CAR_WHEEL_RADIUS: f32 = 0.35;
/// Wheel hard points relative to the chassis centre: half the track, and the distance of the
/// axles from the centre along Z. Front axle is at -Z.
pub const CAR_TRACK_HALF: f32 = 0.85;
pub const CAR_AXLE_Z: f32 = 1.35;
pub const CAR_HARD_POINT_Y: f32 = -0.1;
pub const CAR_SUSPENSION_REST: f32 = 0.3;
/// Largest steering angle of the front wheels, radians. The wire carries it as -127..=127.
pub const CAR_MAX_STEER: f32 = 0.5;
/// Forward speed above which engine force fades to zero, m/s.
pub const CAR_MAX_SPEED: f32 = 30.0;

/// The tank: a heavy six-wheel hull that turns on the spot, with a driver (seat 0) and a gunner
/// (seat 1) who aims the turret and fires. Same body axes as the car.
pub const TANK_SEATS: usize = 2;
pub const TANK_HALF_EXTENTS: [f32; 3] = [1.3, 0.5, 2.6];
pub const TANK_WHEEL_RADIUS: f32 = 0.45;
pub const TANK_TRACK_HALF: f32 = 1.2;
/// Z of the three axles, front first.
pub const TANK_AXLES_Z: [f32; 3] = [-1.9, 0.0, 1.9];
pub const TANK_HARD_POINT_Y: f32 = -0.2;
pub const TANK_SUSPENSION_REST: f32 = 0.35;
pub const TANK_MAX_SPEED: f32 = 12.0;
/// Yaw rate at full stick, rad/s.
pub const TANK_TURN_RATE: f32 = 1.1;
/// Where the turret ring sits in the hull frame, and how far the muzzle is from the ring.
pub const TANK_TURRET_PIVOT: [f32; 3] = [0.0, 0.85, 0.3];
pub const TANK_BARREL_LENGTH: f32 = 3.2;
/// The turret is solid: a cylinder on the ring, so it is the same shape whichever way it points.
/// Its base is the hull's top.
pub const TANK_TURRET_RADIUS: f32 = 1.0;
pub const TANK_TURRET_HALF_HEIGHT: f32 = 0.35;
/// How fast the turret and barrel follow the gunner's aim, rad/s.
pub const TANK_TURRET_SLEW: f32 = 1.4;
pub const TANK_BARREL_SLEW: f32 = 0.9;
/// Barrel elevation limits, radians (+ is up).
pub const TANK_BARREL_MIN: f32 = -0.12;
pub const TANK_BARREL_MAX: f32 = 0.5;

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
/// Climbing fades out over the last 20 m below this height, and above it the rotor pushes down.
pub const HELI_CEILING: f32 = 120.0;
/// How firmly the hull levels out (rad/s of correction per unit of lean error), and how fast the
/// angular velocity follows it, per second.
pub const HELI_LEVEL_GAIN: f32 = 4.0;
pub const HELI_RESPONSE: f32 = 8.0;
/// The nose gun: a four-barrel machine gun on a chin mount. `HELI_GUN_PIVOT` is the mount in the hull
/// frame and `HELI_GUN_LENGTH` how far the muzzle is from it. The gunner's look direction aims it, within
/// `HELI_GUN_YAW_LIMIT` either side of the nose and `HELI_GUN_PITCH_MIN..=HELI_GUN_PITCH_MAX` (radians, the
/// hull's frame), at `HELI_GUN_SLEW` rad/s. It is hitscan: a shot is one ray of `HELI_GUN_RANGE` metres,
/// scattered by up to `HELI_GUN_SPREAD` radians, one every `HELI_GUN_COOLDOWN_TICKS`.
pub const HELI_GUN_PIVOT: [f32; 3] = [0.0, -0.6, -2.0];
pub const HELI_GUN_LENGTH: f32 = 1.3;
pub const HELI_GUN_YAW_LIMIT: f32 = 1.0;
pub const HELI_GUN_PITCH_MIN: f32 = -0.75;
pub const HELI_GUN_PITCH_MAX: f32 = 0.2;
pub const HELI_GUN_SLEW: f32 = 3.0;
pub const HELI_GUN_COOLDOWN_TICKS: u32 = 5;
pub const HELI_GUN_RANGE: f32 = 150.0;
pub const HELI_GUN_DAMAGE: u8 = 8;
pub const HELI_GUN_SPREAD: f32 = 0.01;

/// The tank gun.
pub const SHELL_COOLDOWN_TICKS: u32 = 90;
/// Speed a shell leaves the muzzle at, m/s. It falls under `GRAVITY` from there, so at 100 m it has
/// dropped about 5 m and at 200 m about 20.
pub const SHELL_SPEED: f32 = 140.0;
/// How long a shell flies before it bursts where it is.
pub const SHELL_TTL_TICKS: u32 = 6 * TICK_RATE;
/// Speed a fired shell takes from the tank, m/s, backwards along the barrel.
pub const SHELL_RECOIL: f32 = 1.6;
pub const EXPLOSION_RADIUS: f32 = 7.0;
/// Knockback speed at the centre of a blast, m/s; it fades to zero at the edge.
pub const EXPLOSION_KNOCK: f32 = 18.0;

/// Combat. Every player starts a life with `PLAYER_MAX_HP` and dies at zero.
pub const PLAYER_MAX_HP: u8 = 100;
/// Ticks a dead player waits before respawning.
pub const RESPAWN_TICKS: u32 = 3 * TICK_RATE;
/// Guns are hitscans fired from the eye along the look direction; their stats are in `items`.
/// Height of a stickman's eye above its capsule centre, where a rifle shot leaves.
pub const EYE_HEIGHT: f32 = 0.6;
/// How far a crouch lowers the eye. The capsule keeps its size, so collision is unchanged.
pub const CROUCH_EYE_DROP: f32 = 0.3;

/// The on-foot camera sits behind a pivot this far to the right of the capsule centre and this far
/// above it. The server rebuilds the camera ray from it, so a shot lands where the crosshair is.
/// Mirrors `ON_FOOT_SHOULDER` and `ON_FOOT_HEIGHT` in the client.
pub const ON_FOOT_SHOULDER: f32 = 0.9;
pub const ON_FOOT_HEIGHT: f32 = 1.5;

/// Eye height above the capsule centre for a player holding `buttons`.
pub fn eye_height(buttons: u8) -> f32 {
    if buttons & BUTTON_CROUCH != 0 { EYE_HEIGHT - CROUCH_EYE_DROP } else { EYE_HEIGHT }
}
/// Damage at the centre of an explosion; it falls off linearly to zero at `EXPLOSION_RADIUS`.
/// More than a full life, so a shell that lands within about 1.4 m kills outright.
pub const EXPLOSION_DAMAGE: f32 = 125.0;
/// Most shot events one snapshot carries; the rest are cosmetic and dropped.
pub const MAX_SHOT_EVENTS: usize = 8;

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
