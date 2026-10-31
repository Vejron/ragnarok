/**
 * The geometry of aiming over the shoulder, as the server's `crosshair_dir` has it: the camera turns
 * about a pivot beside the body and looks along the player's look, while a round leaves the eye for
 * whatever the pivot's ray meets.
 */

import { ON_FOOT_HEIGHT, ON_FOOT_SHOULDER } from '@/sim/consts'

type Vec3 = [number, number, number]
type CastRay = (origin: Vec3, direction: Vec3, max: number) => { t: number } | null

/** How finely `clearArm` walks out along the arm, and how many halvings then settle its end (m). */
const ARM_STEP = 0.4
const ARM_HALVINGS = 4
/** A round stopped closer to the crosshair's point than this (m) counts as having got there. */
const BLOCKED_SLACK = 1

/** Which way a look of `yaw` and `pitch` points. */
export const lookDir = (yaw: number, pitch: number): Vec3 => {
  const flat = Math.cos(pitch)
  return [-Math.sin(yaw) * flat, Math.sin(pitch), -Math.cos(yaw) * flat]
}

/** The point the on-foot camera turns about, for a soldier at `position`. Mirrors `Player::pivot`. */
export const shoulderPivot = (position: ArrayLike<number>, yaw: number): Vec3 =>
  [position[0]! + Math.cos(yaw) * ON_FOOT_SHOULDER, position[1]! + ON_FOOT_HEIGHT, position[2]! - Math.sin(yaw) * ON_FOOT_SHOULDER]

/**
 * How far along `back` from `pivot` a camera can sit, at most `distance`, and stay `clearance` above the
 * ground. Shortening the arm keeps the camera on the crosshair's ray, where lifting it would turn the
 * view off it. 0 if the pivot itself is too low.
 */
export const clearArm = (pivot: readonly number[], back: readonly number[], distance: number, groundAt: (x: number, z: number) => number, clearance: number): number => {
  const clear = (arm: number) => pivot[1]! + back[1]! * arm >= groundAt(pivot[0]! + back[0]! * arm, pivot[2]! + back[2]! * arm) + clearance
  if (!clear(0)) return 0
  let free = 0
  let stuck = -1
  for (let arm = ARM_STEP; stuck < 0; arm += ARM_STEP) {
    const at = Math.min(arm, distance)
    if (!clear(at)) stuck = at
    else if (at === distance) return distance
    else free = at
  }
  for (let i = 0; i < ARM_HALVINGS; i++) {
    const middle = (free + stuck) / 2
    if (clear(middle)) free = middle
    else stuck = middle
  }
  return free
}

/** How far from the screen's middle (CSS px) a cone of `spread` radians reaches, through a lens of vertical `fov` degrees on a picture `height` tall. */
export const spreadPixels = (spread: number, fov: number, height: number): number =>
  (Math.tan(spread) / Math.tan((fov * Math.PI) / 360)) * (height / 2)

/**
 * Where something stops a round short of what the crosshair is on, for a soldier at `position` with a
 * gun of `range`; null if it gets there. A round leaves on the crosshair's own line unless a wall stands
 * between `eye` and that line (the server's `crosshair_muzzle`), and only then flies from the eye.
 * Sees the world only, not soldiers.
 */
export const blockedAt = (castRay: CastRay, position: ArrayLike<number>, eye: Vec3, yaw: number, pitch: number, range: number): Vec3 | null => {
  const look = lookDir(yaw, pitch)
  const pivot = shoulderPivot(position, yaw)
  const abreast = (eye[0] - pivot[0]) * look[0] + (eye[1] - pivot[1]) * look[1] + (eye[2] - pivot[2]) * look[2]
  const side: Vec3 = [pivot[0] + look[0] * abreast - eye[0], pivot[1] + look[1] * abreast - eye[1], pivot[2] + look[2] * abreast - eye[2]]
  const wide = Math.hypot(side[0], side[1], side[2])
  if (wide < 1e-3 || !castRay(eye, [side[0] / wide, side[1] / wide, side[2] / wide], wide)) return null
  const reach = castRay(pivot, look, range)?.t ?? range
  const to: Vec3 = [pivot[0] + look[0] * reach - eye[0], pivot[1] + look[1] * reach - eye[1], pivot[2] + look[2] * reach - eye[2]]
  const length = Math.hypot(to[0], to[1], to[2])
  if (length < 1e-3) return null
  const dir: Vec3 = [to[0] / length, to[1] / length, to[2] / length]
  // The server shoots along the look instead, when the point would send the round sideways.
  if (dir[0] * look[0] + dir[1] * look[1] + dir[2] * look[2] < 0.5) return null
  const hit = castRay(eye, dir, length)
  if (!hit || hit.t > length - BLOCKED_SLACK) return null
  return [eye[0] + dir[0] * hit.t, eye[1] + dir[1] * hit.t, eye[2] + dir[2] * hit.t]
}
