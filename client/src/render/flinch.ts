import { Quaternion, Vector3 } from '@ragnarok/engine'

/** How fast a vehicle of size 1 (a tank) rocks back from a blow, Hz, and how much of each swing is lost (0 none, 1 none comes back). */
const FREQUENCY = 3
const DAMPING = 0.3
/** How fast the rocking dies down on a bigger vehicle: frequency over size to this power. */
const SIZE_SLOWS = 0.6
/** The rocking speed a full blow (a shell through it) gives a vehicle of size 1, rad/s; less on a bigger one, as size to this power. */
const KICK = 0.28
const SIZE_STEADIES = 0.5
/** How far a full blow shoves it along the round's way, m/s, at size 1. */
const SHOVE = 0.35
/** The jolt a blow sends through the metal: how far it shakes at size 1, metres, how fast, Hz, and how soon it dies, seconds. */
const SHIVER = 0.025
const SHIVER_HZ = 24
const SHIVER_LIFE = 0.07
/** The furthest it is ever tipped by blows, radians, however many land at once. */
const MOST = 0.05

interface Spring {
  /** Tipped about this axis by its length, radians, in the world, and how fast that changes. */
  tilt: Vector3
  spin: Vector3
  /** Shoved this far, metres, and how fast. */
  offset: Vector3
  velocity: Vector3
  /** The jolt: its size, metres, its way, and the time since it came, seconds. */
  shiver: number
  shiverWay: Vector3
  shiverAge: number
  size: number
}

type V = { x: number; y: number; z: number }

const scratch = { pivot: new Vector3(), arm: new Vector3(), turn: new Quaternion(), axis: new Vector3() }

/**
 * Vehicles rocking from the rounds that strike them, as drawn (HULL-DAMAGE.md). The server pushes the
 * vehicle too, but its push reaches the picture a snapshot later and is slight on a Titan: this is the
 * blow as it lands. Each vehicle is a damped spring about a point on the ground under it, kicked by
 * each hit about the line from there to where it struck, with a short jolt of the metal on top.
 */
export class Flinch {
  private readonly springs = new Map<number, Spring>()

  /**
   * A blow on vehicle `id`, of `size` (1 a tank), at `at` in the world, from a round going along `way`
   * (unit, world), around `pivot`, the point on the ground under it. `strength` is 1 for a shell through
   * it and less for anything less.
   */
  hit(id: number, size: number, pivot: V, at: V, way: V, strength: number) {
    let spring = this.springs.get(id)
    if (!spring) {
      spring = { tilt: new Vector3(), spin: new Vector3(), offset: new Vector3(), velocity: new Vector3(), shiver: 0, shiverWay: new Vector3(), shiverAge: 0, size }
      this.springs.set(id, spring)
    }
    spring.size = size
    const arm = scratch.arm.set(at.x - pivot.x, at.y - pivot.y, at.z - pivot.z)
    const length = arm.length()
    if (length > 1e-3) {
      // About the line square to the arm and the round's way: struck high on its side, it tips away.
      const axis = scratch.axis.crossVectors(arm, way).divideScalar(length)
      spring.spin.addScaledVector(axis, (KICK * strength) / size ** SIZE_STEADIES)
    }
    spring.velocity.addScaledVector(way, (SHOVE * strength) / size)
    spring.shiver = Math.max(spring.shiver * Math.exp(-spring.shiverAge / SHIVER_LIFE), SHIVER * strength * Math.sqrt(size))
    spring.shiverWay.set(way.x, way.y, way.z)
    spring.shiverAge = 0
  }

  /**
   * Vehicle `id`'s pose as drawn, `position` and `rotation`, moved by its rocking after `dt` more
   * seconds, rocked about the point `down` metres below it along its up. Changes both in place.
   */
  pose(id: number, position: [number, number, number], rotation: [number, number, number, number], down: number, dt: number) {
    const spring = this.springs.get(id)
    if (!spring) return
    const w = (2 * Math.PI * FREQUENCY) / spring.size ** SIZE_SLOWS
    // A step longer than a frame or two is taken in pieces, so a slow frame never throws it.
    const steps = Math.min(8, Math.max(1, Math.ceil(dt / (1 / 120))))
    const h = Math.min(dt, 0.1) / steps
    for (let i = 0; i < steps; i++) {
      spring.spin.addScaledVector(spring.tilt, -w * w * h).multiplyScalar(1 - 2 * DAMPING * w * h)
      spring.tilt.addScaledVector(spring.spin, h)
      spring.velocity.addScaledVector(spring.offset, -w * w * h).multiplyScalar(1 - 2 * DAMPING * w * h)
      spring.offset.addScaledVector(spring.velocity, h)
    }
    if (spring.tilt.length() > MOST) spring.tilt.setLength(MOST)
    spring.shiverAge += dt
    const jolt = spring.shiver * Math.exp(-spring.shiverAge / SHIVER_LIFE) * Math.sin(2 * Math.PI * SHIVER_HZ * spring.shiverAge)
    const settled = spring.tilt.lengthSq() < 1e-10 && spring.spin.lengthSq() < 1e-10 && spring.offset.lengthSq() < 1e-10 && spring.velocity.lengthSq() < 1e-10 && spring.shiverAge > SHIVER_LIFE * 6
    if (settled) {
      this.springs.delete(id)
      return
    }
    const q = new Quaternion(...rotation)
    const up = new Vector3(0, 1, 0).applyQuaternion(q)
    const pivot = scratch.pivot.set(...position).addScaledVector(up, -down)
    const angle = spring.tilt.length()
    const turn = angle > 1e-6 ? scratch.turn.setFromAxisAngle(scratch.axis.copy(spring.tilt).divideScalar(angle), angle) : scratch.turn.identity()
    const moved = new Vector3(...position).sub(pivot).applyQuaternion(turn).add(pivot).add(spring.offset).addScaledVector(spring.shiverWay, jolt)
    q.premultiply(turn)
    position[0] = moved.x
    position[1] = moved.y
    position[2] = moved.z
    rotation[0] = q.x
    rotation[1] = q.y
    rotation[2] = q.z
    rotation[3] = q.w
  }

  forget(id?: number) {
    if (id === undefined) this.springs.clear()
    else this.springs.delete(id)
  }
}
