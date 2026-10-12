/**
 * Where a bullet is, from its spawn alone. Mirrors `step_bullet` (`crates/server/src/mechanics/combat.rs`):
 * a tick moves it `v * dt` and then `v.y` loses `GRAVITY * BULLET_GRAVITY_SCALE * dt`, so after `n` ticks
 * it is at `p + v * n * dt - (0, g * dt^2 * n * (n + 1) / 2, 0)`, with `g = GRAVITY * BULLET_GRAVITY_SCALE`.
 * `n` may be fractional, for a frame between two ticks.
 */

import type { BulletSpawn } from '@/net/codec'
import { GRAVITY, TICK_DT, TICK_RATE } from './consts'
import { itemDef } from './items'

/** A bullet falls under this fraction of gravity. */
export const BULLET_GRAVITY_SCALE = 0.25

const G = GRAVITY * BULLET_GRAVITY_SCALE

/** Position, in metres, `n` ticks after the bullet was fired. */
export function bulletPosition(b: BulletSpawn, n: number): [number, number, number] {
  const t = n * TICK_DT
  const drop = 0.5 * G * TICK_DT * TICK_DT * n * (n + 1)
  return [b.x + b.vx * t, b.y + b.vy * t - drop, b.z + b.vz * t]
}

/** Velocity, m/s, `n` ticks after it was fired: the heading of the streak. */
export function bulletVelocity(b: BulletSpawn, n: number): [number, number, number] {
  return [b.vx, b.vy - G * TICK_DT * n, b.vz]
}

/** Ticks it flies before it gives up, like the server's `ttl`. */
export function bulletLifeTicks(b: BulletSpawn): number {
  const speed = Math.hypot(b.vx, b.vy, b.vz)
  const range = itemDef(b.item)?.range ?? 0
  return speed > 0 ? Math.ceil((range / speed) * TICK_RATE) : 0
}

/** About how many ticks after firing it reached `point`, which is where its end event says it stopped. */
export function bulletTicksTo(b: BulletSpawn, point: readonly [number, number, number]): number {
  const speed = Math.hypot(b.vx, b.vy, b.vz)
  if (speed === 0) return 0
  return Math.hypot(point[0] - b.x, point[1] - b.y, point[2] - b.z) / (speed * TICK_DT)
}
