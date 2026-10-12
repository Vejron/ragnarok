//! Simulation and network constants. Mirrored in `client/src/sim/consts.ts`.

/// Wire schema version. Bumped on any incompatible protocol or world-format change.
pub const PROTOCOL_VERSION: u8 = 20;

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
/// Aim held (right mouse): looks down the scope of a gun that has one, and tightens any gun's spread.
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
/// A round that hits above this height over the capsule centre hit the head.
pub const HEAD_ZONE_Y: f32 = 0.45;
/// What a headshot does to the damage.
pub const HEADSHOT_MULT: f32 = 2.0;

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
/// Seats in the roomiest car (the jeep); bounds the seat list of any vehicle.
pub const CAR_SEATS: usize = 4;
/// Vehicles in the world; bounds the vehicle list in a snapshot.
pub const MAX_VEHICLES: usize = 11;
/// A player can enter a vehicle whose centre is within this distance of their capsule centre.
pub const ENTER_RADIUS: f32 = 4.0;

/// Cars come in two kinds, which share the ray-cast wheel physics and differ in these numbers.
/// The jeep (`KIND_JEEP`) is tall on big wheels with plenty of ground clearance: slower, but it
/// rides over kerbs and rough ground. Half extents of the chassis box, metres.
pub const JEEP_HALF_EXTENTS: [f32; 3] = [1.0, 0.42, 1.95];
pub const JEEP_WHEEL_RADIUS: f32 = 0.55;
/// Wheel hard points relative to the chassis centre: half the track, and the distance of the
/// axles from the centre along Z. Front axle is at -Z.
pub const JEEP_TRACK_HALF: f32 = 0.9;
pub const JEEP_AXLE_Z: f32 = 1.25;
pub const JEEP_HARD_POINT_Y: f32 = 0.0;
pub const JEEP_SUSPENSION_REST: f32 = 0.4;
/// Height of the chassis centre above flat ground once the suspension has settled (measured by
/// `ride_heights_differ_between_cars` in vehicle.rs). The clearance under the box is this minus the half height.
pub const JEEP_RIDE_HEIGHT: f32 = 0.857;
/// Largest steering angle of the front wheels, radians. The wire carries it as -127..=127.
pub const JEEP_MAX_STEER: f32 = 0.5;
/// Forward speed above which engine force fades to zero, m/s.
pub const JEEP_MAX_SPEED: f32 = 32.0;

/// The sports car (`KIND_SPORTS`): a low two-seater with small wheels and a big engine. Quick in a
/// straight line, with the chassis only about 12 cm off the ground.
pub const SPORTS_SEATS: usize = 2;
pub const SPORTS_HALF_EXTENTS: [f32; 3] = [0.95, 0.3, 2.2];
pub const SPORTS_WHEEL_RADIUS: f32 = 0.32;
pub const SPORTS_TRACK_HALF: f32 = 0.82;
pub const SPORTS_AXLE_Z: f32 = 1.45;
pub const SPORTS_HARD_POINT_Y: f32 = 0.05;
pub const SPORTS_SUSPENSION_REST: f32 = 0.22;
pub const SPORTS_RIDE_HEIGHT: f32 = 0.397;
pub const SPORTS_MAX_STEER: f32 = 0.4;
pub const SPORTS_MAX_SPEED: f32 = 50.0;

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
/// The nose gun: a four-barrel autocannon on a chin mount. `HELI_GUN_PIVOT` is the mount in the hull
/// frame and `HELI_GUN_LENGTH` how far the muzzle is from it. The gunner's look direction aims it, within
/// `HELI_GUN_YAW_LIMIT` either side of the nose and `HELI_GUN_PITCH_MIN..=HELI_GUN_PITCH_MAX` (radians, the
/// hull's frame), at `HELI_GUN_SLEW` rad/s. It fires a shell (`SHELL_SPEED`, same fall and lifetime) every
/// `HELI_GUN_COOLDOWN_TICKS`, three a second, scattered by up to `HELI_GUN_SPREAD` radians. It bursts
/// like a tank shell on a smaller scale: a third of the radius and a third of the damage.
pub const HELI_GUN_PIVOT: [f32; 3] = [0.0, -0.6, -2.0];
pub const HELI_GUN_LENGTH: f32 = 1.3;
pub const HELI_GUN_YAW_LIMIT: f32 = 1.0;
pub const HELI_GUN_PITCH_MIN: f32 = -0.75;
pub const HELI_GUN_PITCH_MAX: f32 = 0.2;
pub const HELI_GUN_SLEW: f32 = 3.0;
pub const HELI_GUN_COOLDOWN_TICKS: u32 = TICK_RATE / 3;
pub const HELI_GUN_SPREAD: f32 = 0.01;
/// The blast of an autocannon round: a third of a tank shell's radius, a third of its damage.
pub const CANNON_RADIUS: f32 = EXPLOSION_RADIUS / 3.0;
pub const CANNON_DAMAGE: f32 = EXPLOSION_DAMAGE / 3.0;
pub const CANNON_KNOCK: f32 = EXPLOSION_KNOCK / 2.0;

/// The triplane: a Fokker Dr.I stretched into a two-seater, flown by a pilot (seat 0) who also fires the
/// two forward guns and drops the bombs, with a gunner (seat 1) on a ring-mounted gun behind. It rolls on
/// ray-cast wheels like a car, and in the air its wings hold it up once it is fast enough (see `wing` in
/// `physics/vehicle.rs`). Same body axes as the car.
pub const PLANE_SEATS: usize = 2;
/// The fuselage box. The wings are a second, weightless box (`PLANE_WING_*`), centred in the hull frame.
pub const PLANE_HALF_EXTENTS: [f32; 3] = [0.5, 0.6, 2.8];
pub const PLANE_WING_CENTRE: [f32; 3] = [0.0, 0.45, -1.1];
pub const PLANE_WING_HALF_EXTENTS: [f32; 3] = [3.6, 0.85, 0.55];
pub const PLANE_WHEEL_RADIUS: f32 = 0.35;
pub const PLANE_TRACK_HALF: f32 = 0.8;
/// The main gear ahead of the centre, the tail skid behind it. The front pair steers on the ground.
pub const PLANE_AXLES_Z: [f32; 2] = [-1.5, 2.4];
pub const PLANE_HARD_POINT_Y: f32 = -0.5;
pub const PLANE_SUSPENSION_REST: f32 = 0.3;
pub const PLANE_MAX_STEER: f32 = 0.5;
/// Acceleration the engine gives at full throttle, m/s². With the stick released it runs at
/// `PLANE_CRUISE_THROTTLE` of that, and with the stick pulled back the blip switch cuts it to `PLANE_IDLE_THROTTLE`.
pub const PLANE_THRUST: f32 = 6.5;
pub const PLANE_CRUISE_THROTTLE: f32 = 0.6;
pub const PLANE_IDLE_THROTTLE: f32 = 0.03;
/// Air drag per (m/s)², which tops a level flight out at about 45 m/s on full throttle and 35 at cruise.
pub const PLANE_DRAG: f32 = PLANE_THRUST / (45.0 * 45.0);
/// Below this airspeed (m/s) the wings stall: their lift fades out over the last 30% of it and the nose drops.
pub const PLANE_STALL_SPEED: f32 = 17.0;
/// Above this airspeed the wings carry the plane level by themselves; below it they carry less, by the square.
pub const PLANE_LEVEL_SPEED: f32 = 28.0;
/// The speed the controls are tuned for: they lose authority below it, by its square.
pub const PLANE_CRUISE_SPEED: f32 = 35.0;
/// How fast the wings turn the flight path towards where the nose points, per second, and how
/// fast the fin kills a sideslip.
pub const PLANE_AOA_GAIN: f32 = 4.0;
pub const PLANE_SIDE_GAIN: f32 = 3.0;
/// The most the wings can pull, m/s² (about 4 g).
pub const PLANE_MAX_LIFT: f32 = 40.0;
/// Turn rates at full deflection, rad/s, and how fast the body's spin follows them, per second.
pub const PLANE_PITCH_RATE: f32 = 1.4;
pub const PLANE_ROLL_RATE: f32 = 3.0;
pub const PLANE_YAW_RATE: f32 = 0.5;
pub const PLANE_RESPONSE: f32 = 6.0;
/// How hard a stalled plane drops its nose towards where it is going, rad/s.
pub const PLANE_STALL_DROP: f32 = 1.5;
/// Where a plane with nobody at the stick points its nose, radians below the horizon: it glides down.
pub const PLANE_GLIDE_PITCH: f32 = -0.2;
/// Off-target angle (radians) beyond which the plane banks into the turn instead of levelling its wings.
pub const PLANE_BANK_ANGLE: f32 = 0.2;
/// The steepest bank the plane rolls into by itself, radians (about 75°): past it the pull does the turning.
pub const PLANE_MAX_BANK: f32 = 1.3;
/// Rate asked for per radian the target is off the nose, as a share of the full rate.
pub const PLANE_AIM_GAIN: f32 = 2.0;
/// How steeply a pilot's mouse can ask the plane to climb or dive, radians (75 degrees).
pub const PLANE_AIM_MAX_PITCH: f32 = 1.3;
/// With the stick centred the nose settles back to the horizon at this share of full pitch per radian off it.
pub const PLANE_LEVEL_GAIN: f32 = 1.5;
/// The engine fades out over the last 20 m below this height; the boundary walls stop at 200 m.
pub const PLANE_CEILING: f32 = 160.0;
/// Within this many metres of the edge of the world the plane is steered back towards the middle.
pub const PLANE_EDGE_MARGIN: f32 = 120.0;
/// A plane is parked where a straight, flat, clear run of at least this many metres lies ahead.
pub const PLANE_RUNWAY: f32 = 80.0;
/// The two forward guns on the cowling, which fire in turn along the nose. `PLANE_GUN_COOLDOWN_TICKS`
/// between rounds makes twelve a second between them. A round is a hitscan out to `PLANE_GUN_RANGE`.
pub const PLANE_GUN_PIVOTS: [[f32; 3]; 2] = [[-0.16, 0.72, -2.0], [0.16, 0.72, -2.0]];
pub const PLANE_GUN_COOLDOWN_TICKS: u32 = 5;
pub const PLANE_GUN_RANGE: f32 = 300.0;
pub const PLANE_GUN_SPREAD: f32 = 0.012;
/// What a round does to a stickman, and to a vehicle: a plane is all canvas, anything else shrugs it off.
pub const PLANE_GUN_DAMAGE: u8 = 7;
pub const PLANE_GUN_PLANE_DAMAGE: u8 = 4;
pub const PLANE_GUN_VEHICLE_DAMAGE: u8 = 1;
/// Guns heat by this much a round and cool by `GUN_COOL` a tick, out of 255. At 255 they jam until
/// they have cooled to `GUN_UNJAM`: about four seconds of fire, then three of waiting.
pub const PLANE_GUN_HEAT: f32 = 10.5;
pub const GUN_COOL: f32 = 1.0;
pub const GUN_UNJAM: f32 = 77.0;
/// The gunner's ring mount behind the pilot: a pivot in the hull frame, the barrel's length, how far
/// it lifts and dips, and its rate. It turns all the way round, but holds fire within
/// `PLANE_REAR_DEAD_ZONE` of the nose, where the pilot sits.
pub const PLANE_REAR_PIVOT: [f32; 3] = [0.0, 0.85, 0.7];
pub const PLANE_REAR_LENGTH: f32 = 0.9;
pub const PLANE_REAR_PITCH_MIN: f32 = -0.35;
pub const PLANE_REAR_PITCH_MAX: f32 = 1.0;
pub const PLANE_REAR_SLEW: f32 = 4.0;
pub const PLANE_REAR_DEAD_ZONE: f32 = 0.6;
pub const PLANE_REAR_COOLDOWN_TICKS: u32 = 8;
pub const PLANE_REAR_HEAT: f32 = 16.5;
/// Bombs on the rack under the fuselage, one dropped at most every `PLANE_BOMB_COOLDOWN_TICKS`. A
/// bomb leaves with the plane's velocity and falls; it bursts on the first thing it meets. The
/// rack fills again on the ground once the plane has stood still for `PLANE_REARM_TICKS`.
pub const PLANE_BOMBS: u8 = 4;
pub const PLANE_BOMB_PIVOT: [f32; 3] = [0.0, -0.85, -0.3];
pub const PLANE_BOMB_COOLDOWN_TICKS: u32 = 30;
/// The rack holds on to its bombs below this height above the ground, metres.
pub const PLANE_BOMB_MIN_HEIGHT: f32 = 4.0;
pub const PLANE_REARM_TICKS: u32 = 3 * TICK_RATE;
pub const BOMB_RADIUS: f32 = 6.0;
pub const BOMB_DAMAGE: f32 = 110.0;
pub const BOMB_KNOCK: f32 = 16.0;

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
/// A hurt building is drawn in this many worse states. Stage `n` begins when its health falls to
/// `DAMAGE_STAGE_HEALTH[n - 1]` of full (percent); the last is about to collapse.
pub const DAMAGE_STAGES: u8 = 3;
pub const DAMAGE_STAGE_HEALTH: [u32; DAMAGE_STAGES as usize] = [66, 33, 15];
/// A blast hurting a building by at least this much leaves a scorch mark, and is announced.
pub const DAMAGE_MARK_MIN: u16 = 5;

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
