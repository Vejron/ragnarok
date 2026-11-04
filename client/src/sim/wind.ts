import { TICK_RATE } from './consts'

/**
 * The wind over open water, mirroring `crates/shared/src/wind.rs`: which way it blows at a server
 * tick, how much of a sail's drive a ship gets from it on a heading, and how far her boom swings out.
 * The sim sails ships on the Rust one; this one draws the sail and the wind gauge. `wind.spec.ts`
 * holds the two together.
 */

/** Which way the wind blows towards on average (radians from +X towards +Z), how far it veers either side, and over how long. */
export const WIND_HEADING = 0.3
const VEER = 0.6
const LOOP_SECONDS = 512
export const WIND_LOOP_TICKS = LOOP_SECONDS * TICK_RATE
/** Into the wind within `CLOSE` a sail has `IRONS` of its drive; all of it from `BEAM`, and `RUN` of it dead before the wind. */
const CLOSE = 0.7
const BEAM = 1.6
const IRONS = 0.08
const RUN = 0.7
const BOOM_MAX = 1.4

/** Which way the wind blows at server tick `tick` (fractional between ticks), a unit `[x, z]`. */
export function wind(tick: number): [number, number] {
  const t = (((tick % WIND_LOOP_TICKS) + WIND_LOOP_TICKS) % WIND_LOOP_TICKS) / TICK_RATE
  const heading = WIND_HEADING + VEER * Math.sin((Math.PI * 2 * t) / LOOP_SECONDS)
  return [Math.cos(heading), Math.sin(heading)]
}

/** How far off the wind a ship heading along `forward` (unit `[x, z]`) sails, radians: 0 into it, π before it, + with it over the left side. */
export function offWind(forward: readonly [number, number], blowing: readonly [number, number]): number {
  const from = [-blowing[0], -blowing[1]] as const
  return Math.atan2(forward[0] * from[1] - forward[1] * from[0], forward[0] * from[0] + forward[1] * from[1])
}

/** The share of a full sail's drive a ship heading along `forward` gets from the wind `blowing`. */
export function pointOfSail(forward: readonly [number, number], blowing: readonly [number, number]): number {
  const off = Math.abs(offWind(forward, blowing))
  if (off <= CLOSE) return IRONS
  if (off < BEAM) {
    const t = (off - CLOSE) / (BEAM - CLOSE)
    return IRONS + (1 - IRONS) * t * t * (3 - 2 * t)
  }
  const t = (off - BEAM) / (Math.PI - BEAM)
  return 1 - (1 - RUN) * t ** 1.5
}

/** How far the boom swings off the centre line, radians (+ towards the left side), let out away from the wind. */
export function boom(forward: readonly [number, number], blowing: readonly [number, number]): number {
  return Math.min(BOOM_MAX, Math.max(-BOOM_MAX, offWind(forward, blowing) * 0.5))
}
