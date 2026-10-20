import type { Node } from '@ragnarok/engine'
import * as THREE from 'three'
import { WALKER_FOOT_HALF } from '@/sim/consts'

/** The sizes of a pair of bird legs, metres. */
export interface LegSizes {
  /** Thigh and shin, hip to knee and knee to ankle. */
  thigh: number
  shin: number
  /** The foot's thickness: the ankle hangs this far above the sole. */
  pad: number
  /** How far the hips sit above the ground when it stands. */
  stand: number
  /** The hips' depth below the hull's centre. */
  hipDrop: number
}

/** The furthest a foot lands ahead of its hip, metres: at a run it plants this far ahead and slides this far back. */
const STEP = 1.7
/** A foot lifts for a new step when it has slid this far from where a fresh step would land, at the least. */
const SETTLE = 0.3
/** How high a foot is lifted at the middle of its swing, metres. */
const LIFT = 0.6
/** How far tucked feet are drawn up under the hull, metres. */
const TUCK = 1.6

/**
 * Two bird-like legs that step with the ground they cover: each foot stays planted at a world spot until
 * the hip has moved on far enough, then swings to a new one, and two-bone IK bends the leg to reach it,
 * the knee back like a bird's. The legs are drawn only: the physics holds the hull up with springs.
 * `tucked` draws both feet up under the hull, for a leap; they land under the hips.
 */
export class Legs {
  private stamp = 0
  private readonly last = new THREE.Vector3()
  private yaw = 0
  private seen = false
  private tuck = 0
  private wasTucked = false
  /** Each foot: planted at a fixed world spot, or swinging to a new one. */
  private readonly feet = [0, 1].map(() => ({ planted: true, x: 0, z: 0, from: 0, to: 0, time: 0, duration: 0.3 }))
  /** Feet that came down since the last `takeSteps`: where, and how hard (0..1, by speed). */
  private steps: { at: [number, number, number]; weight: number }[] = []

  /** `hips`, `knees` and `ankles` are left then right, each the child of the one before. */
  constructor(
    private readonly hips: readonly Node[],
    private readonly knees: readonly Node[],
    private readonly ankles: readonly Node[],
    private readonly sizes: LegSizes,
  ) {}

  /** The footfalls since the last call, for the thump that goes with each. */
  takeSteps() {
    const steps = this.steps
    this.steps = []
    return steps
  }

  /**
   * Swings the legs by how far the hull at `position`, turned by `rotation`, has moved since the last
   * call. Returns the time since then, the ground speed and a lean (-1..1, + towards the right foot) for
   * the hull to sway over whichever foot is down.
   */
  update(position: readonly [number, number, number], rotation: readonly [number, number, number, number], tucked = false) {
    const { thigh: THIGH, shin: SHIN, pad: PAD, stand: STAND, hipDrop } = this.sizes
    const now = performance.now()
    const dt = this.seen ? Math.min(0.1, Math.max(0.001, (now - this.stamp) / 1000)) : 0.016
    this.stamp = now
    const here = new THREE.Vector3(position[0], 0, position[2])
    // Nose pitch: leaning forward dips it. The legs undo it, so the feet stay flat on the ground.
    const nose = new THREE.Vector3(0, 0, -1).applyQuaternion(new THREE.Quaternion(...rotation))
    const pitch = Math.asin(Math.max(-1, Math.min(1, nose.y)))
    const heading = new THREE.Vector3(nose.x, 0, nose.z)
    if (heading.lengthSq() < 1e-6) heading.set(0, 0, -1)
    heading.normalize()
    // Right of the heading, in the ground plane.
    const right = new THREE.Vector3(-heading.z, 0, heading.x)
    const yaw = Math.atan2(-heading.x, -heading.z)
    const velocity = here.clone().sub(this.last).divideScalar(dt)
    let spin = 0
    if (this.seen) {
      spin = yaw - this.yaw
      spin = Math.atan2(Math.sin(spin), Math.cos(spin)) / dt
    }
    const speed = velocity.length()
    // A jump in position (a respawn) puts the feet back under the hull, and so does a landing.
    const landing = this.wasTucked && !tucked
    this.wasTucked = tucked
    const restart = !this.seen || speed > 40 || tucked
    this.last.copy(here)
    this.yaw = yaw
    this.seen = true
    this.tuck += ((tucked ? 1 : 0) - this.tuck) * Math.min(1, dt * 8)
    const along = velocity.dot(heading)

    let lean = 0
    this.hips.forEach((hip, index) => {
      const foot = this.feet[index]!
      const side = index === 0 ? -1 : 1
      const hipX = position[0] + right.x * side * WALKER_FOOT_HALF
      const hipZ = position[2] + right.z * side * WALKER_FOOT_HALF
      if (restart) {
        foot.planted = true
        foot.x = hipX
        foot.z = hipZ
      }
      if (landing) this.steps.push({ at: [hipX, position[1] - hipDrop - STAND, hipZ], weight: 1 })
      // How fast this hip moves along the heading: the hull's own speed, and the turn swinging it round.
      const hipRate = along - side * WALKER_FOOT_HALF * spin
      const direction = Math.sign(hipRate)
      const wanted = direction * STEP * Math.min(1, Math.abs(hipRate) / 6)
      const moving = Math.abs(hipRate) > 0.15
      // Where the foot is, ahead of the hip along the heading.
      let reach = (foot.x - hipX) * heading.x + (foot.z - hipZ) * heading.z
      let lift = 0

      if (foot.planted) {
        const other = this.feet[1 - index]!
        const behind = moving ? reach * direction < -Math.max(SETTLE, Math.abs(wanted)) : Math.abs(reach) > SETTLE
        const stretched = Math.abs(reach) > STEP * 1.4
        if (behind && (other.planted || stretched)) {
          foot.planted = false
          foot.from = reach
          foot.to = moving ? wanted : 0
          foot.time = 0
          // Quick enough to be down before the other foot has run out of room.
          foot.duration = Math.min(0.4, Math.max(0.15, (1.6 * STEP) / Math.max(1, Math.abs(hipRate))))
        }
      }
      if (!foot.planted) {
        foot.time += dt
        const s = Math.min(1, foot.time / foot.duration)
        reach = foot.from + (foot.to - foot.from) * (s * s * (3 - 2 * s))
        lift = Math.sin(Math.PI * s) * LIFT
        lean -= side * Math.sin(Math.PI * s)
        if (s >= 1) {
          foot.planted = true
          foot.x = hipX + heading.x * reach
          foot.z = hipZ + heading.z * reach
          lift = 0
          if (!restart) this.steps.push({ at: [foot.x, position[1] - hipDrop - STAND, foot.z], weight: Math.min(1, 0.45 + speed / 8) })
        }
      }
      lift += this.tuck * TUCK

      // Two-bone IK in the leg's plane: the ankle goes `reach` ahead of the hip and `STAND - PAD - lift`
      // below it. The hull's tilt moves the hip, so the target is turned back by the pitch.
      const drop = STAND - PAD - lift
      const distance = Math.min(Math.hypot(reach, drop), THIGH + SHIN - 0.01)
      const toTarget = Math.atan2(reach, drop)
      const kneeInside = Math.acos(Math.max(-1, Math.min(1, (THIGH * THIGH + SHIN * SHIN - distance * distance) / (2 * THIGH * SHIN))))
      const thighOffset = Math.acos(Math.max(-1, Math.min(1, (THIGH * THIGH + distance * distance - SHIN * SHIN) / (2 * THIGH * distance))))
      // The knee points back, like a bird's: the thigh goes behind the line to the foot and the shin forward of it.
      const thighAngle = toTarget - thighOffset
      const bend = Math.PI - kneeInside
      // A positive rotation about x swings a hanging leg towards -z, the way the nose points.
      hip.rotation.x = thighAngle - pitch
      this.knees[index]!.rotation.x = bend
      this.ankles[index]!.rotation.x = -(hip.rotation.x + this.knees[index]!.rotation.x) - pitch
    })
    return { dt, speed, spin, lean: Math.max(-1, Math.min(1, lean)) }
  }
}
