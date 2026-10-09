/** Mirrors `crates/shared/src/consts.rs`. Keep the two in step. */

export const PROTOCOL_VERSION = 5

export const TICK_RATE = 60
export const TICK_DT = 1 / TICK_RATE
export const TICK_MS = 1000 / TICK_RATE
export const SNAPSHOT_EVERY = 2

export const MAX_PACKET = 1200
export const INPUT_REDUNDANCY = 4

export const BUTTON_JUMP = 1 << 0
export const BUTTON_RUN = 1 << 1
export const BUTTON_USE = 1 << 2
export const BUTTON_FIRE = 1 << 3

export const WALK_SPEED = 4.0
export const RUN_SPEED = 7.5
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

/** Capsule-centre height when standing on y = 0. Mirrors `STANDING_Y` in `physics.rs`. */
export const STANDING_Y = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS + CONTROLLER_OFFSET

/** Vehicles. Body-local axes: +X right, +Y up, -Z forward. */
export const CAR_SEATS = 4
export const ENTER_RADIUS = 4.0
/** Half extents of the chassis box. */
export const CAR_HALF_EXTENTS = [0.9, 0.4, 2.1] as const
export const CAR_WHEEL_RADIUS = 0.35
export const CAR_TRACK_HALF = 0.85
export const CAR_AXLE_Z = 1.35
export const CAR_HARD_POINT_Y = -0.1
export const CAR_SUSPENSION_REST = 0.3
export const CAR_MAX_STEER = 0.5
export const CAR_MAX_SPEED = 30.0

/** The tank. Mirrors `TANK_*` in consts.rs. Same body axes as the car. */
export const TANK_SEATS = 2
export const TANK_HALF_EXTENTS = [1.3, 0.5, 2.6] as const
export const TANK_WHEEL_RADIUS = 0.45
export const TANK_TRACK_HALF = 1.2
export const TANK_AXLES_Z = [-1.9, 0, 1.9] as const
export const TANK_HARD_POINT_Y = -0.2
export const TANK_SUSPENSION_REST = 0.35
/** Where the turret ring sits in the hull frame, and how far the muzzle is from it. */
export const TANK_TURRET_PIVOT = [0, 0.85, 0.3] as const
export const TANK_BARREL_LENGTH = 3.2
/** Barrel elevation limits, radians (+ is up). */
export const TANK_BARREL_MIN = -0.12
export const TANK_BARREL_MAX = 0.5
export const SHELL_COOLDOWN_TICKS = 90
export const EXPLOSION_RADIUS = 7.0
export const SHELL_RANGE = 300.0
