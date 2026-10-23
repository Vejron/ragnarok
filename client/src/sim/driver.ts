import * as THREE from 'three'
import { TICK_DT, ROADSTER_AIR } from './consts'
import { controlsOf, type CarBodyState, type SimMotor, type VehiclePose } from './motor'
import type { InputFrame } from '@/net/codec'
import type { DriveState } from '@/entities/units'

/** A correction larger than this is a teleport (respawn, unflip), not a misprediction: snap. */
const SNAP_DISTANCE = 3
/** How fast the visual error left by a correction decays, per second. */
const ERROR_DECAY = 10
const MAX_PENDING = 120

/** A drawn vehicle: world pose and front wheel steering as a fraction of the lock. */
export interface DrivenPose {
  position: [number, number, number]
  rotation: [number, number, number, number]
  steer: number
}

/**
 * Client-side prediction of the vehicle the local player drives: the server's own vehicle physics
 * (`crates/sim`, in WASM), in the same world as the stickman, where the vehicle stands in for its
 * snapshot solid while it is predicted. Inputs are applied at once and replayed over each
 * authoritative state, as the stickman's are; what is left of a correction is kept as a visual offset
 * that decays.
 */
export class VehicleDriver {
  private id = 0
  private kind = 0
  private pending: InputFrame[] = []
  private previous: CarBodyState | null = null
  private current: CarBodyState | null = null
  private readonly errorPosition = new THREE.Vector3()
  private readonly errorRotation = new THREE.Quaternion()
  /** Size of the last correction, metres. */
  lastCorrection = 0

  constructor(private readonly motor: SimMotor) {}

  /** Whether a vehicle is being predicted, and which. */
  get driving(): number | null {
    return this.id !== 0 ? this.id : null
  }

  /** The predicted vehicle's air tank or stamina (0..1) and its `THRUST_*` flags; null for a kind without either. */
  get thrust(): { air: number; flags: number } | null {
    const state = this.current
    return this.id !== 0 && state && state.tank !== null ? { air: state.tank / ROADSTER_AIR, flags: state.firing } : null
  }

  /** Remembers an input the vehicle has not been told of, for a replay once the player drives. */
  record(input: InputFrame) {
    this.pending.push(input)
    if (this.pending.length > MAX_PENDING) this.pending.shift()
  }

  /** Forgets inputs that were never sent, e.g. those made while offline. */
  clearPending() {
    this.pending = []
  }

  acknowledge(ack: number) {
    this.pending = this.pending.filter((input) => input.seq > ack)
  }

  /** Applies one new input to the predicted vehicle, if there is one, and remembers it. */
  apply(input: InputFrame) {
    this.record(input)
    if (this.id === 0) return
    this.previous = this.current
    this.current = this.stepWith(input)
  }

  private stepWith(input: InputFrame): CarBodyState | null {
    this.motor.driveCar(this.id, controlsOf(input, this.kind), TICK_DT)
    return this.motor.carState(this.id)
  }

  /** Rewinds to the server's state as of input `ack` and replays everything newer. */
  reconcile(ack: number, vehicle: VehiclePose, drive: DriveState) {
    this.acknowledge(ack)
    if (this.id !== vehicle.id || this.kind !== vehicle.kind) {
      this.leave()
      this.motor.startCar(vehicle.id, vehicle.kind, [vehicle.x, vehicle.y, vehicle.z])
      this.id = vehicle.id
      this.kind = vehicle.kind
    }
    const before = this.current
    this.motor.setCarState(this.id, vehicle, drive)
    let state = this.motor.carState(this.id)
    let previous = state
    for (const input of this.pending) {
      previous = state
      state = this.stepWith(input)
    }
    this.previous = previous
    this.current = state
    if (!before || !state) {
      this.lastCorrection = 0
      this.errorPosition.set(0, 0, 0)
      this.errorRotation.identity()
      return
    }
    const delta = new THREE.Vector3(...before.position).sub(new THREE.Vector3(...state.position))
    this.lastCorrection = delta.length()
    if (this.lastCorrection > SNAP_DISTANCE) {
      this.errorPosition.set(0, 0, 0)
      this.errorRotation.identity()
      return
    }
    // What was drawn stays where it was, and eases onto the corrected pose.
    this.errorPosition.add(delta)
    const was = new THREE.Quaternion(...before.rotation)
    const now = new THREE.Quaternion(...state.rotation)
    this.errorRotation.multiply(was.multiply(now.invert()))
  }

  /** Stops predicting: the player left the driver's seat. The next snapshot makes the vehicle a solid again. */
  leave() {
    if (this.id !== 0) this.motor.stopCar(this.id)
    this.previous = null
    this.current = null
    this.id = 0
    this.errorPosition.set(0, 0, 0)
    this.errorRotation.identity()
  }

  /** The pose to draw, between the last two ticks, plus the decaying correction. */
  pose(alpha: number, dtSeconds: number): DrivenPose | null {
    const { current } = this
    if (this.id === 0 || !current) return null
    const previous = this.previous ?? current
    const decay = Math.exp(-ERROR_DECAY * dtSeconds)
    this.errorPosition.multiplyScalar(decay)
    this.errorRotation.slerp(IDENTITY, 1 - decay)
    const position = new THREE.Vector3(...previous.position).lerp(new THREE.Vector3(...current.position), alpha).add(this.errorPosition)
    const rotation = new THREE.Quaternion(...previous.rotation).slerp(new THREE.Quaternion(...current.rotation), alpha)
    rotation.premultiply(this.errorRotation)
    return { position: [position.x, position.y, position.z], rotation: [rotation.x, rotation.y, rotation.z, rotation.w], steer: current.steerFraction }
  }

  dispose() {
    this.leave()
  }
}

const IDENTITY = new THREE.Quaternion()
