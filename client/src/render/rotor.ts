import { Group, type Node, type Vector3 } from '@ragnarok/engine'
import type { Arms, Turret } from '@/entities/units'
import type { VehicleModel } from './car'

/** How a helicopter's rotors are drawn turning. */
export interface RotorLook {
  /** The main rotor's speed at its governed speed, rad/s. */
  speed: number
  /** The tail rotor's speed, as a share of the main rotor's. */
  tail: number
}

/** How fast the drawn rotor's speed follows the real one, rad/s². */
const ROTOR_ACCEL = 14

/**
 * What every helicopter's model shares: a `rotor` and a `tail_rotor` that turn as fast as the physics
 * has the rotor turning, and a searchlight.
 */
export abstract class RotorModel implements VehicleModel {
  readonly root = new Group()
  private readonly rotor: Node
  private readonly tailRotor: Node
  private rotorSpeed = 0
  /** The rotor's speed as the physics has it, a share of governed (`Thrust.air`), or null if not told. */
  private rotorShare: number | null = null
  private lastUpdate = performance.now()

  constructor(model: Node, private readonly look: RotorLook) {
    this.root.add(model)
    this.rotor = model.getObjectByName('rotor')!
    this.tailRotor = model.getObjectByName('tail_rotor')!
  }

  /** The rotor's speed as the physics has it, a share of its governed speed (`Thrust.air`), or null if not known. */
  setRotor(share: number | null) {
    this.rotorShare = share
  }

  /**
   * Places the helicopter and turns its rotors, at the speed `setRotor` was last given, or before any
   * as if spun up while seat 0 is taken. `steer` is unused: its lean and turn are in the orientation itself.
   */
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    _steer: number,
    seats: readonly number[],
    _turret?: Turret | null,
    _arms?: Arms | null,
  ) {
    const now = performance.now()
    const dt = Math.min(0.1, (now - this.lastUpdate) / 1000)
    this.lastUpdate = now
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    const { speed, tail } = this.look
    const target = this.rotorShare !== null ? this.rotorShare * speed : (seats[0] ?? 0) !== 0 ? speed : 0
    const step = ROTOR_ACCEL * dt
    this.rotorSpeed += Math.max(-step, Math.min(step, target - this.rotorSpeed))
    this.rotor.rotation.y += this.rotorSpeed * dt
    this.tailRotor.rotation.x += this.rotorSpeed * tail * dt
  }

  /** Where its searchlight shines from and which way, as of the last `update`. The vectors are reused by the next call. */
  abstract searchlight(): { at: Vector3; direction: Vector3 }

  dispose() {
    this.root.removeFromParent()
  }
}
