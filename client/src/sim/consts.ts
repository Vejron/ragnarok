/** Mirrors `crates/shared/src/consts.rs`. Keep the two in step. */

export const PROTOCOL_VERSION = 14

export const TICK_RATE = 60
export const TICK_DT = 1 / TICK_RATE
export const TICK_MS = 1000 / TICK_RATE
export const SNAPSHOT_EVERY = 2

/** Mirrors `MAX_PLAYERS` in consts.rs: the room's capacity, and so the most stickmen there can be to draw. */
export const MAX_PLAYERS = 64
export const MAX_PACKET = 1200
/** Things farther than this from the player are not sent; mirrors consts.rs. */
export const INTEREST_RADIUS = 300
export const INTEREST_HYSTERESIS = 40
/** Snapshots the client keeps to rebuild deltas from. At least the server's `BASELINE_WINDOW` (32). */
export const BASELINE_WINDOW_CLIENT = 64
export const INPUT_REDUNDANCY = 4

export const BUTTON_JUMP = 1 << 0
export const BUTTON_RUN = 1 << 1
export const BUTTON_USE = 1 << 2
export const BUTTON_FIRE = 1 << 3
/** Aim held (right mouse): looks down the scope of a gun that has one. */
export const BUTTON_ALT = 1 << 4
/** Held to crouch: slower, and a lower eye. */
export const BUTTON_CROUCH = 1 << 5
/** One press, sent with run and crouch held: starts a slide. */
export const BUTTON_SLIDE = 1 << 6
/** One press: reload the selected gun. */
export const BUTTON_RELOAD = 1 << 7

export const WALK_SPEED = 4.0
export const RUN_SPEED = 7.5
/** Crouching overrides running. */
export const CROUCH_SPEED = 2.0
/** Mirrors the slide constants in `consts.rs`. */
export const SLIDE_SPEED = 12.0
export const SLIDE_KEEP = 0.96
export const SLIDE_RETRIGGER = 1.0
/** Above this leftover velocity a grounded crouching player is sliding. */
export const SLIDE_MIN_SPEED = 2.0
export const JUMP_SPEED = 6.5
export const GRAVITY = 20.0
export const MAX_FALL_SPEED = 50.0

/** Knockback bleeds off by this factor per tick. Mirrors `KNOCK_*` in consts.rs. */
export const KNOCK_KEEP_GROUND = 0.92
export const KNOCK_KEEP_AIR = 0.99
export const KNOCK_MIN_SPEED = 0.05
export const KNOCK_MAX_SPEED = 20.0

export const CAPSULE_HALF_HEIGHT = 0.5
export const CAPSULE_RADIUS = 0.35

export const CONTROLLER_OFFSET = 0.02
export const MAX_SLOPE_CLIMB_DEG = 50.0
export const AUTOSTEP_HEIGHT = 0.35
export const AUTOSTEP_MIN_WIDTH = 0.2
/** How far below its feet a standing character is pulled back onto the ground. Mirrors `SNAP_DISTANCE`. */
export const SNAP_DISTANCE = 0.3

/** Capsule-centre height when standing on y = 0. Mirrors `STANDING_Y` in `physics.rs`. */
export const STANDING_Y = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS + CONTROLLER_OFFSET

/** Vehicles. Body-local axes: +X right, +Y up, -Z forward. */
export const CAR_SEATS = 4
export const ENTER_RADIUS = 4.0
/**
 * The jeep: tall on big wheels with plenty of ground clearance. Half extents of the chassis box;
 * `*_RIDE_HEIGHT` is the chassis centre above flat ground once the suspension has settled.
 */
export const JEEP_HALF_EXTENTS = [1.0, 0.42, 1.95] as const
export const JEEP_WHEEL_RADIUS = 0.55
export const JEEP_TRACK_HALF = 0.9
export const JEEP_AXLE_Z = 1.25
export const JEEP_HARD_POINT_Y = 0.0
export const JEEP_SUSPENSION_REST = 0.4
export const JEEP_RIDE_HEIGHT = 0.857
export const JEEP_MAX_STEER = 0.5
export const JEEP_MAX_SPEED = 28.0

/** The sports car: a low two-seater with small wheels, a big engine and a high top speed. */
export const SPORTS_SEATS = 2
export const SPORTS_HALF_EXTENTS = [0.95, 0.3, 2.2] as const
export const SPORTS_WHEEL_RADIUS = 0.32
export const SPORTS_TRACK_HALF = 0.82
export const SPORTS_AXLE_Z = 1.45
export const SPORTS_HARD_POINT_Y = 0.05
export const SPORTS_SUSPENSION_REST = 0.22
export const SPORTS_RIDE_HEIGHT = 0.397
export const SPORTS_MAX_STEER = 0.4
export const SPORTS_MAX_SPEED = 44.0

/** The tank. Mirrors `TANK_*` in consts.rs. Same body axes as the car. */
export const TANK_SEATS = 2
export const TANK_HALF_EXTENTS = [1.3, 0.5, 2.6] as const
export const TANK_WHEEL_RADIUS = 0.45
export const TANK_TRACK_HALF = 1.2
export const TANK_AXLES_Z = [-1.9, 0, 1.9] as const
export const TANK_HARD_POINT_Y = -0.2
export const TANK_SUSPENSION_REST = 0.35
export const TANK_MAX_SPEED = 12.0
/** Yaw rate at full stick, rad/s. */
export const TANK_TURN_RATE = 1.1
/** Where the turret ring sits in the hull frame, and how far the muzzle is from it. */
export const TANK_TURRET_PIVOT = [0, 0.85, 0.3] as const
export const TANK_BARREL_LENGTH = 3.2
/** The solid turret: a cylinder on the ring, so any heading is the same shape. Mirrors consts.rs. */
export const TANK_TURRET_RADIUS = 1.0
export const TANK_TURRET_HALF_HEIGHT = 0.35
/** Barrel elevation limits, radians (+ is up). */
export const TANK_BARREL_MIN = -0.12
export const TANK_BARREL_MAX = 0.5
/** The helicopter. Mirrors `HELI_*` in consts.rs. */
export const HELI_SEATS = 2
export const HELI_HALF_EXTENTS = [1.0, 0.8, 2.4] as const
export const HELI_TILT = 0.5
export const HELI_BANK = 0.35
export const HELI_YAW_RATE = 1.4
export const HELI_CLIMB_SPEED = 6.0
export const HELI_SINK_SPEED = 4.0
export const HELI_VERT_GAIN = 1.5
export const HELI_VERT_ACCEL = 5.0
export const HELI_CEILING = 120.0
export const HELI_LEVEL_GAIN = 4.0
export const HELI_RESPONSE = 8.0
/** The nose gun: the mount in the hull frame (X right, Y up, Z back), muzzle distance, travel and range. */
export const HELI_GUN_PIVOT = [0.0, -0.6, -2.0] as const
export const HELI_GUN_LENGTH = 1.3
export const HELI_GUN_YAW_LIMIT = 1.0
export const HELI_GUN_PITCH_MIN = -0.75
export const HELI_GUN_PITCH_MAX = 0.2
export const HELI_GUN_COOLDOWN_TICKS = TICK_RATE / 3
/** Combat. Mirrors the combat block of consts.rs. */
export const PLAYER_MAX_HP = 100
export const RESPAWN_TICKS = 3 * TICK_RATE
/** Height of a stickman's eye above its capsule centre, where a rifle shot leaves. */
export const EYE_HEIGHT = 0.6
/** How far a crouch lowers the eye. Mirrors `CROUCH_EYE_DROP`. */
export const CROUCH_EYE_DROP = 0.3
/** The on-foot camera pivots this far right of and above the capsule centre. The server rebuilds the camera ray from it. Mirrors `ON_FOOT_SHOULDER`. */
export const ON_FOOT_SHOULDER = 0.9
export const ON_FOOT_HEIGHT = 1.5
/** Health of a fresh vehicle of `kind`. Mirrors `vehicle_hp` in protocol.rs. */
export const vehicleMaxHp = (kind: number) => (kind === 2 ? 250 : 100)
export const ROCKET_BLAST_RADIUS = 5.0
export const WRECK_BLAST_RADIUS = 8.0
export const SHELL_COOLDOWN_TICKS = 90
export const EXPLOSION_RADIUS = 7.0
/** An autocannon round bursts a third as wide as a tank shell. */
export const CANNON_RADIUS = EXPLOSION_RADIUS / 3
