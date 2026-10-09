/** Mirrors `crates/shared/src/consts.rs`. Keep the two in step. */

export const PROTOCOL_VERSION = 1

export const TICK_RATE = 60
export const TICK_DT = 1 / TICK_RATE
export const TICK_MS = 1000 / TICK_RATE
export const SNAPSHOT_EVERY = 2

export const MAX_PACKET = 1200
export const INPUT_REDUNDANCY = 4

export const BUTTON_JUMP = 1 << 0
export const BUTTON_RUN = 1 << 1
export const BUTTON_USE = 1 << 2

export const WALK_SPEED = 4.0
export const RUN_SPEED = 7.5
export const JUMP_SPEED = 6.5
export const GRAVITY = 20.0
export const MAX_FALL_SPEED = 50.0

export const CAPSULE_HALF_HEIGHT = 0.5
export const CAPSULE_RADIUS = 0.35

export const CONTROLLER_OFFSET = 0.02
export const MAX_SLOPE_CLIMB_DEG = 50.0
export const AUTOSTEP_HEIGHT = 0.35
export const AUTOSTEP_MIN_WIDTH = 0.2

/** Capsule-centre height when standing on y = 0. Mirrors `STANDING_Y` in `physics.rs`. */
export const STANDING_Y = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS + CONTROLLER_OFFSET
