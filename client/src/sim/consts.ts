/**
 * Every constant in `crates/shared/src/consts.rs` is generated into `consts.generated.ts` (run
 * `npm run gen:consts` after editing the Rust) and re-exported here. Only what the server does not
 * have is written by hand below.
 */
import { CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS, CONTROLLER_OFFSET, TICK_RATE } from './consts.generated'

export * from './consts.generated'

export const TICK_MS = 1000 / TICK_RATE

/** Capsule-centre height when standing on y = 0. Mirrors `STANDING_Y` in `physics.rs`. */
export const STANDING_Y = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS + CONTROLLER_OFFSET

/** Health of a fresh vehicle of `kind`. Mirrors `vehicle_hp` in protocol.rs. */
export const vehicleMaxHp = (kind: number) => (kind === 2 ? 250 : kind === 6 ? 70 : kind === 7 ? 200 : kind === 8 ? 220 : kind === 9 ? 150 : 100)
