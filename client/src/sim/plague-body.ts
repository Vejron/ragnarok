import { FACE_EAR_LEFT, FACE_EAR_RIGHT, FACE_EYE_LEFT, FACE_EYE_RIGHT, FACE_MOUTH, PLAGUE_RECOVER, PLAGUE_STOMP, PLAGUE_STRIKE, PLAGUE_WALK } from '@/net/codec'
import { PLAGUE_FOOT_SPREAD } from './consts'

/**
 * The Orange Plague's body: his skeleton, how he holds it in each mode, and the capsules that cover
 * it. Mirrors `crates/shared/src/plague.rs` (checked by `fixtures/parity/plague.json`); the server
 * hits and collides with the same capsules, and his model is built on the same joints.
 *
 * His own frame: feet on the ground at the origin, +X to his right, +Y up, facing -Z.
 */

export type Vec3 = [number, number, number]

export const HIP_Y = 17.5
export const HIP_X = PLAGUE_FOOT_SPREAD
export const THIGH = 8.3
export const SHIN = 7.8
export const SHOULDER_X = 8.6
export const SHOULDER_Y = 29.5
export const UPPER_ARM = 6.8
export const FOREARM = 6.2
export const HAND_REACH = 7.6
export const HEAD: Vec3 = [0, 35.6, 0]
export const HEAD_RADIUS = 3.7
/** From the middle of his head; his right side (mirror X for the left). */
export const EYE: Vec3 = [1.35, 0.8, -3.15]
export const EYE_RADIUS = 0.75
export const EAR: Vec3 = [3.6, 0.15, 0.3]
export const EAR_HALF = 0.85
export const EAR_RADIUS = 0.85
export const MOUTH: Vec3 = [0, -1.5, -3.05]
export const MOUTH_HALF = 1.0
export const MOUTH_RADIUS = 0.7

const LEG_SWING = 0.32
const ARM_SWING = 0.22
const KNEE_LIFT = 0.6

/** Every joint angle, radians; index 0 is his left. Shoulders and elbows are `[x, z]`. */
export interface Pose {
  tilt: number
  bob: number
  hip: [number, number]
  knee: [number, number]
  shoulder: [[number, number], [number, number]]
  elbow: [[number, number], [number, number]]
}

/** What a capsule is part of: a `FACE_*` bit for his face, or one of these. */
export type Part = 'leg' | 'body' | 'arm' | 'hand' | 'head' | number

export interface Capsule {
  part: Part
  a: Vec3
  b: Vec3
  radius: number
  solid: boolean
}

const smooth = (x: number) => {
  const t = Math.min(1, Math.max(0, x))
  return t * t * (3 - 2 * t)
}

/** The progress `progress_byte` sent: a walk's place in its cycle, or how much of anything else is done. */
export function progressOf(mode: number, toppling: boolean, byte: number) {
  return mode === PLAGUE_WALK && !toppling ? byte / 256 : byte / 255
}

/** How he stands in `mode` (`PLAGUE_*`), `progress` (0..1) through it, or falling over. */
export function pose(mode: number, progress: number, toppling: boolean): Pose {
  const p: Pose = { tilt: 0, bob: 0, hip: [0, 0], knee: [0, 0], shoulder: [[0, -0.1], [0, 0.1]], elbow: [[0.25, 0], [0.25, 0]] }
  if (toppling) {
    const t = Math.min(1, Math.max(0, progress))
    p.tilt = t * t * (Math.PI / 2)
    p.hip = [0.35 * t, 0.2 * t]
    p.knee = [-0.3 * t, -0.5 * t]
    p.shoulder = [[2.6 * t, -0.5 * t], [2.4 * t, 0.6 * t]]
    return p
  }
  if (mode === PLAGUE_STOMP) {
    const lift = smooth(progress * 1.6)
    p.hip = [0, 1.4 * lift]
    p.knee = [-0.1 * lift, -1.4 * lift]
    p.tilt = 0.1 * lift
    p.shoulder = [[0.3 * lift, -0.1 - 0.6 * lift], [0.3 * lift, 0.1 + 0.6 * lift]]
  } else if (mode === PLAGUE_RECOVER) {
    const left = 1 - smooth(progress)
    p.hip = [0.15 * left, 0.15 * left]
    p.knee = [-0.3 * left, -0.3 * left]
    p.tilt = -0.06 * left
  } else if (mode === PLAGUE_STRIKE) {
    const call = smooth(progress * 3)
    const point = smooth(progress * 2)
    p.shoulder = [[1.5 * point, -0.1], [2.5 * call, 0.1 + 0.6 * call]]
    p.elbow = [[0.25 - 0.15 * point, 0], [0.25 + 2.45 * call, -0.8 * call]]
  } else {
    const angle = progress * Math.PI * 2
    const [sin, cos] = [Math.sin(angle), Math.cos(angle)]
    p.hip = [LEG_SWING * cos, -LEG_SWING * cos]
    p.knee = [-KNEE_LIFT * Math.max(0, -sin) - 0.05, -KNEE_LIFT * Math.max(0, sin) - 0.05]
    p.shoulder = [[-ARM_SWING * cos, -0.1], [ARM_SWING * cos, 0.1]]
    p.bob = Math.abs(Math.sin(2 * angle)) * 0.4
  }
  return p
}

type Mat = readonly [Vec3, Vec3, Vec3]

const rotX = (t: number): Mat => {
  const [s, c] = [Math.sin(t), Math.cos(t)]
  return [[1, 0, 0], [0, c, -s], [0, s, c]]
}
const rotZ = (t: number): Mat => {
  const [s, c] = [Math.sin(t), Math.cos(t)]
  return [[c, -s, 0], [s, c, 0], [0, 0, 1]]
}
const mul = (a: Mat, b: Mat): Mat =>
  [0, 1, 2].map((i) => [0, 1, 2].map((j) => a[i]![0] * b[0]![j]! + a[i]![1] * b[1]![j]! + a[i]![2] * b[2]![j]!)) as unknown as Mat
const apply = (m: Mat, v: Vec3): Vec3 => [
  m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
  m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
  m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
]

class Frame {
  constructor(readonly origin: Vec3, readonly rotation: Mat) {}
  point(local: Vec3): Vec3 {
    const r = apply(this.rotation, local)
    return [this.origin[0] + r[0], this.origin[1] + r[1], this.origin[2] + r[2]]
  }
  child(at: Vec3, rotation: Mat) {
    return new Frame(this.point(at), mul(this.rotation, rotation))
  }
}

const down = (length: number): Vec3 => [0, -length, 0]

/** Every capsule of his body in `p`, in his frame, in the server's order. */
export function capsules(p: Pose): Capsule[] {
  const body = new Frame([0, p.bob, 0], rotX(p.tilt))
  const out: Capsule[] = []
  const push = (part: Part, frame: Frame, a: Vec3, b: Vec3, radius: number, solid: boolean) => {
    out.push({ part, a: frame.point(a), b: frame.point(b), radius, solid })
  }
  ;[-1, 1].forEach((side, i) => {
    const thigh = body.child([side * HIP_X, HIP_Y, 0], rotX(p.hip[i]!))
    const shin = thigh.child(down(THIGH), rotX(p.knee[i]!))
    push('leg', thigh, [0, 0, 0], down(THIGH), 2.7, true)
    push('leg', shin, [0, 0, 0], down(SHIN - 1.2), 2.2, true)
    push('leg', shin, [0, -SHIN - 0.2, 0.9], [0, -SHIN - 0.2, -2.9], 1.2, true)
  })
  push('body', body, [-HIP_X, HIP_Y - 0.3, 0.3], [HIP_X, HIP_Y - 0.3, 0.3], 3.6, true)
  push('body', body, [0, 19.8, -1.4], [0, 25.2, -0.8], 6.9, true)
  push('body', body, [-6.6, 28.4, 0], [6.6, 28.4, 0], 3.1, true)
  push('head', body, [0, 30.4, 0], [0, 32.4, 0], 2.3, true)
  push('head', body, HEAD, HEAD, HEAD_RADIUS, true)
  push('head', body, [0, 37.2, -0.5], [0, 37.2, -0.5], 3.3, true)
  ;[-1, 1].forEach((side, i) => {
    const [sx, sz] = p.shoulder[i]!
    const [ex, ez] = p.elbow[i]!
    const upper = body.child([side * SHOULDER_X, SHOULDER_Y, 0], mul(rotX(sx), rotZ(sz)))
    const fore = upper.child(down(UPPER_ARM), mul(rotX(ex), rotZ(ez)))
    push('arm', upper, [0, 0, 0], down(UPPER_ARM), 2.0, true)
    push('arm', fore, [0, 0, 0], down(FOREARM), 1.7, true)
    push(side > 0 ? 'hand' : 'arm', fore, down(HAND_REACH), down(HAND_REACH), 1.9, true)
  })
  const head = new Frame(body.point(HEAD), body.rotation)
  const at = (offset: Vec3, side: number): Vec3 => [offset[0] * side, offset[1], offset[2]]
  for (const [bit, side] of [[FACE_EYE_LEFT, -1], [FACE_EYE_RIGHT, 1]] as const) push(bit, head, at(EYE, side), at(EYE, side), EYE_RADIUS, false)
  for (const [bit, side] of [[FACE_EAR_LEFT, -1], [FACE_EAR_RIGHT, 1]] as const) {
    const [x, y, z] = at(EAR, side)
    push(bit, head, [x, y - EAR_HALF, z], [x, y + EAR_HALF, z], EAR_RADIUS, false)
  }
  const [x, y, z] = MOUTH
  push(FACE_MOUTH, head, [x - MOUTH_HALF, y, z], [x + MOUTH_HALF, y, z], MOUTH_RADIUS, false)
  return out
}

/** The bottom of each shoe, in his frame, his left first. */
export function feet(p: Pose): [Vec3, Vec3] {
  const body = new Frame([0, p.bob, 0], rotX(p.tilt))
  const foot = (i: number) => {
    const side = i === 0 ? -1 : 1
    const thigh = body.child([side * HIP_X, HIP_Y, 0], rotX(p.hip[i]!))
    const shin = thigh.child(down(THIGH), rotX(p.knee[i]!))
    return shin.point([0, -SHIN - 1.4, -1])
  }
  return [foot(0), foot(1)]
}

/** A point of his frame in the world, for him at `position` facing `yaw`. */
export function toWorld(position: readonly number[], yaw: number, local: Vec3): Vec3 {
  const [sin, cos] = [Math.sin(yaw), Math.cos(yaw)]
  return [position[0]! + cos * local[0] + sin * local[2], position[1]! + local[1], position[2]! - sin * local[0] + cos * local[2]]
}

/** `capsules` placed in the world. */
export function worldCapsules(p: Pose, position: readonly number[], yaw: number): Capsule[] {
  return capsules(p).map((c) => ({ ...c, a: toWorld(position, yaw, c.a), b: toWorld(position, yaw, c.b) }))
}

/** His solid capsules for an entity in a snapshot, or null while he is falling and in nobody's way. */
export function solidsOf(state: { x: number; y: number; z: number; yaw: number; held: number; dead: boolean; stance: number }): Capsule[] | null {
  if (state.dead) return null
  const p = pose(state.held, progressOf(state.held, false, state.stance), false)
  return worldCapsules(p, [state.x, state.y, state.z], state.yaw).filter((c) => c.solid)
}
