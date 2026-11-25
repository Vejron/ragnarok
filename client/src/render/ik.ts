/**
 * What makes a character move like a body rather than play its clips (ANIMATION.md in docs/): run
 * after the clips are sampled, on the bones of a character near the camera.
 *
 * - `Feet` stands the feet on the ground under them, lowering the hips onto the lower planted foot,
 *   tips each planted foot to the slope, and holds a planted foot still until it lifts.
 * - `Grip` keeps the left hand on what the right one holds, whatever bends the arms apart.
 * - `Inertia` takes out a jump in the pose (a clip snapped in) by easing out of the pose last drawn.
 * - `Follow` and `Sway` are the springs: the body following the look a moment late, and the head
 *   nodding as the hips start, stop and bob.
 *
 * Everything works on world rotations taken from the bones' world matrices, so it holds whatever the
 * bones' own axes are. Nothing here allocates once made.
 */
import { Quaternion, Vector3, type Node } from '@ragnarok/engine'

const EPS = 1e-6
const UP = new Vector3(0, 1, 0)
const S = {
  a: new Quaternion(),
  b: new Quaternion(),
  c: new Quaternion(),
  d: new Quaternion(),
  e: new Quaternion(),
  u: new Vector3(),
  v: new Vector3(),
  w: new Vector3(),
  x: new Vector3(),
  y: new Vector3(),
  z: new Vector3(),
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v))
const smoothstep = (a: number, b: number, v: number) => {
  const t = clamp((v - a) / (b - a), 0, 1)
  return t * t * (3 - 2 * t)
}
/** Moves `value` towards `target`, `rate` of the way per second, frame-rate independent. */
const chase = (value: number, target: number, rate: number, dt: number) => value + (target - value) * (1 - Math.exp(-rate * dt))

/** The rotation of column-major matrix `m`, its scale taken out. */
export function rotationOf(m: ArrayLike<number>, out: Quaternion): Quaternion {
  const sx = Math.hypot(m[0]!, m[1]!, m[2]!) || 1
  const sy = Math.hypot(m[4]!, m[5]!, m[6]!) || 1
  const sz = Math.hypot(m[8]!, m[9]!, m[10]!) || 1
  const m00 = m[0]! / sx, m10 = m[1]! / sx, m20 = m[2]! / sx
  const m01 = m[4]! / sy, m11 = m[5]! / sy, m21 = m[6]! / sy
  const m02 = m[8]! / sz, m12 = m[9]! / sz, m22 = m[10]! / sz
  const trace = m00 + m11 + m22
  if (trace > 0) {
    const s = 0.5 / Math.sqrt(trace + 1)
    return out.set((m21 - m12) * s, (m02 - m20) * s, (m10 - m01) * s, 0.25 / s)
  }
  if (m00 > m11 && m00 > m22) {
    const s = 2 * Math.sqrt(1 + m00 - m11 - m22)
    return out.set(0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s)
  }
  if (m11 > m22) {
    const s = 2 * Math.sqrt(1 + m11 - m00 - m22)
    return out.set((m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s)
  }
  const s = 2 * Math.sqrt(1 + m22 - m00 - m11)
  return out.set((m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s)
}

/** `node`'s rotation in the world, its world matrix brought up to date first. */
export function worldRotation(node: Node, out: Quaternion): Quaternion {
  node.updateWorldMatrix(true, false)
  return rotationOf(node.matrixWorld, out)
}

/** Turns `bone` by `delta`, a rotation in world axes about its own joint. */
export function turnWorld(bone: Node, delta: Quaternion) {
  const parent = bone.parent ? worldRotation(bone.parent, S.a) : S.a.identity()
  // In the parent's axes the turn is P⁻¹ D P, and it goes on before the bone's own rotation.
  const local = S.b.copy(parent).invert().multiply(delta).multiply(parent)
  bone.quaternion.premultiply(local)
}

/** Gives `bone` the world rotation `rotation`, whatever its parent's. */
export function setWorldRotation(bone: Node, rotation: Quaternion) {
  const parent = bone.parent ? worldRotation(bone.parent, S.a) : S.a.identity()
  const local = S.b.copy(parent).invert().multiply(rotation)
  bone.quaternion.set(local.x, local.y, local.z, local.w)
}

/**
 * Solves a two-bone chain (thigh and shin, upper arm and forearm) so its end reaches `target`, as far
 * as the bones reach: the turns in world axes that the root bone takes (`root`, about the root joint)
 * and the middle one takes before it (`mid`, about the middle joint). The chain bends the way it bends
 * already, or towards `bend` when it is straight. All points are in the world.
 */
export function twoBone(a: Vector3, b: Vector3, c: Vector3, target: Vector3, bend: Vector3, root: Quaternion, mid: Quaternion) {
  const lab = a.distanceTo(b)
  const lcb = b.distanceTo(c)
  const lat = clamp(a.distanceTo(target), EPS, lab + lcb - 1e-4)
  const ac = S.u.subVectors(c, a).normalize()
  const ab = S.v.subVectors(b, a).normalize()
  const ba = S.w.copy(ab).negate()
  const bc = S.x.subVectors(c, b).normalize()
  const at = S.y.subVectors(target, a).normalize()
  const acAb0 = Math.acos(clamp(ac.dot(ab), -1, 1))
  const baBc0 = Math.acos(clamp(ba.dot(bc), -1, 1))
  const acAt0 = Math.acos(clamp(ac.dot(at), -1, 1))
  const acAb1 = Math.acos(clamp((lcb * lcb - lab * lab - lat * lat) / (-2 * lab * lat), -1, 1))
  const baBc1 = Math.acos(clamp((lat * lat - lab * lab - lcb * lcb) / (-2 * lab * lcb), -1, 1))
  // The plane the chain bends in, and the one that swings it onto the target.
  const axis0 = S.z.crossVectors(ac, ab)
  if (axis0.lengthSq() < 1e-8) axis0.crossVectors(ac, bend)
  axis0.normalize()
  mid.setFromAxisAngle(axis0, baBc1 - baBc0)
  root.setFromAxisAngle(axis0, acAb1 - acAb0)
  const axis1 = S.w.crossVectors(ac, at)
  if (axis1.lengthSq() > 1e-10) root.premultiply(S.e.setFromAxisAngle(axis1.normalize(), acAt0))
}

/** A critically damped spring that follows a target, `halfLife` seconds to close half the gap. */
export class Follow {
  x = 0
  v = 0
  constructor(private readonly halfLife: number) {}

  step(target: number, dt: number): number {
    const y = (2 * Math.LN2) / this.halfLife
    const j0 = this.x - target
    const j1 = this.v + j0 * y
    const e = Math.exp(-y * dt)
    this.x = e * (j0 + j1 * dt) + target
    this.v = e * (this.v - j1 * y * dt)
    return this.x
  }

  /** Jumps to `x`, still. */
  reset(x = 0) {
    this.x = x
    this.v = 0
  }
}

/** How far a foot's ground may be from where the clip puts the feet before it is not the ground they stand on, metres. */
const ON_GROUND = 0.35
/** The furthest a foot is raised or the hips lowered to meet the ground, metres. */
const REACH = 0.45
/** How high the ankle is over the sole standing flat, metres (`ANKLE_Z` in tools/character/characters.py). */
const ANKLE = 0.075
/** A foot this far over standing flat is lifted, and from the lower one planted, metres. */
const LIFTED = 0.1
const PLANTED = 0.025
/** The steepest a planted foot tips to the slope, radians. */
const TIP_MAX = 0.5
/** How far either side of a foot the slope is sampled, metres. */
const SLOPE_STEP = 0.15
/** A planted foot held this far from where the clip has it lets go, metres. */
const LOCK_SLIP = 0.22
/** How fast a foot takes hold and lets go of where it was planted, and the hips follow the ground, per second. */
const LOCK_RATE = 30
const UNLOCK_RATE = 9
const DROP_RATE = 14
const ON_RATE = 6

/** One leg's bones, and what its foot holds onto. */
interface Leg {
  thigh: Node
  shin: Node
  foot: Node
  /** Where the foot was planted in the world, and how much it holds there (0 to 1). */
  lock: Vector3
  locked: boolean
  hold: number
  /** How much higher the ground is under the foot than where the clip stands it, metres. */
  rise: number
}

/**
 * Stands the feet on the ground under them. The clips stand the body on the flat; on a slope the hips
 * drop onto the lower foot, each leg is solved onto its ground, and a planted foot tips to the slope and
 * holds where it came down until it lifts. Only the terrain is known here, so the ground is trusted only
 * where it is about where the clip stands the body: on a floor or a deck the layer fades out.
 */
export class Feet {
  private readonly legs: Leg[]
  /** How far the hips are lowered, metres, and how much of the layer is on (0 to 1). */
  private drop = 0
  private on = 0
  private readonly ankle = new Vector3()
  private readonly knee = new Vector3()
  private readonly hip = new Vector3()
  private readonly target = new Vector3()
  private readonly normal = new Vector3()
  private readonly forward = new Vector3()
  private readonly footWorld = new Quaternion()
  private readonly rootTurn = new Quaternion()
  private readonly midTurn = new Quaternion()

  constructor(bones: (name: string) => Node | undefined) {
    this.legs = []
    for (const side of ['L', 'R']) {
      const thigh = bones(`thigh_${side}`)
      const shin = bones(`shin_${side}`)
      const foot = bones(`foot_${side}`)
      if (thigh && shin && foot) this.legs.push({ thigh, shin, foot, lock: new Vector3(), locked: false, hold: 0, rise: 0 })
    }
  }

  /** How much the layer is on, 0 to 1: for tests and debugging. */
  get weight() {
    return this.on
  }

  /** How far it lowers the hips, metres: for tests and debugging. */
  get lowered() {
    return -this.drop * this.on
  }

  /** Lets go of every foot and the ground: it is somewhere else now, or not standing. */
  reset() {
    this.drop = 0
    this.on = 0
    for (const leg of this.legs) {
      leg.locked = false
      leg.hold = 0
    }
  }

  /**
   * Fits the feet of `model` (posed and placed, its origin where the clip's feet stand) to `ground`.
   * `wanted` is how much it should (0 off: in the air, lying, swimming, on skis), and `forward` the way
   * the body faces in the world, for which way a straight knee bends.
   */
  fit(model: Node, ground: (x: number, z: number) => number, wanted: number, forward: Vector3, dt: number) {
    if (this.legs.length < 2) return
    model.updateWorldMatrix(true, true)
    const { x, y: floor, z } = model.position
    // Off the terrain (a floor, a deck, a roof) nothing here knows where the ground is.
    const onTerrain = Math.abs(floor - ground(x, z)) < ON_GROUND ? 1 : 0
    this.on = chase(this.on, wanted * onTerrain, ON_RATE, dt)
    if (this.on < 0.01) {
      this.reset()
      return
    }
    // How much higher the ground is under each foot than where the clip stands it. The hips come down
    // for a foot that is down, not for one swinging out over lower ground.
    let low = 0
    for (const leg of this.legs) {
      leg.foot.getWorldPosition(this.ankle)
      leg.rise = clamp(ground(this.ankle.x, this.ankle.z) - floor, -REACH, REACH)
      const planted = 1 - smoothstep(PLANTED, LIFTED, this.ankle.y - floor - ANKLE)
      low = Math.min(low, leg.rise * planted)
    }
    this.drop = chase(this.drop, low, DROP_RATE, dt)
    const w = this.on
    // The hips come down onto the lower foot, and every foot with them; each is then raised to its ground.
    model.position.y += this.drop * w
    model.updateWorldMatrix(false, true)
    this.forward.copy(forward)
    for (const leg of this.legs) this.fitLeg(leg, leg.rise - this.drop, floor, ground, w, dt)
  }

  private fitLeg(leg: Leg, raise: number, floor: number, ground: (x: number, z: number) => number, w: number, dt: number) {
    leg.thigh.getWorldPosition(this.hip)
    leg.shin.getWorldPosition(this.knee)
    leg.foot.getWorldPosition(this.ankle)
    worldRotation(leg.foot, this.footWorld)
    // How far the clip has the foot off the flat it stands on: planted, or on its way up.
    const lift = this.ankle.y - this.drop * w - floor - ANKLE
    const planted = 1 - smoothstep(PLANTED, LIFTED, lift)
    const target = this.target.copy(this.ankle).addScaledVector(UP, raise * w)

    // A planted foot holds where it came down until it lifts, or until the body has gone on without it.
    if (planted > 0.7 && !leg.locked) {
      leg.locked = true
      leg.lock.copy(target)
    } else if (leg.locked && (planted < 0.4 || Math.hypot(leg.lock.x - target.x, leg.lock.z - target.z) > LOCK_SLIP)) {
      leg.locked = false
    }
    leg.hold = chase(leg.hold, leg.locked ? 1 : 0, leg.locked ? LOCK_RATE : UNLOCK_RATE, dt)
    // Never further than a slip from where the clip has it: a body moved a long way at once (a
    // correction, a respawn) lets go at once rather than reaching back.
    const dx = leg.lock.x - target.x
    const dz = leg.lock.z - target.z
    const hold = leg.hold * w * Math.min(1, LOCK_SLIP / Math.max(Math.hypot(dx, dz), EPS))
    target.x += dx * hold
    target.z += dz * hold

    twoBone(this.hip, this.knee, this.ankle, target, this.forward, this.rootTurn, this.midTurn)
    // The shin takes its own turn first, then the thigh carries both; the foot keeps its world turn.
    turnWorld(leg.shin, this.midTurn)
    turnWorld(leg.thigh, this.rootTurn)

    // Flat on the slope while planted.
    const x = target.x
    const z = target.z
    const n = this.normal.set(ground(x - SLOPE_STEP, z) - ground(x + SLOPE_STEP, z), 2 * SLOPE_STEP, ground(x, z - SLOPE_STEP) - ground(x, z + SLOPE_STEP)).normalize()
    const tip = S.c.setFromUnitVectors(UP, n)
    const angle = 2 * Math.acos(clamp(Math.abs(tip.w), 0, 1))
    const keep = angle > TIP_MAX ? TIP_MAX / angle : 1
    tip.slerp(S.d.identity(), 1 - keep * planted * w)
    setWorldRotation(leg.foot, S.e.copy(tip).multiply(this.footWorld))
  }
}

/**
 * Keeps the left hand on what the right one holds: the hand is solved onto a bone on the right hand
 * (`grip`, `grip_pistol`, ... from the build), which the stance's clips lay it on.
 */
export class Grip {
  private readonly upper: Node | null
  private readonly fore: Node | null
  private readonly hand: Node | null
  private readonly shoulder = new Vector3()
  private readonly elbow = new Vector3()
  private readonly wrist = new Vector3()
  private readonly target = new Vector3()
  private readonly bend = new Vector3()
  private readonly want = new Quaternion()
  private readonly handWorld = new Quaternion()
  private readonly rootTurn = new Quaternion()
  private readonly midTurn = new Quaternion()

  constructor(bones: (name: string) => Node | undefined) {
    this.upper = bones('upper_arm_L') ?? null
    this.fore = bones('forearm_L') ?? null
    this.hand = bones('hand_L') ?? null
  }

  /** Lays the left hand on `grip` (a bone), `weight` of the way from where the clips have it. */
  hold(grip: Node, weight: number) {
    const { upper, fore, hand } = this
    if (!upper || !fore || !hand || weight < 0.01) return
    upper.getWorldPosition(this.shoulder)
    fore.getWorldPosition(this.elbow)
    hand.getWorldPosition(this.wrist)
    worldRotation(hand, this.handWorld)
    grip.getWorldPosition(this.target)
    worldRotation(grip, this.want)
    this.target.lerp(this.wrist, 1 - weight)
    // A straight arm bends its elbow down.
    this.bend.set(0, -1, 0)
    twoBone(this.shoulder, this.elbow, this.wrist, this.target, this.bend, this.rootTurn, this.midTurn)
    turnWorld(fore, this.midTurn)
    turnWorld(upper, this.rootTurn)
    if (this.handWorld.dot(this.want) < 0) this.want.set(-this.want.x, -this.want.y, -this.want.z, -this.want.w)
    setWorldRotation(hand, this.handWorld.slerp(this.want, weight))
  }
}

/** Copies the places of `nodes` into `out`, three numbers each. */
function copyPlaces(nodes: Node[], out: Float32Array) {
  for (let i = 0; i < nodes.length; i++) {
    const p = nodes[i]!.position
    out[i * 3] = p.x
    out[i * 3 + 1] = p.y
    out[i * 3 + 2] = p.z
  }
}

/** A pose that turns a bone this far in a frame, radians, has jumped rather than moved (at least, or this many radians a second). */
const SNAP_ANGLE = 0.5
const SNAP_SPEED = 14
/** And one that moves the hips this far in a frame, metres. */
const SNAP_MOVE = 0.12
/** How quickly a jump eases out, per second: the critically damped ease `(1 + kt) e^(-kt)`, half gone in about 0.1 s. */
const EASE_RATE = 17

/**
 * Takes the pops out of the clips: where the sampled pose jumps from one frame to the next (a clip
 * snapped in, getting up, climbing into a seat), the difference from the pose last drawn is kept and
 * eased away, so the body moves through it instead of teleporting. It runs on the clips' pose, before
 * anything is added to it.
 */
export class Inertia {
  private readonly bones: Node[]
  /** The rotations, and the places of the bones the clips move, that the clips last gave and that were last drawn; and the offsets being eased out. */
  private readonly sampled: Float32Array
  private readonly last: Float32Array
  private readonly offset: Float32Array
  private readonly sampledAt: Float32Array
  private readonly lastAt: Float32Array
  private readonly offsetAt: Float32Array
  private readonly moved: Node[]
  private has = false
  private age = Infinity
  private readonly q = new Quaternion()
  private readonly o = new Quaternion()

  /** `bones` are the bones it smooths; `moved` the ones whose places the clips move too (the root and the hips). */
  constructor(bones: Node[], moved: Node[] = []) {
    this.bones = bones
    this.moved = moved
    this.sampledAt = new Float32Array(moved.length * 3)
    this.lastAt = new Float32Array(moved.length * 3)
    this.offsetAt = new Float32Array(moved.length * 3)
    this.sampled = new Float32Array(bones.length * 4)
    this.last = new Float32Array(bones.length * 4)
    this.offset = new Float32Array(bones.length * 4)
  }

  /** Whether it is easing out of a jump now: for tests and debugging. */
  get easing() {
    return this.age < 4 / EASE_RATE
  }

  /** Forgets the last pose: the next one is not a jump from it (a ragdoll had the bones, or it was not drawn). */
  forget() {
    this.has = false
    this.age = Infinity
  }

  /** Smooths the clips' pose just sampled into the bones, `dt` seconds after the last. */
  apply(dt: number) {
    const { bones, sampled, last, offset } = this
    if (this.has) {
      // A jump is told from the clips' own pose frame to frame; what is eased out is the drawn pose's.
      let worst = 0
      for (let i = 0; i < bones.length; i++) {
        const q = bones[i]!.quaternion
        const dot = Math.abs(q.x * sampled[i * 4]! + q.y * sampled[i * 4 + 1]! + q.z * sampled[i * 4 + 2]! + q.w * sampled[i * 4 + 3]!)
        worst = Math.max(worst, 2 * Math.acos(Math.min(1, dot)))
      }
      let moved = 0
      for (let i = 0; i < this.moved.length; i++) {
        const p = this.moved[i]!.position
        moved = Math.max(moved, Math.hypot(p.x - this.sampledAt[i * 3]!, p.y - this.sampledAt[i * 3 + 1]!, p.z - this.sampledAt[i * 3 + 2]!))
      }
      if (worst > Math.max(SNAP_ANGLE, SNAP_SPEED * dt) || moved > Math.max(SNAP_MOVE, 2 * dt)) {
        // A jump: keep what it jumped by, the drawn pose against the new one, to ease out.
        for (let i = 0; i < bones.length; i++) {
          const q = bones[i]!.quaternion
          this.q.set(q.x, q.y, q.z, q.w).invert()
          this.o.set(last[i * 4]!, last[i * 4 + 1]!, last[i * 4 + 2]!, last[i * 4 + 3]!).multiply(this.q)
          if (this.o.w < 0) this.o.set(-this.o.x, -this.o.y, -this.o.z, -this.o.w)
          offset[i * 4] = this.o.x
          offset[i * 4 + 1] = this.o.y
          offset[i * 4 + 2] = this.o.z
          offset[i * 4 + 3] = this.o.w
        }
        for (let i = 0; i < this.moved.length; i++) {
          const p = this.moved[i]!.position
          this.offsetAt[i * 3] = this.lastAt[i * 3]! - p.x
          this.offsetAt[i * 3 + 1] = this.lastAt[i * 3 + 1]! - p.y
          this.offsetAt[i * 3 + 2] = this.lastAt[i * 3 + 2]! - p.z
        }
        this.age = 0
      } else this.age += dt
    }
    for (let i = 0; i < bones.length; i++) {
      const q = bones[i]!.quaternion
      sampled[i * 4] = q.x
      sampled[i * 4 + 1] = q.y
      sampled[i * 4 + 2] = q.z
      sampled[i * 4 + 3] = q.w
    }
    copyPlaces(this.moved, this.sampledAt)
    if (this.age < 4 / EASE_RATE) {
      const k = EASE_RATE * this.age
      const left = (1 + k) * Math.exp(-k)
      for (let i = 0; i < bones.length; i++) {
        this.o.set(offset[i * 4]!, offset[i * 4 + 1]!, offset[i * 4 + 2]!, offset[i * 4 + 3]!)
        this.q.identity().slerp(this.o, left)
        bones[i]!.quaternion.premultiply(this.q)
      }
      for (let i = 0; i < this.moved.length; i++) {
        const p = this.moved[i]!.position
        p.set(p.x + this.offsetAt[i * 3]! * left, p.y + this.offsetAt[i * 3 + 1]! * left, p.z + this.offsetAt[i * 3 + 2]! * left)
      }
    }
    for (let i = 0; i < bones.length; i++) {
      const q = bones[i]!.quaternion
      last[i * 4] = q.x
      last[i * 4 + 1] = q.y
      last[i * 4 + 2] = q.z
      last[i * 4 + 3] = q.w
    }
    copyPlaces(this.moved, this.lastAt)
    this.has = true
  }
}

/** The head's springs: how stiff and how damped (a little under critical, so it settles with a small overshoot). */
const SWAY_K = 110
const SWAY_C = 13
/** How far the head nods per m/s² the hips speed up or slow down, and rolls per m/s² sideways, radians. */
const NOD_PER = 0.012
const ROLL_PER = 0.01
/** And dips per m/s² they bob, radians. */
const BOB_PER = 0.004
/** Accelerations past this are a teleport or a hit, not a stride, m/s². */
const ACCEL_MAX = 25
const SWAY_MAX = 0.25

/**
 * The head and neck carried by the hips like a weight on a stalk: starting off they tip back, stopping
 * they nod on, each footfall's bob dips them a little, and they settle with a small overshoot.
 */
export class Sway {
  /** The head's nod (forward) and roll (to its right), radians, and how fast each goes. */
  nod = 0
  roll = 0
  private nodV = 0
  private rollV = 0
  private readonly at = new Vector3()
  private readonly velocity = new Vector3()
  private readonly accel = new Vector3()
  private has = false

  reset() {
    this.nod = this.roll = this.nodV = this.rollV = 0
    this.has = false
  }

  /**
   * Steps the springs from the hips' world place `hips` this frame. `yaw` is where the body faces (0
   * faces -Z, as the game's yaw), and `on` how much of it shows.
   */
  step(hips: Vector3, yaw: number, on: number, dt: number) {
    if (dt <= 0) return
    if (!this.has) {
      this.at.copy(hips)
      this.velocity.set(0, 0, 0)
      this.has = true
      return
    }
    const vx = (hips.x - this.at.x) / dt
    const vy = (hips.y - this.at.y) / dt
    const vz = (hips.z - this.at.z) / dt
    this.accel.set((vx - this.velocity.x) / dt, (vy - this.velocity.y) / dt, (vz - this.velocity.z) / dt)
    this.velocity.set(vx, vy, vz)
    this.at.copy(hips)
    if (this.accel.length() > ACCEL_MAX) this.accel.setLength(ACCEL_MAX)
    // Into the body's frame: ahead is -Z turned by `yaw`, its right +X turned likewise.
    const sin = Math.sin(yaw)
    const cos = Math.cos(yaw)
    const ahead = -(this.accel.x * sin + this.accel.z * cos)
    const right = this.accel.x * cos - this.accel.z * sin
    // Speeding up leaves the head behind (a nod back); the bob's rise presses it down.
    const pushNod = (-ahead * NOD_PER + this.accel.y * BOB_PER) * on
    const pushRoll = -right * ROLL_PER * on
    const n = Math.max(1, Math.ceil(dt * 120))
    const h = dt / n
    for (let i = 0; i < n; i++) {
      this.nodV += (-SWAY_K * (this.nod - pushNod) - SWAY_C * this.nodV) * h
      this.nod += this.nodV * h
      this.rollV += (-SWAY_K * (this.roll - pushRoll) - SWAY_C * this.rollV) * h
      this.roll += this.rollV * h
    }
    this.nod = clamp(this.nod, -SWAY_MAX, SWAY_MAX)
    this.roll = clamp(this.roll, -SWAY_MAX, SWAY_MAX)
  }
}
