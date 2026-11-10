/**
 * Laying a field mortar on a mark: the elevation of the high lob that lands a bomb on a point, worked out
 * for the height of that point, as the server's `lob_pitch` does for level ground. A gunner laying from
 * above sends this elevation in place of the look's pitch while aim is held (`gunnery::turn`).
 */

import { GRAVITY, MORTAR_BARREL_LENGTH, MORTAR_RANGE_MAX, MORTAR_RANGE_MIN, MORTAR_SHELL_SPEED } from './consts'
import type { Vec3 } from './motor'

const SPEED_SQUARED = MORTAR_SHELL_SPEED * MORTAR_SHELL_SPEED

/** The elevation of the high lob that lands `range` metres out on level ground, radians: the server's `lob_pitch`, unclamped. */
export function lobPitch(range: number): number {
  return 0.5 * (Math.PI - Math.asin(Math.min(1, (GRAVITY * range) / SPEED_SQUARED)))
}

/** How far a bomb lobbed at elevation `pitch` (radians) lands on level ground, metres: the server's `lob_range`. */
export function lobRange(pitch: number): number {
  return (SPEED_SQUARED * Math.sin(2 * pitch)) / GRAVITY
}

/** The flattest and steepest elevations the server lays a tube to: the ends of the range dial. */
export const LOB_PITCH_LOW = lobPitch(MORTAR_RANGE_MAX)
export const LOB_PITCH_HIGH = lobPitch(MORTAR_RANGE_MIN)

/**
 * The elevation of the high lob that carries a bomb `horizontal` metres out and `rise` metres up (down
 * when negative) from where it leaves the muzzle, radians, kept within what the server lays. A mark out of
 * reach gets the longest throw, 45 degrees.
 */
export function layLob(horizontal: number, rise: number): number {
  const x = Math.max(horizontal, 1e-3)
  const under = SPEED_SQUARED * SPEED_SQUARED - GRAVITY * (GRAVITY * x * x + 2 * rise * SPEED_SQUARED)
  const pitch = under < 0 ? Math.PI / 4 : Math.atan((SPEED_SQUARED + Math.sqrt(under)) / (GRAVITY * x))
  return Math.max(LOB_PITCH_LOW, Math.min(LOB_PITCH_HIGH, pitch))
}

/**
 * Where to lay a tube whose mount is at `pivot` so the bomb lands on `mark`: the bearing (a look's yaw,
 * 0 facing −Z) and the elevation. The muzzle is a barrel's length out along the tube, so it moves as the
 * tube rises; solving twice puts it close enough.
 */
export function layTube(pivot: Vec3, mark: Vec3): { yaw: number; pitch: number } {
  const dx = mark[0] - pivot[0]
  const dz = mark[2] - pivot[2]
  const along = Math.hypot(dx, dz)
  const yaw = Math.atan2(-dx, -dz)
  let pitch = layLob(along, mark[1] - pivot[1])
  for (let pass = 0; pass < 2; pass++) {
    const out = Math.cos(pitch) * MORTAR_BARREL_LENGTH
    const up = Math.sin(pitch) * MORTAR_BARREL_LENGTH
    pitch = layLob(along - out, mark[1] - pivot[1] - up)
  }
  return { yaw, pitch }
}
