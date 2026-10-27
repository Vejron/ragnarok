//! Simulation and network constants. `client/scripts/gen-consts.ts` generates the client's copy from this file: run `npm run gen:consts` in `client` after editing.

/// Wire schema version. Bumped on any incompatible protocol or world-format change.
pub const PROTOCOL_VERSION: u8 = 47;

/// Entity kinds, the `kind` component: which model a client draws. A vehicle kind's definition is
/// `crate::vehicles`.
pub const KIND_STICKMAN: u8 = 0;
/// A Hedemora rooster, an NPC. Replicated like a stickman, with its own model.
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

/// Stickman movement, in metres and seconds.
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

/// Stickman collision capsule. Total height is `2 * (half_height + radius)` = 1.7 m, with the
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
pub const MAX_VEHICLES: usize = 32;
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

/// The Roadster (`KIND_ROADSTER`): Tesla's 2026 two-seater with SpaceX cold-gas thrusters fed from a
/// tank of compressed air. Same chassis and wheels as the sports car, a stronger motor, and three
/// tricks: rear nozzles that push it far past what the tyres could (Shift), a hop straight up off the
/// ground (Space), and lift nozzles that hold it in the air while Space stays held. Thrust drains
/// the tank, which refills while it sits on its wheels.
pub const ROADSTER_SEATS: usize = 2;
pub const ROADSTER_HALF_EXTENTS: [f32; 3] = SPORTS_HALF_EXTENTS;
pub const ROADSTER_WHEEL_RADIUS: f32 = SPORTS_WHEEL_RADIUS;
pub const ROADSTER_TRACK_HALF: f32 = SPORTS_TRACK_HALF;
pub const ROADSTER_AXLE_Z: f32 = SPORTS_AXLE_Z;
pub const ROADSTER_HARD_POINT_Y: f32 = SPORTS_HARD_POINT_Y;
pub const ROADSTER_SUSPENSION_REST: f32 = SPORTS_SUSPENSION_REST;
pub const ROADSTER_RIDE_HEIGHT: f32 = SPORTS_RIDE_HEIGHT;
pub const ROADSTER_MAX_STEER: f32 = 0.4;
pub const ROADSTER_MAX_SPEED: f32 = 62.0;
/// The air tank, as a share of full.
pub const ROADSTER_AIR: f32 = 1.0;
/// The rear nozzles' push along the nose, m/s²: well past the tyres' grip.
pub const ROADSTER_BOOST_ACCEL: f32 = 24.0;
/// Air the rear nozzles use, share of the tank per second: about three seconds of boost.
pub const ROADSTER_BOOST_DRAIN: f32 = 0.33;
/// Upward speed a hop adds, m/s: enough to clear a row of cars.
pub const ROADSTER_HOP_SPEED: f32 = 9.0;
/// Air a hop uses, share of the tank.
pub const ROADSTER_HOP_COST: f32 = 0.15;
/// While hovering, the lift nozzles hold this upward speed, m/s, and use this share of the tank per second.
pub const ROADSTER_HOVER_CLIMB: f32 = 1.0;
pub const ROADSTER_HOVER_DRAIN: f32 = 0.25;
/// How hard the lift nozzles chase that speed, per second, and the most they add to gravity's worth, m/s².
pub const ROADSTER_HOVER_GAIN: f32 = 3.0;
pub const ROADSTER_HOVER_ACCEL: f32 = 6.0;
/// While hovering, the lift nozzles lean the car this far (radians, roughly) with the stick, level
/// it this hard and this fast (per second), and turn it at this yaw rate, rad/s.
pub const ROADSTER_HOVER_TILT: f32 = 0.2;
pub const ROADSTER_HOVER_LEVEL: f32 = 4.0;
pub const ROADSTER_HOVER_RESPONSE: f32 = 6.0;
pub const ROADSTER_HOVER_YAW: f32 = 1.2;
/// The lift nozzles fade out over 20 m below this height.
pub const ROADSTER_CEILING: f32 = 40.0;
/// Refill rate on the ground with no nozzle firing, share of the tank per second.
pub const ROADSTER_AIR_REFILL: f32 = 0.2;

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
/// What it takes to knock the turret off: damage from hits on the turret and the hull's roof, which a
/// hull hit never adds to. Past it `DAMAGE_TURRET` is set, the gunner dies and the gun is gone until the
/// tank respawns. Mirrors `TANK_TURRET_HP` in `sim/consts.ts`.
pub const TANK_TURRET_HP: u8 = 90;

/// The Strv S (`KIND_STRV`): a turretless tank with the gun fixed in the hull. One seat: the driver steers
/// by pivoting the hull and fires the main gun, which only the barrel's elevation can aim. Quicker and
/// lower than the tank, with a steeply sloped glacis. Same body axes as the tank; mirrors `STRV_*` in
/// `sim/consts.ts`.
pub const STRV_SEATS: usize = 1;
pub const STRV_HALF_EXTENTS: [f32; 3] = [1.3, 0.45, 2.7];
/// Bigger wheels and a longer suspension than the tank's, with the hull higher on them: it steps over
/// obstacles the tank has to go around.
pub const STRV_WHEEL_RADIUS: f32 = 0.52;
pub const STRV_TRACK_HALF: f32 = 1.2;
/// Z of the three axles, front first.
pub const STRV_AXLES_Z: [f32; 3] = [-1.9, 0.0, 1.9];
pub const STRV_HARD_POINT_Y: f32 = -0.2;
pub const STRV_SUSPENSION_REST: f32 = 0.42;
pub const STRV_MAX_SPEED: f32 = 16.0;
/// Yaw rate at full stick, rad/s.
pub const STRV_TURN_RATE: f32 = 1.5;
/// The casemate the gun sits in is a solid box on the hull's roof, weightless like the tank's turret:
/// its half extents and centre in the hull frame.
pub const STRV_CASEMATE_HALF: [f32; 3] = [0.9, 0.17, 1.25];
pub const STRV_CASEMATE_CENTRE: [f32; 3] = [0.0, 0.62, -0.25];
/// Where the gun's trunnion is in the hull frame, how far the muzzle is from it, how fast the barrel
/// elevates (rad/s) and its limits, radians (+ is up). The gun points where the hull does.
pub const STRV_GUN_PIVOT: [f32; 3] = [0.0, 0.72, -1.3];
pub const STRV_BARREL_LENGTH: f32 = 4.4;
pub const STRV_BARREL_SLEW: f32 = 1.2;
pub const STRV_BARREL_MIN: f32 = -0.15;
pub const STRV_BARREL_MAX: f32 = 0.3;

/// The walker (`KIND_WALKER`): a two-legged assault walker, one driver in the head. Its body is a single
/// box held up by two spring legs that are ray casts to the ground, and kept upright by a torque that a
/// big enough blow overpowers. Mirrors `WALKER_*` in `sim/consts.ts`.
pub const WALKER_SEATS: usize = 1;
pub const WALKER_HALF_EXTENTS: [f32; 3] = [1.0, 1.2, 1.8];
/// Distance of each foot from the middle of the hull, sideways, metres, and how long a leg is from the
/// bottom of the hull to the ground when it stands.
pub const WALKER_FOOT_HALF: f32 = 1.7;
pub const WALKER_LEG_LENGTH: f32 = 4.2;
/// The legs are solid, so a car or a shell can hit them: a box on each hip, this wide and deep (half
/// extents, metres), ending this far above the ground when it stands so the feet's own springs carry it.
pub const WALKER_LEG_HALF_WIDTH: f32 = 0.25;
pub const WALKER_LEG_HALF_DEPTH: f32 = 0.3;
pub const WALKER_LEG_CLEARANCE: f32 = 0.5;
/// Top speed forwards, backwards (a share of it), m/s, and the acceleration towards it, m/s².
pub const WALKER_MAX_SPEED: f32 = 9.0;
pub const WALKER_REVERSE_SHARE: f32 = 0.5;
pub const WALKER_ACCEL: f32 = 4.5;
/// Deceleration, m/s², when it slows or reverses: it plants its feet and leans back.
pub const WALKER_BRAKE: f32 = 6.0;
/// Planted feet do not slide sideways: the push against it, per m/s of sideways speed, and its cap, m/s².
pub const WALKER_GRIP_GAIN: f32 = 8.0;
pub const WALKER_GRIP: f32 = 8.0;
/// Yaw rate at full stick, rad/s, and the angular acceleration towards it, rad/s².
pub const WALKER_TURN_RATE: f32 = 1.7;
pub const WALKER_TURN_ACCEL: f32 = 8.0;
/// Gravity acting on vehicle bodies. Stickmen use `GRAVITY`; the ray-cast suspension is tuned
/// around the real-world value.
pub const CAR_GRAVITY: f32 = 9.81;
/// Leg spring and damper, per kilogram of the whole body for the pair: the spring settles at
/// `CAR_GRAVITY / WALKER_LEG_SPRING` of compression.
pub const WALKER_LEG_SPRING: f32 = 80.0;
pub const WALKER_LEG_DAMPING: f32 = 14.0;
/// How hard the hull is pulled upright, rad/s² per radian of tilt, and the damping on its tilt rate, per
/// second. The pull never exceeds `WALKER_BALANCE_MAX` rad/s²: a blow that tips it further than that
/// can undo goes over.
pub const WALKER_BALANCE_GAIN: f32 = 10.0;
pub const WALKER_BALANCE_DAMP: f32 = 4.0;
pub const WALKER_BALANCE_MAX: f32 = 3.0;
/// Pitching (forwards and backwards) is the axis a walker can fight best: its feet are in line with the
/// tilt, so the pull is up to this, and it also takes a catching step. The step is a push towards where
/// the top is falling, by `WALKER_CATCH_GAIN` m/s² per unit of lean from where it should be and
/// `WALKER_CATCH_DAMP` per rad/s of fall, at most `WALKER_CATCH_MAX`.
pub const WALKER_BALANCE_MAX_PITCH: f32 = 6.0;
pub const WALKER_CATCH_GAIN: f32 = 20.0;
pub const WALKER_CATCH_DAMP: f32 = 3.0;
pub const WALKER_CATCH_MAX: f32 = 8.0;
/// Lean (up's horizontal part along the heading beyond its walking lean) and fall rate (rad/s) it
/// tolerates before it takes a step, so a walk's own lean and its pushes are left alone.
pub const WALKER_CATCH_LEAN_MARGIN: f32 = 0.15;
pub const WALKER_CATCH_FALL_MARGIN: f32 = 0.5;
/// Extra lean into its heading at top speed, as a tilt: the walker walks a little forward of its feet.
pub const WALKER_CRUISE_LEAN: f32 = 0.08;
/// How much of the tilt that would match its push it leans by: the push goes through the middle of
/// the hull, so the lean is for show and for how it handles.
pub const WALKER_ACCEL_LEAN: f32 = 0.5;
/// A blast pushes a walker at its shoulders, this far above the middle of the hull, so it tips it.
pub const WALKER_BLAST_LEVER: f32 = 3.0;
/// Below this `up_y` it has fallen: the legs no longer push or drive, and only a weak pull
/// (`WALKER_GETUP_SHARE` of the full one) stands it up again.
pub const WALKER_FALLEN_UP: f32 = 0.6;
pub const WALKER_GETUP_SHARE: f32 = 0.25;
/// The chin guns: the driver aims them with the look, and the upper body turns to point them, up to
/// `WALKER_TORSO_LIMIT` either side of the way the legs face (and `WALKER_GUN_PITCH_MIN..=_MAX` up and
/// down, radians, hull frame), at `WALKER_GUN_SLEW` rad/s. `WALKER_GUN_PIVOT` is where the upper body turns
/// in the hull frame and `WALKER_GUN_LENGTH` how far the muzzle is from it. A round is a cannon shell, as
/// in a helicopter's nose gun, fired every `WALKER_GUN_COOLDOWN_TICKS` and scattered by up to `WALKER_GUN_SPREAD`.
pub const WALKER_TORSO_LIMIT: f32 = 0.52;
pub const WALKER_GUN_PITCH_MIN: f32 = -0.5;
pub const WALKER_GUN_PITCH_MAX: f32 = 0.4;
pub const WALKER_GUN_SLEW: f32 = 2.5;
pub const WALKER_GUN_PIVOT: [f32; 3] = [0.0, 0.1, 0.0];
pub const WALKER_GUN_LENGTH: f32 = 3.7;
/// The two barrels hang `WALKER_GUN_BARREL_DROP` below the pivot, `WALKER_GUN_BARREL_X` either side of
/// it, and fire in turn.
pub const WALKER_GUN_BARREL_X: f32 = 0.3;
pub const WALKER_GUN_BARREL_DROP: f32 = 1.0;
pub const WALKER_GUN_COOLDOWN_TICKS: u32 = TICK_RATE / 4;
pub const WALKER_GUN_SPREAD: f32 = 0.01;

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
/// Highest relief a world may have, metres from its lowest dry ground (y = 0) to its highest. The
/// importer refuses more; the boundary walls reach above it, and fliers find the terrain under them by
/// a ray from just over it.
pub const MAX_RELIEF: f32 = 1000.0;

/// Climbing fades out over the last 20 m below this height above the terrain, and above it the rotor
/// pushes down. Every flier's ceiling is measured from the terrain under it, not from y = 0.
pub const HELI_CEILING: f32 = 120.0;
/// How firmly the hull levels out (rad/s of correction per unit of lean error), and how fast the
/// angular velocity follows it, per second.
pub const HELI_LEVEL_GAIN: f32 = 4.0;
pub const HELI_RESPONSE: f32 = 8.0;
/// The FPV drone: a quadcopter the size of a dinner plate. It is flown by velocity: forward stick asks
/// for `DRONE_SPEED` along where its pilot looks (diving or climbing with the look's pitch), strafe for
/// `DRONE_STRAFE_SPEED` sideways, jump and run for `DRONE_CLIMB_SPEED` / `DRONE_SINK_SPEED` straight up
/// or down, and nothing held for a hover. The rotors close the gap at `DRONE_ACCEL_GAIN` per second, never
/// pushing harder than `DRONE_MAX_ACCEL` m/s² beyond holding it up, and the hull leans the way they push.
/// The look turns it: `DRONE_YAW_GAIN` rad/s per radian off, at most `DRONE_YAW_RATE`.
pub const DRONE_SEATS: usize = 1;
pub const DRONE_HALF_EXTENTS: [f32; 3] = [0.22, 0.06, 0.22];
pub const DRONE_SPEED: f32 = 30.0;
pub const DRONE_STRAFE_SPEED: f32 = 12.0;
pub const DRONE_CLIMB_SPEED: f32 = 10.0;
pub const DRONE_SINK_SPEED: f32 = 10.0;
pub const DRONE_ACCEL_GAIN: f32 = 2.0;
pub const DRONE_MAX_ACCEL: f32 = 18.0;
pub const DRONE_YAW_GAIN: f32 = 6.0;
pub const DRONE_YAW_RATE: f32 = 4.0;
/// Climbing fades out over the last 20 m below this height, and above it the rotors push down.
pub const DRONE_CEILING: f32 = 150.0;
/// How firmly the hull follows its lean (rad/s per unit of error), and how fast the angular velocity
/// follows that, per second.
pub const DRONE_LEVEL_GAIN: f32 = 8.0;
pub const DRONE_RESPONSE: f32 = 14.0;
/// Air drag on the body, which is all that slows a drone whose motors are off.
pub const DRONE_DAMPING: f32 = 0.1;
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
/// The rocket pods under the stub wings, fired by the pilot (seat 0) on the fire button. `HELI_POD_PIVOTS`
/// are the four tube mouths in the hull frame, used in turn, one rocket every `HELI_ROCKET_COOLDOWN_TICKS`.
/// A rocket leaves at `HELI_ROCKET_LAUNCH_SPEED` over the helicopter's own velocity, along the nose, and
/// is driven at `ROCKET_ACCEL` m/s² up to `ROCKET_MAX_SPEED`: it flies straight, no gravity.
pub const HELI_POD_PIVOTS: [[f32; 3]; 4] = [[-1.0, -0.39, -0.3], [1.0, -0.39, -0.3], [-1.75, -0.525, -0.3], [1.75, -0.525, -0.3]];
pub const HELI_ROCKET_COOLDOWN_TICKS: u32 = TICK_RATE / 5;
pub const HELI_ROCKET_LAUNCH_SPEED: f32 = 20.0;
pub const ROCKET_ACCEL: f32 = 110.0;
pub const ROCKET_MAX_SPEED: f32 = 170.0;
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
pub const PLANE_GUN_DAMAGE: u8 = 14;
pub const PLANE_GUN_PLANE_DAMAGE: u8 = 7;
pub const PLANE_GUN_VEHICLE_DAMAGE: u8 = 3;
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

/// The field mortar (`KIND_MORTAR`): a towed, unpowered two-wheeler with one gunner, who aims its bearing
/// with the look and its range with the dial. It is towed by a car's rear tow point through a spherical joint.
pub const MORTAR_SEATS: usize = 1;
pub const MORTAR_HALF_EXTENTS: [f32; 3] = [1.0, 0.5, 1.5];
pub const MORTAR_WHEEL_RADIUS: f32 = 0.45;
pub const MORTAR_TRACK_HALF: f32 = 0.85;
/// Where the mount sits in the hull frame, and the barrel's length from it.
pub const MORTAR_GUN_PIVOT: [f32; 3] = [0.0, 0.7, -0.2];
pub const MORTAR_BARREL_LENGTH: f32 = 3.2;
/// Elevation travel, radians: a high lob at the bottom, nearly vertical at the top. The gunner never sets
/// it by hand: the range dial does (`lob_pitch` in gunnery).
pub const MORTAR_PITCH_MIN: f32 = 0.785;
pub const MORTAR_PITCH_MAX: f32 = 1.484;
/// The range dial, metres: W and S walk it at `MORTAR_RANGE_RATE` while `BUTTON_RANGE_UP` /
/// `BUTTON_RANGE_DOWN` are held. The longest lob is 245 m (`GRAVITY` and the muzzle speed at 45 degrees),
/// the shortest the high lob reaches is about 42 m, so the dial stops at 50.
pub const MORTAR_RANGE_START: f32 = 120.0;
pub const MORTAR_RANGE_MIN: f32 = 50.0;
pub const MORTAR_RANGE_MAX: f32 = 245.0;
pub const MORTAR_RANGE_RATE: f32 = 60.0;
/// How fast the mount follows the gunner's look, rad/s. Quick enough that the barrel stays on the crosshair.
pub const MORTAR_TRAVERSE_SLEW: f32 = 3.0;
/// Muzzle speed of a mortar bomb, m/s. Under `GRAVITY` it reaches about 245 m at 45 degrees and 40 m at 85.
pub const MORTAR_SHELL_SPEED: f32 = 70.0;
/// A reload is long: the gunner waits between bombs.
pub const MORTAR_COOLDOWN_TICKS: u32 = 7 * TICK_RATE;
pub const MORTAR_SHELL_TTL_TICKS: u32 = 10 * TICK_RATE;
/// Speed the mortar is shoved back at when it fires, m/s, along the barrel's horizontal heading.
pub const MORTAR_RECOIL: f32 = 3.0;
/// Blast of a mortar bomb: wider and harder than a tank shell.
pub const MORTAR_RADIUS: f32 = 11.0;
pub const MORTAR_DAMAGE: f32 = 260.0;
pub const MORTAR_KNOCK: f32 = 30.0;
/// The Pak 38 (`KIND_PAK`): a towed 5 cm anti-tank gun on two wheels with split trails and a shield,
/// one gunner. Its muzzle points down the hull's -Z, the trails run back to +Z, and it is towed by the
/// trails' eye (`PAK_TONGUE_Z`), so it travels muzzle-last. It fires the tank's shell, laid by the look
/// within its traverse and elevation, as a hull-mounted gun (`Aim::Mount`).
pub const PAK_SEATS: usize = 1;
pub const PAK_HALF_EXTENTS: [f32; 3] = [0.8, 0.35, 1.4];
pub const PAK_WHEEL_RADIUS: f32 = 0.4;
pub const PAK_TRACK_HALF: f32 = 0.8;
/// The road wheels' axle, and the trails' spades it rests on at the back (the physics' second axle).
pub const PAK_AXLES_Z: [f32; 2] = [-0.7, 1.25];
/// The towing eye where the closed trails meet, behind the hull box.
pub const PAK_TONGUE_Z: f32 = PAK_HALF_EXTENTS[2] + TOW_BACK;
/// The trunnions, over the axle, and the barrel's length from them.
pub const PAK_GUN_PIVOT: [f32; 3] = [0.0, 0.25, -0.7];
pub const PAK_BARREL_LENGTH: f32 = 3.0;
/// Traverse either side of the trails' centre line, and elevation, radians: about 65 degrees in all,
/// and -8 to +27 degrees.
pub const PAK_TRAVERSE: f32 = 0.57;
pub const PAK_PITCH_MIN: f32 = -0.14;
pub const PAK_PITCH_MAX: f32 = 0.47;
/// How fast the gun is laid onto the look, rad/s: hand wheels, a little quicker than a tank's turret.
pub const PAK_SLEW: f32 = 1.2;
/// Speed it is shoved back at when it fires, m/s: its spades are dug in.
pub const PAK_RECOIL: f32 = 1.0;
/// The shield across the front, in the hull frame: it stops what would hit the gunner from ahead.
pub const PAK_SHIELD_HALF: [f32; 3] = [0.85, 0.4, 0.03];
pub const PAK_SHIELD_CENTRE: [f32; 3] = [0.0, 0.15, -0.95];
/// The tow point sits this far behind a car's rear face, metres.
pub const TOW_BACK: f32 = 0.25;
/// How close a mortar's tongue must be to a car's tow point to hitch, metres.
pub const HITCH_REACH: f32 = 2.5;
/// A hitch or an unhitch needs the car at a near standstill, m/s.
pub const HITCH_MAX_SPEED: f32 = 1.5;
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
/// A prone stickman's eye is 0.2 m above its feet, so this is relative to the capsule centre.
pub const PRONE_EYE_HEIGHT: f32 = 0.2 - PRONE_CENTRE;

/// The on-foot camera sits behind a pivot this far to the right of the capsule centre and this far
/// above it. The server rebuilds the camera ray from it, so a shot lands where the crosshair is.
/// Mirrors `ON_FOOT_SHOULDER` and `ON_FOOT_HEIGHT` in the client.
pub const ON_FOOT_SHOULDER: f32 = 0.9;
pub const ON_FOOT_HEIGHT: f32 = 1.5;

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

/// Seated players. Each seat of a vehicle with open seats has a head point in the hull frame, where
/// the client draws the sitting body's head (a car's `seat_N` markers, built by `build_car.py`). The
/// body that can be shot is a short upright capsule hanging below it: head and shoulders down to the
/// waist, so the hull box in front of the lower body takes the rounds aimed there.
pub const JEEP_SEAT_HEADS: [[f32; 3]; 4] = [[-0.4, 0.62, -0.3], [0.4, 0.62, -0.3], [-0.4, 0.62, 0.75], [0.4, 0.62, 0.75]];
pub const SPORTS_SEAT_HEADS: [[f32; 3]; 2] = [[-0.35, 0.4, 0.1], [0.35, 0.4, 0.1]];
pub const ROADSTER_SEAT_HEADS: [[f32; 3]; 2] = [[-0.33, 0.42, 0.12], [0.33, 0.42, 0.12]];
/// The pilot raised behind and above the gunner in the nose.
pub const HELI_SEAT_HEADS: [[f32; 3]; 2] = [[0.0, 0.45, -0.45], [0.0, 0.1, -1.55]];
/// The pilot behind the wings, the gunner standing in the ring.
pub const PLANE_SEAT_HEADS: [[f32; 3]; 2] = [[0.0, 0.98, -0.45], [0.0, 1.15, 0.7]];
/// The mortar's gunner rides its mount, so this is in the mount's frame: `MORTAR_GUN_PIVOT` turned by the gun's yaw.
pub const MORTAR_SEAT_HEAD: [f32; 3] = [0.6, 0.45, 0.6];
/// The Pak 38's gunner kneels behind the shield, left of the breech, at the sight: in the mount's frame.
pub const PAK_SEAT_HEAD: [f32; 3] = [-0.4, 0.2, 0.6];
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
/// Most shot events one snapshot carries; the rest are cosmetic and dropped. A shotgun pull is ten.
pub const MAX_SHOT_EVENTS: usize = 24;
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
