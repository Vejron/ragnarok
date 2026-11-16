/**
 * Laying a mortar on a mark: the elevation of the high lob that lands a bomb on a point, worked out for
 * the height of that point, as the server's `Dial::pitch` does for level ground. A gunner laying from
 * above sends this elevation in place of the look's pitch while aim is held (`gunnery::turn`).
 */

import {
  AMV_BARREL_LENGTH, AMV_BOMB_RADIUS, AMV_RANGE_MAX, AMV_RANGE_MIN, AMV_RANGE_START, AMV_SHELL_SPEED, GRAVITY, MORTAR_BARREL_LENGTH, MORTAR_RANGE_MAX, MORTAR_RANGE_MIN, MORTAR_RADIUS, MORTAR_RANGE_START, MORTAR_SHELL_SPEED,
  MORTAR_SHELL_TTL_TICKS,
} from './consts'
import type { Vec3 } from './motor'

/** A mortar's range dial and its bomb, as the server's `Dial` and `Fire::Shell` have them. */
export interface Lob {
  /** The bomb's muzzle speed, m/s. */
  speed: number
  /** The shortest and longest range the dial reaches, and where it starts, metres. */
  min: number
  max: number
  start: number
  /** How far the muzzle is from the tube's pivot, metres. */
  length: number
  /** How long a bomb flies before it bursts on its own, ticks. */
  ttl: number
  /** How far its burst reaches, metres: the landing mark's size. */
  radius: number
}

/** The field mortar's. */
export const MORTAR_LOB: Lob = { speed: MORTAR_SHELL_SPEED, min: MORTAR_RANGE_MIN, max: MORTAR_RANGE_MAX, start: MORTAR_RANGE_START, length: MORTAR_BARREL_LENGTH, ttl: MORTAR_SHELL_TTL_TICKS, radius: MORTAR_RADIUS }
/** The AMV's: a slower bomb, out to 200 m. */
export const AMV_LOB: Lob = { speed: AMV_SHELL_SPEED, min: AMV_RANGE_MIN, max: AMV_RANGE_MAX, start: AMV_RANGE_START, length: AMV_BARREL_LENGTH, ttl: MORTAR_SHELL_TTL_TICKS, radius: AMV_BOMB_RADIUS }

/** The elevation of the high lob that lands `range` metres out on level ground, radians: the server's `Dial::pitch`, unclamped. */
export function lobPitch(lob: Lob, range: number): number {
  return 0.5 * (Math.PI - Math.asin(Math.min(1, (GRAVITY * range) / (lob.speed * lob.speed))))
}

/** How far a bomb lobbed at elevation `pitch` (radians) lands on level ground, metres: the server's `Dial::range`. */
export function lobRange(lob: Lob, pitch: number): number {
  return (lob.speed * lob.speed * Math.sin(2 * pitch)) / GRAVITY
}

/** The flattest and steepest elevations the server lays a tube to: the ends of the range dial. */
export function lobPitches(lob: Lob): { low: number; high: number } {
  return { low: lobPitch(lob, lob.max), high: lobPitch(lob, lob.min) }
}

/**
 * The elevation of the high lob that carries a bomb `horizontal` metres out and `rise` metres up (down
 * when negative) from where it leaves the muzzle, radians, kept within what the server lays. A mark out of
 * reach gets the longest throw, 45 degrees.
 */
export function layLob(lob: Lob, horizontal: number, rise: number): number {
  const v2 = lob.speed * lob.speed
  const x = Math.max(horizontal, 1e-3)
  const under = v2 * v2 - GRAVITY * (GRAVITY * x * x + 2 * rise * v2)
  const pitch = under < 0 ? Math.PI / 4 : Math.atan((v2 + Math.sqrt(under)) / (GRAVITY * x))
  const { low, high } = lobPitches(lob)
  return Math.max(low, Math.min(high, pitch))
}

/**
 * Where to lay a tube whose mount is at `pivot` so the bomb lands on `mark`: the bearing (a look's yaw,
 * 0 facing −Z) and the elevation. The muzzle is a barrel's length out along the tube, so it moves as the
 * tube rises; solving twice puts it close enough.
 */
export function layTube(lob: Lob, pivot: Vec3, mark: Vec3): { yaw: number; pitch: number } {
  const dx = mark[0] - pivot[0]
  const dz = mark[2] - pivot[2]
  const along = Math.hypot(dx, dz)
  const yaw = Math.atan2(-dx, -dz)
  let pitch = layLob(lob, along, mark[1] - pivot[1])
  for (let pass = 0; pass < 2; pass++) {
    const out = Math.cos(pitch) * lob.length
    const up = Math.sin(pitch) * lob.length
    pitch = layLob(lob, along - out, mark[1] - pivot[1] - up)
  }
  return { yaw, pitch }
}
