// GENERATED from crates/shared/src/consts.rs by scripts/gen-consts.ts. Do not edit:
// change the Rust and run `npm run gen:consts`. Client-only constants live in consts.ts.

/** Wire schema version. Bumped on any incompatible protocol or world-format change. */
export const PROTOCOL_VERSION = 42

/**
 * Entity kinds, the `kind` component: which model a client draws. A vehicle kind's definition is
 * `crate::vehicles`.
 */
export const KIND_STICKMAN = 0

/** A Hedemora rooster, an NPC. Replicated like a stickman, with its own model. */
export const KIND_ROOSTER = 5

/** Vehicle kinds: replicated with an `orientation`. */
export const KIND_JEEP = 1
export const KIND_TANK = 2
export const KIND_HELI = 3
export const KIND_SPORTS = 4

/** The triplane. 5 is the rooster's. */
export const KIND_PLANE = 6

/** The two-legged walker. 5 is the rooster's. */
export const KIND_WALKER = 7

/** The turretless Strv S tank, one seat. */
export const KIND_STRV = 8

/** The towed field mortar, one seat for its gunner. */
export const KIND_MORTAR = 9

/** The Tesla Roadster, a sports car with cold-gas thrusters: boost, hop and hover. Two seats. */
export const KIND_ROADSTER = 10

/** An FPV drone, flown from afar by the player who launched it. Gone once it blows up or its pilot lets go. */
export const KIND_DRONE = 11

/** The hen: a giant robot hen on the walker's legs that jumps, flaps and glides, and lays egg bombs. One seat. */
export const KIND_HEN = 12

/**
 * A loitering drone: a Shahed-sized delta wing with a big warhead, flown from afar like an FPV drone
 * but launched high above and behind its pilot, and always cruising forward.
 */
export const KIND_LOITER = 13

/** Fixed simulation rate, in ticks per second. */
export const TICK_RATE = 60

/** Seconds per simulation tick. */
export const TICK_DT = 1.0 / TICK_RATE

/** A snapshot is sent every this many ticks (60 / 2 = 30 Hz). */
export const SNAPSHOT_EVERY = 2

/**
 * Players admitted to the single room. Interest management and the packet budget (`MAX_PACKET`)
 * keep a snapshot within one datagram however many of them are in view.
 */
export const MAX_PLAYERS = 64

/** Outfit colours a player can pick from (`client/src/profile.ts` holds the colours themselves). */
export const PLAYER_COLOR_COUNT = 12

/** Spawn points a player may choose, used in turn each time they spawn. */
export const MAX_SPAWNS = 5

/** Longest player name, in characters. */
export const MAX_NAME_CHARS = 16

/** Datagrams above this size are dropped unread. Stays under a typical QUIC datagram MTU. */
export const MAX_PACKET = 1200

/**
 * A player is sent the entities, vehicles and effects within this many metres of their stickman
 * (measured on the ground plane), and keeps them until they are `INTEREST_HYSTERESIS` further out,
 * so something at the edge does not flicker in and out.
 */
export const INTEREST_RADIUS = 300.0
export const INTEREST_HYSTERESIS = 40.0

/**
 * Snapshots the server remembers per player to diff against, and the client remembers to
 * rebuild from. The client keeps at least as many as the server, so any baseline the server
 * still offers is one the client still has. At 30 Hz, 32 is about a second.
 */
export const BASELINE_WINDOW = 32
export const BASELINE_WINDOW_CLIENT = 64

/** Each input datagram repeats this many of the most recent frames, so a lost packet costs nothing. */
export const INPUT_REDUNDANCY = 4

/** Input buttons, as bits of `InputFrame::buttons`. */
export const BUTTON_JUMP = 1 << 0
export const BUTTON_RUN = 1 << 1
export const BUTTON_USE = 1 << 2

/** Gunner's trigger. Held to keep firing; the gun's reload paces it. */
export const BUTTON_FIRE = 1 << 3

/** Aim held (right mouse): looks down the scope of a gun that has one, and tightens any gun's spread. */
export const BUTTON_ALT = 1 << 4

/** Held to crouch: slower, and a lower eye. */
export const BUTTON_CROUCH = 1 << 5

/** One press, sent with run and crouch held: starts a slide. Like a jump it is not repeated. */
export const BUTTON_SLIDE = 1 << 6

/** One press: reload the selected gun. Like a jump it is not repeated. */
export const BUTTON_RELOAD = 1 << 7

/** Held to lie down: slower, and a low eye. Lets go to stand again, if there is room overhead. */
export const BUTTON_PRONE = 1 << 8

/** One press: hitch a free mortar to the car in front of you, or unhitch the one it tows. Like a jump it is not repeated. */
export const BUTTON_HITCH = 1 << 9

/** Held by a mortar's gunner to walk the range dial out and in (W and S). */
export const BUTTON_RANGE_UP = 1 << 10
export const BUTTON_RANGE_DOWN = 1 << 11

/** One press: open or cut the parachute while in the air. Like a jump it is not repeated. */
export const BUTTON_CHUTE = 1 << 12

/** Stickman movement, in metres and seconds. */
export const WALK_SPEED = 4.0
export const RUN_SPEED = 7.5

/** Crouching overrides running. */
export const CROUCH_SPEED = 2.0

/** Lying down overrides both: crawling. */
export const PRONE_SPEED = 1.0

/** A slide is a burst of knockback along the run direction that bleeds off slowly while crouch is held. */
export const SLIDE_SPEED = 12.0
export const SLIDE_KEEP = 0.96

/** A new slide can start once the last has slowed below this, in m/s. */
export const SLIDE_RETRIGGER = 1.0

/** Above this much leftover velocity, a grounded crouching player is sliding. */
export const SLIDE_MIN_SPEED = 2.0
export const JUMP_SPEED = 6.5
export const GRAVITY = 20.0

/**
 * Knockback (a car hitting a stickman) is a horizontal velocity added to the walking velocity.
 * Each tick it keeps this fraction of itself: quickly on the ground, barely in the air.
 */
export const KNOCK_KEEP_GROUND = 0.92
export const KNOCK_KEEP_AIR = 0.99

/** Below this speed (m/s) knockback is dropped, so it always dies out exactly. */
export const KNOCK_MIN_SPEED = 0.05

/** Fastest knockback the wire can carry and the server will give, m/s. */
export const KNOCK_MAX_SPEED = 20.0

/** Terminal fall speed, so `vy` always fits its i16 cm/s wire field. */
export const MAX_FALL_SPEED = 50.0

/** Parachute. An open canopy settles the fall at this speed, m/s; a faster fall is braked at `CHUTE_DRAG` m/s². */
export const CHUTE_DESCENT = 5.0
export const CHUTE_DRAG = 40.0

/** Glide speed under a canopy: full forward, backward (braking) and sideways, m/s. */
export const CHUTE_FORWARD = 15.0
export const CHUTE_BACK = 3.0
export const CHUTE_SIDE = 8.0

/** How much of the gap to the wanted glide velocity is closed each tick: a canopy turns lazily. */
export const CHUTE_STEER = 0.03

/** Respawns drop in from this far above the ground, metres, with the canopy open. */
export const SPAWN_ALTITUDE = 100.0

/**
 * Stickman collision capsule. Total height is `2 * (half_height + radius)` = 1.7 m, with the
 * body origin at the capsule centre.
 */
export const CAPSULE_HALF_HEIGHT = 0.5
export const CAPSULE_RADIUS = 0.35

/**
 * Lying down, the body is a ball of this radius (0.6 m across) resting on the ground. Its bottom is
 * where the standing capsule's is, so going prone drops the centre by `STAND_CENTRE - PRONE_CENTRE`.
 */
export const PRONE_RADIUS = 0.3

/** Capsule-centre height above the feet, standing and prone. */
export const STAND_CENTRE = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS
export const PRONE_CENTRE = PRONE_RADIUS

/** Stance, as `replicated::Flags::stance` on the wire. Crouching is a held button, not a stance. */
export const STANCE_STANDING = 0
export const STANCE_PRONE = 1

/** Hanging under an open parachute. */
export const STANCE_CHUTE = 2

/** A round that hits above this height over the capsule centre hit the head. */
export const HEAD_ZONE_Y = 0.45

/** What a headshot does to the damage. */
export const HEADSHOT_MULT = 2.0

/** Character controller tuning. The client builds its Rapier controller from the same values. */
export const CONTROLLER_OFFSET = 0.02
export const MAX_SLOPE_CLIMB_DEG = 50.0
export const AUTOSTEP_HEIGHT = 0.35
export const AUTOSTEP_MIN_WIDTH = 0.2

/**
 * How far below its feet a standing character is pulled back onto the ground when it walks off
 * a slope, metres. Without it, running downhill is a series of small falls.
 */
export const SNAP_DISTANCE = 0.3

/** Anything that falls below this is respawned (e.g. a player squeezed through a gap in the mesh). */
export const KILL_Y = -50.0

/**
 * Vehicles. Chassis and wheel geometry are shared with the client, which draws the car from them.
 * Body-local axes: +X right, +Y up, -Z forward (yaw 0 faces -Z, like a stickman).
 * Seats in the roomiest car (the jeep); bounds the seat list of any vehicle.
 */
export const CAR_SEATS = 4

/** Vehicles in the world; bounds the vehicle list in a snapshot. */
export const MAX_VEHICLES = 32

/** A player can enter a vehicle whose centre is within this distance of their capsule centre. */
export const ENTER_RADIUS = 4.0

/**
 * Cars come in two kinds, which share the ray-cast wheel physics and differ in these numbers.
 * The jeep (`KIND_JEEP`) is tall on big wheels with plenty of ground clearance: slower, but it
 * rides over kerbs and rough ground. Half extents of the chassis box, metres.
 */
export const JEEP_HALF_EXTENTS = [1.0, 0.42, 1.95] as const
export const JEEP_WHEEL_RADIUS = 0.55

/**
 * Wheel hard points relative to the chassis centre: half the track, and the distance of the
 * axles from the centre along Z. Front axle is at -Z.
 */
export const JEEP_TRACK_HALF = 0.9
export const JEEP_AXLE_Z = 1.25
export const JEEP_HARD_POINT_Y = 0.0
export const JEEP_SUSPENSION_REST = 0.4

/**
 * Height of the chassis centre above flat ground once the suspension has settled (measured by
 * `ride_heights_differ_between_cars` in vehicle.rs). The clearance under the box is this minus the half height.
 */
export const JEEP_RIDE_HEIGHT = 0.857

/** Largest steering angle of the front wheels, radians. The wire carries it as -127..=127. */
export const JEEP_MAX_STEER = 0.5

/** Forward speed above which engine force fades to zero, m/s. */
export const JEEP_MAX_SPEED = 32.0

/**
 * The sports car (`KIND_SPORTS`): a low two-seater with small wheels and a big engine. Quick in a
 * straight line, with the chassis only about 12 cm off the ground.
 */
export const SPORTS_SEATS = 2
export const SPORTS_HALF_EXTENTS = [0.95, 0.3, 2.2] as const
export const SPORTS_WHEEL_RADIUS = 0.32
export const SPORTS_TRACK_HALF = 0.82
export const SPORTS_AXLE_Z = 1.45
export const SPORTS_HARD_POINT_Y = 0.05
export const SPORTS_SUSPENSION_REST = 0.22
export const SPORTS_RIDE_HEIGHT = 0.397
export const SPORTS_MAX_STEER = 0.4
export const SPORTS_MAX_SPEED = 50.0

/**
 * The Roadster (`KIND_ROADSTER`): Tesla's 2026 two-seater with SpaceX cold-gas thrusters fed from a
 * tank of compressed air. Same chassis and wheels as the sports car, a stronger motor, and three
 * tricks: rear nozzles that push it far past what the tyres could (Shift), a hop straight up off the
 * ground (Space), and lift nozzles that hold it in the air while Space stays held. Thrust drains
 * the tank, which refills while it sits on its wheels.
 */
export const ROADSTER_SEATS = 2
export const ROADSTER_HALF_EXTENTS = SPORTS_HALF_EXTENTS
export const ROADSTER_WHEEL_RADIUS = SPORTS_WHEEL_RADIUS
export const ROADSTER_TRACK_HALF = SPORTS_TRACK_HALF
export const ROADSTER_AXLE_Z = SPORTS_AXLE_Z
export const ROADSTER_HARD_POINT_Y = SPORTS_HARD_POINT_Y
export const ROADSTER_SUSPENSION_REST = SPORTS_SUSPENSION_REST
export const ROADSTER_RIDE_HEIGHT = SPORTS_RIDE_HEIGHT
export const ROADSTER_MAX_STEER = 0.4
export const ROADSTER_MAX_SPEED = 62.0

/** The air tank, as a share of full. */
export const ROADSTER_AIR = 1.0

/** The rear nozzles' push along the nose, m/s²: well past the tyres' grip. */
export const ROADSTER_BOOST_ACCEL = 24.0

/** Air the rear nozzles use, share of the tank per second: about three seconds of boost. */
export const ROADSTER_BOOST_DRAIN = 0.33

/** Upward speed a hop adds, m/s: enough to clear a row of cars. */
export const ROADSTER_HOP_SPEED = 9.0

/** Air a hop uses, share of the tank. */
export const ROADSTER_HOP_COST = 0.15

/** While hovering, the lift nozzles hold this upward speed, m/s, and use this share of the tank per second. */
export const ROADSTER_HOVER_CLIMB = 1.0
export const ROADSTER_HOVER_DRAIN = 0.25

/** How hard the lift nozzles chase that speed, per second, and the most they add to gravity's worth, m/s². */
export const ROADSTER_HOVER_GAIN = 3.0
export const ROADSTER_HOVER_ACCEL = 6.0

/**
 * While hovering, the lift nozzles lean the car this far (radians, roughly) with the stick, level
 * it this hard and this fast (per second), and turn it at this yaw rate, rad/s.
 */
export const ROADSTER_HOVER_TILT = 0.2
export const ROADSTER_HOVER_LEVEL = 4.0
export const ROADSTER_HOVER_RESPONSE = 6.0
export const ROADSTER_HOVER_YAW = 1.2

/** The lift nozzles fade out over 20 m below this height. */
export const ROADSTER_CEILING = 40.0

/** Refill rate on the ground with no nozzle firing, share of the tank per second. */
export const ROADSTER_AIR_REFILL = 0.2

/**
 * The tank: a heavy six-wheel hull that turns on the spot, with a driver (seat 0) and a gunner
 * (seat 1) who aims the turret and fires. Same body axes as the car.
 */
export const TANK_SEATS = 2
export const TANK_HALF_EXTENTS = [1.3, 0.5, 2.6] as const
export const TANK_WHEEL_RADIUS = 0.45
export const TANK_TRACK_HALF = 1.2

/** Z of the three axles, front first. */
export const TANK_AXLES_Z = [-1.9, 0.0, 1.9] as const
export const TANK_HARD_POINT_Y = -0.2
export const TANK_SUSPENSION_REST = 0.35
export const TANK_MAX_SPEED = 12.0

/** Yaw rate at full stick, rad/s. */
export const TANK_TURN_RATE = 1.1

/** Where the turret ring sits in the hull frame, and how far the muzzle is from the ring. */
export const TANK_TURRET_PIVOT = [0.0, 0.85, 0.3] as const
export const TANK_BARREL_LENGTH = 3.2

/**
 * The turret is solid: a cylinder on the ring, so it is the same shape whichever way it points.
 * Its base is the hull's top.
 */
export const TANK_TURRET_RADIUS = 1.0
export const TANK_TURRET_HALF_HEIGHT = 0.35

/** How fast the turret and barrel follow the gunner's aim, rad/s. */
export const TANK_TURRET_SLEW = 1.4
export const TANK_BARREL_SLEW = 0.9

/** Barrel elevation limits, radians (+ is up). */
export const TANK_BARREL_MIN = -0.12
export const TANK_BARREL_MAX = 0.5

/**
 * What it takes to knock the turret off: damage from hits on the turret and the hull's roof, which a
 * hull hit never adds to. Past it `DAMAGE_TURRET` is set, the gunner dies and the gun is gone until the
 * tank respawns. Mirrors `TANK_TURRET_HP` in `sim/consts.ts`.
 */
export const TANK_TURRET_HP = 90

/**
 * The Strv S (`KIND_STRV`): a turretless tank with the gun fixed in the hull. One seat: the driver steers
 * by pivoting the hull and fires the main gun, which only the barrel's elevation can aim. Quicker and
 * lower than the tank, with a steeply sloped glacis. Same body axes as the tank; mirrors `STRV_*` in
 * `sim/consts.ts`.
 */
export const STRV_SEATS = 1
export const STRV_HALF_EXTENTS = [1.3, 0.45, 2.7] as const

/**
 * Bigger wheels and a longer suspension than the tank's, with the hull higher on them: it steps over
 * obstacles the tank has to go around.
 */
export const STRV_WHEEL_RADIUS = 0.52
export const STRV_TRACK_HALF = 1.2

/** Z of the three axles, front first. */
export const STRV_AXLES_Z = [-1.9, 0.0, 1.9] as const
export const STRV_HARD_POINT_Y = -0.2
export const STRV_SUSPENSION_REST = 0.42
export const STRV_MAX_SPEED = 16.0

/** Yaw rate at full stick, rad/s. */
export const STRV_TURN_RATE = 1.5

/**
 * The casemate the gun sits in is a solid box on the hull's roof, weightless like the tank's turret:
 * its half extents and centre in the hull frame.
 */
export const STRV_CASEMATE_HALF = [0.9, 0.17, 1.25] as const
export const STRV_CASEMATE_CENTRE = [0.0, 0.62, -0.25] as const

/**
 * Where the gun's trunnion is in the hull frame, how far the muzzle is from it, how fast the barrel
 * elevates (rad/s) and its limits, radians (+ is up). The gun points where the hull does.
 */
export const STRV_GUN_PIVOT = [0.0, 0.72, -1.3] as const
export const STRV_BARREL_LENGTH = 4.4
export const STRV_BARREL_SLEW = 1.2
export const STRV_BARREL_MIN = -0.15
export const STRV_BARREL_MAX = 0.3

/**
 * The walker (`KIND_WALKER`): a two-legged assault walker, one driver in the head. Its body is a single
 * box held up by two spring legs that are ray casts to the ground, and kept upright by a torque that a
 * big enough blow overpowers. Mirrors `WALKER_*` in `sim/consts.ts`.
 */
export const WALKER_SEATS = 1
export const WALKER_HALF_EXTENTS = [1.0, 1.2, 1.8] as const

/**
 * Distance of each foot from the middle of the hull, sideways, metres, and how long a leg is from the
 * bottom of the hull to the ground when it stands.
 */
export const WALKER_FOOT_HALF = 1.7
export const WALKER_LEG_LENGTH = 4.2

/**
 * The legs are solid, so a car or a shell can hit them: a box on each hip, this wide and deep (half
 * extents, metres), ending this far above the ground when it stands so the feet's own springs carry it.
 */
export const WALKER_LEG_HALF_WIDTH = 0.25
export const WALKER_LEG_HALF_DEPTH = 0.3
export const WALKER_LEG_CLEARANCE = 0.5

/** Top speed forwards, backwards (a share of it), m/s, and the acceleration towards it, m/s². */
export const WALKER_MAX_SPEED = 9.0
export const WALKER_REVERSE_SHARE = 0.5
export const WALKER_ACCEL = 4.5

/** Deceleration, m/s², when it slows or reverses: it plants its feet and leans back. */
export const WALKER_BRAKE = 6.0

/** Planted feet do not slide sideways: the push against it, per m/s of sideways speed, and its cap, m/s². */
export const WALKER_GRIP_GAIN = 8.0
export const WALKER_GRIP = 8.0

/** Yaw rate at full stick, rad/s, and the angular acceleration towards it, rad/s². */
export const WALKER_TURN_RATE = 1.7
export const WALKER_TURN_ACCEL = 8.0

/**
 * Gravity acting on vehicle bodies. Stickmen use `GRAVITY`; the ray-cast suspension is tuned
 * around the real-world value.
 */
export const CAR_GRAVITY = 9.81

/**
 * Leg spring and damper, per kilogram of the whole body for the pair: the spring settles at
 * `CAR_GRAVITY / WALKER_LEG_SPRING` of compression.
 */
export const WALKER_LEG_SPRING = 80.0
export const WALKER_LEG_DAMPING = 14.0

/**
 * How hard the hull is pulled upright, rad/s² per radian of tilt, and the damping on its tilt rate, per
 * second. The pull never exceeds `WALKER_BALANCE_MAX` rad/s²: a blow that tips it further than that
 * can undo goes over.
 */
export const WALKER_BALANCE_GAIN = 10.0
export const WALKER_BALANCE_DAMP = 4.0
export const WALKER_BALANCE_MAX = 3.0

/**
 * Pitching (forwards and backwards) is the axis a walker can fight best: its feet are in line with the
 * tilt, so the pull is up to this, and it also takes a catching step. The step is a push towards where
 * the top is falling, by `WALKER_CATCH_GAIN` m/s² per unit of lean from where it should be and
 * `WALKER_CATCH_DAMP` per rad/s of fall, at most `WALKER_CATCH_MAX`.
 */
export const WALKER_BALANCE_MAX_PITCH = 6.0
export const WALKER_CATCH_GAIN = 20.0
export const WALKER_CATCH_DAMP = 3.0
export const WALKER_CATCH_MAX = 8.0

/**
 * Lean (up's horizontal part along the heading beyond its walking lean) and fall rate (rad/s) it
 * tolerates before it takes a step, so a walk's own lean and its pushes are left alone.
 */
export const WALKER_CATCH_LEAN_MARGIN = 0.15
export const WALKER_CATCH_FALL_MARGIN = 0.5

/** Extra lean into its heading at top speed, as a tilt: the walker walks a little forward of its feet. */
export const WALKER_CRUISE_LEAN = 0.08

/**
 * How much of the tilt that would match its push it leans by: the push goes through the middle of
 * the hull, so the lean is for show and for how it handles.
 */
export const WALKER_ACCEL_LEAN = 0.5

/** A blast pushes a walker at its shoulders, this far above the middle of the hull, so it tips it. */
export const WALKER_BLAST_LEVER = 3.0

/**
 * Below this `up_y` it has fallen: the legs no longer push or drive, and only a weak pull
 * (`WALKER_GETUP_SHARE` of the full one) stands it up again.
 */
export const WALKER_FALLEN_UP = 0.6
export const WALKER_GETUP_SHARE = 0.25

/**
 * The chin guns: the driver aims them with the look, and the upper body turns to point them, up to
 * `WALKER_TORSO_LIMIT` either side of the way the legs face (and `WALKER_GUN_PITCH_MIN..=_MAX` up and
 * down, radians, hull frame), at `WALKER_GUN_SLEW` rad/s. `WALKER_GUN_PIVOT` is where the upper body turns
 * in the hull frame and `WALKER_GUN_LENGTH` how far the muzzle is from it. A round is a cannon shell, as
 * in a helicopter's nose gun, fired every `WALKER_GUN_COOLDOWN_TICKS` and scattered by up to `WALKER_GUN_SPREAD`.
 */
export const WALKER_TORSO_LIMIT = 0.52
export const WALKER_GUN_PITCH_MIN = -0.5
export const WALKER_GUN_PITCH_MAX = 0.4
export const WALKER_GUN_SLEW = 2.5
export const WALKER_GUN_PIVOT = [0.0, 0.1, 0.0] as const
export const WALKER_GUN_LENGTH = 3.7

/**
 * The two barrels hang `WALKER_GUN_BARREL_DROP` below the pivot, `WALKER_GUN_BARREL_X` either side of
 * it, and fire in turn.
 */
export const WALKER_GUN_BARREL_X = 0.3
export const WALKER_GUN_BARREL_DROP = 1.0
export const WALKER_GUN_COOLDOWN_TICKS = Math.trunc(TICK_RATE / 4)
export const WALKER_GUN_SPREAD = 0.01

/**
 * The hen (`KIND_HEN`): a giant robot hen on the walker's legs, one driver in a cockpit on its back. It
 * walks and balances exactly as the walker does (the `WALKER_*` legs, spring and balance), and adds
 * wings: a leap off the ground (Space), flapping that holds a slow climb while Space stays held in the
 * air, and a glide that holds its fall to a slow sink once the flapping has run out. Jumping and
 * flapping drain the wings' stamina, which comes back while it stands with Space let go.
 */
export const HEN_SEATS = 1
export const HEN_HALF_EXTENTS = WALKER_HALF_EXTENTS

/** The wings' stamina, as a share of full. */
export const HEN_STAMINA = 1.0

/**
 * Upward speed a leap adds, m/s (about a 6 m leap), plus this much along its heading at full stick
 * (half of it backwards), and the stamina it costs.
 */
export const HEN_JUMP_SPEED = 11.0
export const HEN_JUMP_FORWARD = 6.0
export const HEN_JUMP_COST = 0.2

/**
 * Flapping, once the leap has topped out, holds this upward speed, m/s, chasing it this hard (per
 * second) by at most this much on top of gravity's worth, m/s², and uses this share of the stamina
 * per second: about three seconds of it.
 */
export const HEN_FLAP_CLIMB = 1.5
export const HEN_FLAP_GAIN = 3.0
export const HEN_FLAP_ACCEL = 6.0
export const HEN_FLAP_DRAIN = 0.33

/**
 * Held in the air, the wings carry it forward towards this speed at full stick, m/s, by at most
 * this much, m/s².
 */
export const HEN_GLIDE_SPEED = 12.0
export const HEN_GLIDE_ACCEL = 4.0

/**
 * With no stamina left the wings spread and hold its fall to this sink, m/s, chasing it this hard,
 * per second.
 */
export const HEN_GLIDE_SINK = 4.0
export const HEN_GLIDE_GAIN = 3.0

/**
 * In the air the wings keep it level, leaning it this far (radians, roughly) with the stick, levelling
 * it this hard and this fast (per second), and turn it at this yaw rate, rad/s.
 */
export const HEN_AIR_TILT = 0.2
export const HEN_AIR_LEVEL = 4.0
export const HEN_AIR_RESPONSE = 6.0
export const HEN_AIR_YAW = 1.2

/** Stamina comes back at this share per second while a foot is down and Space is let go. */
export const HEN_STAMINA_REFILL = 0.35

/**
 * Eggs: a clutch of `HEN_EGGS`, laid one at most every `HEN_EGG_COOLDOWN_TICKS` from `HEN_VENT` (hull
 * frame, under the tail) with the hull's velocity and `HEN_EGG_TOSS` m/s backwards, and one growing back
 * every `HEN_EGG_REGROW_TICKS`. An egg falls and bounces like a grenade (`HEN_EGG_RADIUS`, keeping
 * `HEN_EGG_BOUNCE` of its speed), bursts when `HEN_EGG_FUSE_TICKS` run out, and at once when it meets
 * something at more than `HEN_EGG_SMASH_SPEED` m/s, which a laid egg never does but a dropped one does.
 */
export const HEN_EGGS = 6
export const HEN_EGG_COOLDOWN_TICKS = Math.trunc(TICK_RATE / 2)
export const HEN_EGG_REGROW_TICKS = TICK_RATE * 4
export const HEN_VENT = [0.0, -1.0, 2.0] as const
export const HEN_EGG_TOSS = 3.0
export const HEN_EGG_RADIUS = 0.35
export const HEN_EGG_BOUNCE = 0.3
export const HEN_EGG_FUSE_TICKS = TICK_RATE * 2
export const HEN_EGG_SMASH_SPEED = 16.0

/** An egg's blast, as a plane's bomb. */
export const HEN_EGG_BLAST_RADIUS = 6.0
export const HEN_EGG_DAMAGE = 110.0
export const HEN_EGG_KNOCK = 16.0

/**
 * The helicopter: a hull flown by a pilot (seat 0) with a gunner (seat 1) on the nose gun. There are no
 * wheels; the rotor is a thrust along the hull's up axis, and the pilot leans the hull to go
 * anywhere. Same body axes as the car.
 */
export const HELI_SEATS = 2
export const HELI_HALF_EXTENTS = [1.0, 0.8, 2.4] as const

/** How far the rotor disc leans at full stick: tangent of the angle, forward and sideways. */
export const HELI_TILT = 0.5
export const HELI_BANK = 0.35

/** Yaw rate at full stick, rad/s. */
export const HELI_YAW_RATE = 1.4

/**
 * Vertical speed the climb input asks for and the descent input asks for, m/s. With neither
 * held the rotor holds altitude, whatever speed it had. `HELI_VERT_GAIN` (per second) turns the
 * speed error into an acceleration, which never exceeds `HELI_VERT_ACCEL`, m/s².
 */
export const HELI_CLIMB_SPEED = 6.0
export const HELI_SINK_SPEED = 4.0
export const HELI_VERT_GAIN = 1.5
export const HELI_VERT_ACCEL = 5.0

/** Climbing fades out over the last 20 m below this height, and above it the rotor pushes down. */
export const HELI_CEILING = 120.0

/**
 * How firmly the hull levels out (rad/s of correction per unit of lean error), and how fast the
 * angular velocity follows it, per second.
 */
export const HELI_LEVEL_GAIN = 4.0
export const HELI_RESPONSE = 8.0

/**
 * The FPV drone: a quadcopter the size of a dinner plate. It is flown by velocity: forward stick asks
 * for `DRONE_SPEED` along where its pilot looks (diving or climbing with the look's pitch), strafe for
 * `DRONE_STRAFE_SPEED` sideways, jump and run for `DRONE_CLIMB_SPEED` / `DRONE_SINK_SPEED` straight up
 * or down, and nothing held for a hover. The rotors close the gap at `DRONE_ACCEL_GAIN` per second, never
 * pushing harder than `DRONE_MAX_ACCEL` m/s² beyond holding it up, and the hull leans the way they push.
 * The look turns it: `DRONE_YAW_GAIN` rad/s per radian off, at most `DRONE_YAW_RATE`.
 */
export const DRONE_SEATS = 1
export const DRONE_HALF_EXTENTS = [0.22, 0.06, 0.22] as const
export const DRONE_SPEED = 30.0
export const DRONE_STRAFE_SPEED = 12.0
export const DRONE_CLIMB_SPEED = 10.0
export const DRONE_SINK_SPEED = 10.0
export const DRONE_ACCEL_GAIN = 2.0
export const DRONE_MAX_ACCEL = 18.0
export const DRONE_YAW_GAIN = 6.0
export const DRONE_YAW_RATE = 4.0

/** Climbing fades out over the last 20 m below this height, and above it the rotors push down. */
export const DRONE_CEILING = 150.0

/**
 * How firmly the hull follows its lean (rad/s per unit of error), and how fast the angular velocity
 * follows that, per second.
 */
export const DRONE_LEVEL_GAIN = 8.0
export const DRONE_RESPONSE = 14.0

/** Air drag on the body, which is all that slows a drone whose motors are off. */
export const DRONE_DAMPING = 0.1

/**
 * The loitering drone: a delta wing about 2.5 m across and 3.5 m long, with a pusher prop. It flies as
 * a simplified, nimble plane: always at `LOITER_SPEED` along its nose, with no throttle. The wings and
 * motor close any gap between its velocity and that at `LOITER_GRIP` per second, never harder than
 * `LOITER_MAX_ACCEL` m/s², so it goes where its nose points, and they hold it up whatever its attitude.
 * The nose is steered as a plane's is: the mouse sets where to go and it banks into the turn and pulls
 * (see `autopilot`), and W/S pitch and A/D roll by hand while held. It turns at most `LOITER_PITCH_RATE`,
 * `LOITER_YAW_RATE` and `LOITER_ROLL_RATE` rad/s about its own axes; its angular velocity follows the
 * controls at `LOITER_RESPONSE` per second.
 */
export const LOITER_SEATS = 1
export const LOITER_HALF_EXTENTS = [1.25, 0.25, 1.75] as const
export const LOITER_SPEED = 35.0
export const LOITER_GRIP = 10.0
export const LOITER_MAX_ACCEL = 80.0
export const LOITER_PITCH_RATE = 2.0
export const LOITER_ROLL_RATE = 4.0
export const LOITER_YAW_RATE = 0.8
export const LOITER_RESPONSE = 8.0

/**
 * The most the autopilot banks it into a turn, radians (about 57 degrees): less than the plane, so a
 * hard reversal stays a turn rather than a roll on to its back.
 */
export const LOITER_MAX_BANK = 1.0

/** Climbing fades out over the last 20 m below this height, and above it it is pushed down. */
export const LOITER_CEILING = 500.0

/** Air drag on the body, which is all that slows one whose motor is off. */
export const LOITER_DAMPING = 0.05

/**
 * The nose gun: a four-barrel autocannon on a chin mount. `HELI_GUN_PIVOT` is the mount in the hull
 * frame and `HELI_GUN_LENGTH` how far the muzzle is from it. The gunner's look direction aims it, within
 * `HELI_GUN_YAW_LIMIT` either side of the nose and `HELI_GUN_PITCH_MIN..=HELI_GUN_PITCH_MAX` (radians, the
 * hull's frame), at `HELI_GUN_SLEW` rad/s. It fires a shell (`SHELL_SPEED`, same fall and lifetime) every
 * `HELI_GUN_COOLDOWN_TICKS`, three a second, scattered by up to `HELI_GUN_SPREAD` radians. It bursts
 * like a tank shell on a smaller scale: a third of the radius and a third of the damage.
 */
export const HELI_GUN_PIVOT = [0.0, -0.6, -2.0] as const
export const HELI_GUN_LENGTH = 1.3
export const HELI_GUN_YAW_LIMIT = 1.0
export const HELI_GUN_PITCH_MIN = -0.75
export const HELI_GUN_PITCH_MAX = 0.2
export const HELI_GUN_SLEW = 3.0
export const HELI_GUN_COOLDOWN_TICKS = Math.trunc(TICK_RATE / 3)
export const HELI_GUN_SPREAD = 0.01

/**
 * The rocket pods under the stub wings, fired by the pilot (seat 0) on the fire button. `HELI_POD_PIVOTS`
 * are the four tube mouths in the hull frame, used in turn, one rocket every `HELI_ROCKET_COOLDOWN_TICKS`.
 * A rocket leaves at `HELI_ROCKET_LAUNCH_SPEED` over the helicopter's own velocity, along the nose, and
 * is driven at `ROCKET_ACCEL` m/s² up to `ROCKET_MAX_SPEED`: it flies straight, no gravity.
 */
export const HELI_POD_PIVOTS = [[-1.0, -0.39, -0.3], [1.0, -0.39, -0.3], [-1.75, -0.525, -0.3], [1.75, -0.525, -0.3]] as const
export const HELI_ROCKET_COOLDOWN_TICKS = Math.trunc(TICK_RATE / 5)
export const HELI_ROCKET_LAUNCH_SPEED = 20.0
export const ROCKET_ACCEL = 110.0
export const ROCKET_MAX_SPEED = 170.0

/**
 * The triplane: a Fokker Dr.I stretched into a two-seater, flown by a pilot (seat 0) who also fires the
 * two forward guns and drops the bombs, with a gunner (seat 1) on a ring-mounted gun behind. It rolls on
 * ray-cast wheels like a car, and in the air its wings hold it up once it is fast enough (see `wing` in
 * `physics/vehicle.rs`). Same body axes as the car.
 */
export const PLANE_SEATS = 2

/** The fuselage box. The wings are a second, weightless box (`PLANE_WING_*`), centred in the hull frame. */
export const PLANE_HALF_EXTENTS = [0.5, 0.6, 2.8] as const
export const PLANE_WING_CENTRE = [0.0, 0.45, -1.1] as const
export const PLANE_WING_HALF_EXTENTS = [3.6, 0.85, 0.55] as const
export const PLANE_WHEEL_RADIUS = 0.35
export const PLANE_TRACK_HALF = 0.8

/** The main gear ahead of the centre, the tail skid behind it. The front pair steers on the ground. */
export const PLANE_AXLES_Z = [-1.5, 2.4] as const
export const PLANE_HARD_POINT_Y = -0.5
export const PLANE_SUSPENSION_REST = 0.3
export const PLANE_MAX_STEER = 0.5

/**
 * Acceleration the engine gives at full throttle, m/s². With the stick released it runs at
 * `PLANE_CRUISE_THROTTLE` of that, and with the stick pulled back the blip switch cuts it to `PLANE_IDLE_THROTTLE`.
 */
export const PLANE_THRUST = 6.5
export const PLANE_CRUISE_THROTTLE = 0.6
export const PLANE_IDLE_THROTTLE = 0.03

/** Air drag per (m/s)², which tops a level flight out at about 45 m/s on full throttle and 35 at cruise. */
export const PLANE_DRAG = PLANE_THRUST / (45.0 * 45.0)

/** Below this airspeed (m/s) the wings stall: their lift fades out over the last 30% of it and the nose drops. */
export const PLANE_STALL_SPEED = 17.0

/** Above this airspeed the wings carry the plane level by themselves; below it they carry less, by the square. */
export const PLANE_LEVEL_SPEED = 28.0

/** The speed the controls are tuned for: they lose authority below it, by its square. */
export const PLANE_CRUISE_SPEED = 35.0

/**
 * How fast the wings turn the flight path towards where the nose points, per second, and how
 * fast the fin kills a sideslip.
 */
export const PLANE_AOA_GAIN = 4.0
export const PLANE_SIDE_GAIN = 3.0

/** The most the wings can pull, m/s² (about 4 g). */
export const PLANE_MAX_LIFT = 40.0

/** Turn rates at full deflection, rad/s, and how fast the body's spin follows them, per second. */
export const PLANE_PITCH_RATE = 1.4
export const PLANE_ROLL_RATE = 3.0
export const PLANE_YAW_RATE = 0.5
export const PLANE_RESPONSE = 6.0

/** How hard a stalled plane drops its nose towards where it is going, rad/s. */
export const PLANE_STALL_DROP = 1.5

/** Where a plane with nobody at the stick points its nose, radians below the horizon: it glides down. */
export const PLANE_GLIDE_PITCH = -0.2

/** Off-target angle (radians) beyond which the plane banks into the turn instead of levelling its wings. */
export const PLANE_BANK_ANGLE = 0.2

/** The steepest bank the plane rolls into by itself, radians (about 75°): past it the pull does the turning. */
export const PLANE_MAX_BANK = 1.3

/** Rate asked for per radian the target is off the nose, as a share of the full rate. */
export const PLANE_AIM_GAIN = 2.0

/** How steeply a pilot's mouse can ask the plane to climb or dive, radians (75 degrees). */
export const PLANE_AIM_MAX_PITCH = 1.3

/** With the stick centred the nose settles back to the horizon at this share of full pitch per radian off it. */
export const PLANE_LEVEL_GAIN = 1.5

/** The engine fades out over the last 20 m below this height; the boundary walls stop at 200 m. */
export const PLANE_CEILING = 160.0

/** Within this many metres of the edge of the world the plane is steered back towards the middle. */
export const PLANE_EDGE_MARGIN = 120.0

/** A plane is parked where a straight, flat, clear run of at least this many metres lies ahead. */
export const PLANE_RUNWAY = 80.0

/**
 * The two forward guns on the cowling, which fire in turn along the nose. `PLANE_GUN_COOLDOWN_TICKS`
 * between rounds makes twelve a second between them. A round is a hitscan out to `PLANE_GUN_RANGE`.
 */
export const PLANE_GUN_PIVOTS = [[-0.16, 0.72, -2.0], [0.16, 0.72, -2.0]] as const
export const PLANE_GUN_COOLDOWN_TICKS = 5
export const PLANE_GUN_RANGE = 300.0
export const PLANE_GUN_SPREAD = 0.012

/** What a round does to a stickman, and to a vehicle: a plane is all canvas, anything else shrugs it off. */
export const PLANE_GUN_DAMAGE = 7
export const PLANE_GUN_PLANE_DAMAGE = 4
export const PLANE_GUN_VEHICLE_DAMAGE = 1

/**
 * Guns heat by this much a round and cool by `GUN_COOL` a tick, out of 255. At 255 they jam until
 * they have cooled to `GUN_UNJAM`: about four seconds of fire, then three of waiting.
 */
export const PLANE_GUN_HEAT = 10.5
export const GUN_COOL = 1.0
export const GUN_UNJAM = 77.0

/**
 * The gunner's ring mount behind the pilot: a pivot in the hull frame, the barrel's length, how far
 * it lifts and dips, and its rate. It turns all the way round, but holds fire within
 * `PLANE_REAR_DEAD_ZONE` of the nose, where the pilot sits.
 */
export const PLANE_REAR_PIVOT = [0.0, 0.85, 0.7] as const
export const PLANE_REAR_LENGTH = 0.9
export const PLANE_REAR_PITCH_MIN = -0.35
export const PLANE_REAR_PITCH_MAX = 1.0
export const PLANE_REAR_SLEW = 4.0
export const PLANE_REAR_DEAD_ZONE = 0.6
export const PLANE_REAR_COOLDOWN_TICKS = 8
export const PLANE_REAR_HEAT = 16.5

/**
 * Bombs on the rack under the fuselage, one dropped at most every `PLANE_BOMB_COOLDOWN_TICKS`. A
 * bomb leaves with the plane's velocity and falls; it bursts on the first thing it meets. The
 * rack fills again on the ground once the plane has stood still for `PLANE_REARM_TICKS`.
 */
export const PLANE_BOMBS = 4
export const PLANE_BOMB_PIVOT = [0.0, -0.85, -0.3] as const
export const PLANE_BOMB_COOLDOWN_TICKS = 30

/** The rack holds on to its bombs below this height above the ground, metres. */
export const PLANE_BOMB_MIN_HEIGHT = 4.0
export const PLANE_REARM_TICKS = 3 * TICK_RATE
export const BOMB_RADIUS = 6.0
export const BOMB_DAMAGE = 110.0
export const BOMB_KNOCK = 16.0

/** The tank gun. */
export const SHELL_COOLDOWN_TICKS = 90

/**
 * Speed a shell leaves the muzzle at, m/s. It falls under `GRAVITY` from there, so at 100 m it has
 * dropped about 5 m and at 200 m about 20.
 */
export const SHELL_SPEED = 140.0

/**
 * A sniper round flies at twice a tank shell's speed, m/s, and falls under a quarter of gravity, so
 * at 200 m (0.7 s) it has dropped about 1 m.
 */
export const BULLET_SPEED = 2.0 * SHELL_SPEED
export const BULLET_GRAVITY_SCALE = 0.25

/** A gun fired with aim held scatters over this fraction of its hip-fire cone. */
export const AIM_SPREAD_SCALE = 0.25

/** How long a shell flies before it bursts where it is. */
export const SHELL_TTL_TICKS = 6 * TICK_RATE

/** Speed a fired shell takes from the tank, m/s, backwards along the barrel. */
export const SHELL_RECOIL = 1.6

/**
 * The field mortar (`KIND_MORTAR`): a towed, unpowered two-wheeler with one gunner, who aims its bearing
 * with the look and its range with the dial. It is towed by a car's rear tow point through a spherical joint.
 */
export const MORTAR_SEATS = 1
export const MORTAR_HALF_EXTENTS = [1.0, 0.5, 1.5] as const
export const MORTAR_WHEEL_RADIUS = 0.45
export const MORTAR_TRACK_HALF = 0.85

/** Where the mount sits in the hull frame, and the barrel's length from it. */
export const MORTAR_GUN_PIVOT = [0.0, 0.7, -0.2] as const
export const MORTAR_BARREL_LENGTH = 3.2

/**
 * Elevation travel, radians: a high lob at the bottom, nearly vertical at the top. The gunner never sets
 * it by hand: the range dial does (`lob_pitch` in gunnery).
 */
export const MORTAR_PITCH_MIN = 0.785
export const MORTAR_PITCH_MAX = 1.484

/**
 * The range dial, metres: W and S walk it at `MORTAR_RANGE_RATE` while `BUTTON_RANGE_UP` /
 * `BUTTON_RANGE_DOWN` are held. The longest lob is 245 m (`GRAVITY` and the muzzle speed at 45 degrees),
 * the shortest the high lob reaches is about 42 m, so the dial stops at 50.
 */
export const MORTAR_RANGE_START = 120.0
export const MORTAR_RANGE_MIN = 50.0
export const MORTAR_RANGE_MAX = 245.0
export const MORTAR_RANGE_RATE = 60.0

/** How fast the mount follows the gunner's look, rad/s. Quick enough that the barrel stays on the crosshair. */
export const MORTAR_TRAVERSE_SLEW = 3.0

/** Muzzle speed of a mortar bomb, m/s. Under `GRAVITY` it reaches about 245 m at 45 degrees and 40 m at 85. */
export const MORTAR_SHELL_SPEED = 70.0

/** A reload is long: the gunner waits between bombs. */
export const MORTAR_COOLDOWN_TICKS = 7 * TICK_RATE
export const MORTAR_SHELL_TTL_TICKS = 10 * TICK_RATE

/** Speed the mortar is shoved back at when it fires, m/s, along the barrel's horizontal heading. */
export const MORTAR_RECOIL = 3.0

/** Blast of a mortar bomb: wider and harder than a tank shell. */
export const MORTAR_RADIUS = 11.0
export const MORTAR_DAMAGE = 260.0
export const MORTAR_KNOCK = 30.0

/** The tow point sits this far behind a car's rear face, metres. */
export const TOW_BACK = 0.25

/** How close a mortar's tongue must be to a car's tow point to hitch, metres. */
export const HITCH_REACH = 2.5

/** A hitch or an unhitch needs the car at a near standstill, m/s. */
export const HITCH_MAX_SPEED = 1.5
export const EXPLOSION_RADIUS = 7.0

/** The blast of an autocannon round: a third of a tank shell's radius, a third of its damage. */
export const CANNON_RADIUS = EXPLOSION_RADIUS / 3.0

/** Knockback speed at the centre of a blast, m/s; it fades to zero at the edge. */
export const EXPLOSION_KNOCK = 18.0
export const CANNON_KNOCK = EXPLOSION_KNOCK / 2.0

/** Combat. Every player starts a life with `PLAYER_MAX_HP` and dies at zero. */
export const PLAYER_MAX_HP = 100

/** Ticks a dead player waits before respawning. */
export const RESPAWN_TICKS = 3 * TICK_RATE

/**
 * Guns are hitscans fired from the eye along the look direction; their stats are in `items`.
 * Height of a stickman's eye above its capsule centre, where a rifle shot leaves.
 */
export const EYE_HEIGHT = 0.6

/** How far a crouch lowers the eye. The capsule keeps its size, so collision is unchanged. */
export const CROUCH_EYE_DROP = 0.3

/** A prone stickman's eye is 0.2 m above its feet, so this is relative to the capsule centre. */
export const PRONE_EYE_HEIGHT = 0.2 - PRONE_CENTRE

/**
 * The on-foot camera sits behind a pivot this far to the right of the capsule centre and this far
 * above it. The server rebuilds the camera ray from it, so a shot lands where the crosshair is.
 * Mirrors `ON_FOOT_SHOULDER` and `ON_FOOT_HEIGHT` in the client.
 */
export const ON_FOOT_SHOULDER = 0.9
export const ON_FOOT_HEIGHT = 1.5

/**
 * Seated players. Each seat of a vehicle with open seats has a head point in the hull frame, where
 * the client draws the sitting body's head (a car's `seat_N` markers, built by `build_car.py`). The
 * body that can be shot is a short upright capsule hanging below it: head and shoulders down to the
 * waist, so the hull box in front of the lower body takes the rounds aimed there.
 */
export const JEEP_SEAT_HEADS = [[-0.4, 0.62, -0.3], [0.4, 0.62, -0.3], [-0.4, 0.62, 0.75], [0.4, 0.62, 0.75]] as const
export const SPORTS_SEAT_HEADS = [[-0.35, 0.4, 0.1], [0.35, 0.4, 0.1]] as const
export const ROADSTER_SEAT_HEADS = [[-0.33, 0.42, 0.12], [0.33, 0.42, 0.12]] as const

/** The pilot raised behind and above the gunner in the nose. */
export const HELI_SEAT_HEADS = [[0.0, 0.45, -0.45], [0.0, 0.1, -1.55]] as const

/** The pilot behind the wings, the gunner standing in the ring. */
export const PLANE_SEAT_HEADS = [[0.0, 0.98, -0.45], [0.0, 1.15, 0.7]] as const

/** The mortar's gunner rides its mount, so this is in the mount's frame: `MORTAR_GUN_PIVOT` turned by the gun's yaw. */
export const MORTAR_SEAT_HEAD = [0.6, 0.45, 0.6] as const

/**
 * The seated capsule's centre is this far below the head point, and it spans `SEATED_HALF_HEIGHT`
 * above and below that, plus `SEATED_RADIUS`.
 */
export const SEATED_CENTRE_DROP = 0.3
export const SEATED_HALF_HEIGHT = 0.25
export const SEATED_RADIUS = 0.25

/** A round that hits a seated body above this far below its head point hit the head. */
export const SEATED_HEAD_ZONE = 0.12

/**
 * A passenger who can shoot looks over their own shoulder: the camera's pivot is this far to the
 * right of their head point and this far above it, and the camera sits `PASSENGER_DISTANCE` behind.
 * The server rebuilds the crosshair ray from it, as on foot.
 */
export const PASSENGER_SHOULDER = 0.6
export const PASSENGER_HEIGHT = 0.5
export const PASSENGER_DISTANCE = 3.0

/**
 * A hurt building is drawn in this many worse states. Stage `n` begins when its health falls to
 * `DAMAGE_STAGE_HEALTH[n - 1]` of full (percent); the last is about to collapse.
 */
export const DAMAGE_STAGES = 3
export const DAMAGE_STAGE_HEALTH = [66, 33, 15] as const

/** A blast hurting a building by at least this much leaves a scorch mark, and is announced. */
export const DAMAGE_MARK_MIN = 5

/**
 * Damage at the centre of an explosion; it falls off linearly to zero at `EXPLOSION_RADIUS`.
 * More than a full life, so a shell that lands within about 1.4 m kills outright.
 */
export const EXPLOSION_DAMAGE = 125.0
export const CANNON_DAMAGE = EXPLOSION_DAMAGE / 3.0

/** Most shot events one snapshot carries; the rest are cosmetic and dropped. A shotgun pull is ten. */
export const MAX_SHOT_EVENTS = 24

/** Most bullet spawns one snapshot carries (nearest first); they are cheap, and are sent twice. */
export const MAX_BULLET_SPAWNS = 32

/** The rocket launcher: a projectile that flies straight and bursts on the first thing it meets. */
export const ROCKET_SPEED = 45.0
export const ROCKET_TTL_TICKS = 4 * TICK_RATE

/** How far in front of the eye a rocket starts. */
export const ROCKET_MUZZLE = 1.0
export const ROCKET_BLAST_RADIUS = 5.0
export const ROCKET_DAMAGE = 110.0

/** Knockback speed at the centre of a rocket's blast, m/s. */
export const ROCKET_KNOCK = 16.0

/** Rockets in flight at once, which bounds the projectile list in a snapshot. */
export const MAX_ROCKETS = 8

/** How long a destroyed vehicle burns before it is put back at its home, and the blast it goes up in. */
export const WRECK_TICKS = 10 * TICK_RATE

/** Angular speed (rad/s) a wreck is thrown into, about a horizontal axis: about one turn in the air. */
export const WRECK_SPIN = 6.5
export const WRECK_BLAST_RADIUS = 8.0
export const WRECK_BLAST_DAMAGE = 100.0
export const WRECK_BLAST_KNOCK = 14.0

/**
 * How far back the server will rewind other players to meet a shooter's view, in ticks (250 ms).
 * A shot never reaches further into the past than this, so a laggy shooter has a cap on how far
 * ahead of themselves they can lead their targets.
 */
export const MAX_REWIND_TICKS = 15

/** How far from home it wanders, metres. */
export const ROOSTER_WANDER_RADIUS = 8.0

/** Walking speed, m/s, and how fast it turns, rad/s. */
export const ROOSTER_SPEED = 1.2
export const ROOSTER_TURN_RATE = 4.0
export const ROOSTER_HP = 30

/** A killed rooster is back at home after this long. */
export const ROOSTER_RESPAWN_TICKS = 60 * TICK_RATE

/** Length of a crow; the client's `crow` clip is the same length. */
export const ROOSTER_CROW_TICKS = 100

/** Hit capsule: centre this high above its feet, metres. */
export const ROOSTER_CAPSULE_CENTRE = 0.4
export const ROOSTER_CAPSULE_HALF_HEIGHT = 0.1
export const ROOSTER_CAPSULE_RADIUS = 0.3
