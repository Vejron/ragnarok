import { Quaternion, Vector3 } from '@ragnarok/engine'
import { TICK_DT, ROADSTER_AIR } from './consts'
import { controlsOf, type CarBodyState, type Neighbour, type SimMotor, type VehiclePose } from './motor'
import type { CarvedBuilding, InputFrame } from '@/net/codec'
import type { DriveState } from '@/entities/units'
import { isRotorcraft } from './vehicles'

/** A correction larger than this is a teleport (respawn, unflip), not a misprediction: snap. */
const SNAP_DISTANCE = 3
/** How fast the visual error left by a correction decays, per second. */
const ERROR_DECAY = 10
const MAX_PENDING = 120
/** A hole the vehicle was predicted to break in a wall is closed again when the server has stepped this many inputs past the ram without confirming it. */
const RAM_PATIENCE = 60
/** Vehicles within this of the driven one are simulated beside it, metres: further than two closing at full speed cover in a replay. */
const NEIGHBOUR_REACH = 30
/** One already simulated stays so out to this, metres, so that it does not flicker at the edge. */
const NEIGHBOUR_KEEP = 40
/** The most vehicles simulated beside the driven one: each is stepped in every replayed tick. */
const NEIGHBOUR_MAX = 6

/** A drawn vehicle: world pose and front wheel steering as a fraction of the lock. */
export interface DrivenPose {
  position: [number, number, number]
  rotation: [number, number, number, number]
  steer: number
}

/**
 * Client-side prediction of the vehicle the local player drives: the server's own vehicle physics
 * (`crates/sim`, in WASM), in the same world as the soldier, where the vehicle stands in for its
 * snapshot solid while it is predicted. Inputs are applied at once and replayed over each
 * authoritative state, as the soldier's are; what is left of a correction is kept as a visual offset
 * that decays.
 */
/** A Titan's feet as a snapshot has them: where each is in the world (left, right), and its `step` and `weight`. */
export interface Feet {
  at: readonly [readonly [number, number, number], readonly [number, number, number]]
  step: number
  weight: number
}

export class VehicleDriver {
  private id = 0
  private kind = 0
  private pending: InputFrame[] = []
  private previous: CarBodyState | null = null
  private current: CarBodyState | null = null
  private readonly errorPosition = new Vector3()
  private readonly errorRotation = new Quaternion()
  /** Size of the last correction, metres. */
  lastCorrection = 0
  /** How hard the newest input works the engine, -1..1: what its engine is heard to do at once (`Steer.throttle`). */
  private worked = 0
  /**
   * The rams that broke through a wall which the server has not yet answered: the input each
   * happened on, and the velocity the vehicle went on with. A replay meets the hole already made,
   * so it is given that velocity at that input instead of ramming again.
   */
  private rams: { seq: number; linvel: [number, number, number] }[] = []
  /** The input of the newest ram whose hole waits for the server's word, or null. */
  private breached: number | null = null
  /** Told of each hole the predicted vehicle breaks in a building, at once: it is drawn ahead of the server's word. */
  onBreach: (holes: readonly CarvedBuilding[]) => void = () => {}
  /** Told when the holes the server never confirmed are closed again. */
  onForget: () => void = () => {}
  /** The vehicles simulated beside the predicted one since the last reconcile. */
  private beside: ReadonlySet<number> = new Set()

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

  /** How hard the driver works the predicted vehicle's engine, -1..1: the throttle, or a helicopter's collective. */
  get throttle(): number {
    return this.id !== 0 ? this.worked : 0
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
    this.rams = this.rams.filter((ram) => ram.seq > ack)
    if (this.breached !== null && ack >= this.breached + RAM_PATIENCE) this.forget()
  }

  /** Applies one new input to the predicted vehicle, if there is one, and remembers it. */
  apply(input: InputFrame) {
    this.record(input)
    if (this.id === 0) return
    this.previous = this.current
    this.current = this.stepWith(input)
  }

  private stepWith(input: InputFrame): CarBodyState | null {
    const controls = controlsOf(input, this.kind)
    this.worked = isRotorcraft(this.kind) ? controls.lift : controls.throttle
    const broke = this.motor.driveCar(this.id, controls, TICK_DT)
    const known = this.rams.find((ram) => ram.seq === input.seq)
    // A replay drives through the hole this input first broke: it slows as it did then.
    if (known && !broke) this.motor.setCarVelocity(this.id, known.linvel)
    const state = this.motor.carState(this.id)
    if (broke) this.onBreach(this.motor.takeRams())
    if (broke && state) {
      this.rams = this.rams.filter((ram) => ram.seq !== input.seq)
      this.rams.push({ seq: input.seq, linvel: state.linvel })
      this.breached = input.seq
    }
    return state
  }

  /**
   * The vehicles of a snapshot to simulate beside `vehicle`, the one driven, nearest first: those of
   * `others` within reach of it, and a little further for one that already is.
   */
  near(vehicle: VehiclePose, others: readonly Neighbour[]): Neighbour[] {
    const away = (other: VehiclePose) => Math.hypot(other.x - vehicle.x, other.y - vehicle.y, other.z - vehicle.z)
    return others
      .filter((other) => other.id !== vehicle.id && away(other) < (this.beside.has(other.id) ? NEIGHBOUR_KEEP : NEIGHBOUR_REACH))
      .sort((a, b) => away(a) - away(b))
  }

  /**
   * Rewinds to the server's state as of input `ack` and replays everything newer. `tick` is the
   * snapshot's: the server's next step, which the first replayed input is, so the waves on open water
   * are where the server has them. `feet` are a Titan's, which it stands on: without them the replay
   * would walk it on from wherever this client's own copy of them had got to. `neighbours` are the
   * same snapshot's vehicles around it (`near`), rewound and replayed with it, so that what it rams
   * gives way as it does on the server.
   */
  reconcile(ack: number, vehicle: VehiclePose, drive: DriveState, tick?: number, feet?: Feet | null, neighbours: readonly Neighbour[] = []) {
    this.acknowledge(ack)
    if (this.id !== vehicle.id || this.kind !== vehicle.kind) {
      this.leave()
      this.motor.startCar(vehicle.id, vehicle.kind, [vehicle.x, vehicle.y, vehicle.z])
      this.id = vehicle.id
      this.kind = vehicle.kind
    }
    const before = this.current
    this.beside = this.motor.setNeighbours(neighbours, NEIGHBOUR_MAX)
    this.motor.setCarState(this.id, vehicle, drive)
    if (feet) this.motor.setCarFeet(this.id, feet.at, feet.step, feet.weight)
    if (tick !== undefined) this.motor.setTick(tick)
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
    const delta = new Vector3(...before.position).sub(new Vector3(...state.position))
    this.lastCorrection = delta.length()
    if (this.lastCorrection > SNAP_DISTANCE) {
      this.errorPosition.set(0, 0, 0)
      this.errorRotation.identity()
      return
    }
    // What was drawn stays where it was, and eases onto the corrected pose.
    this.errorPosition.add(delta)
    const was = new Quaternion(...before.rotation)
    const now = new Quaternion(...state.rotation)
    this.errorRotation.multiply(was.multiply(now.invert()))
  }

  /** Stops predicting: the player left the driver's seat. The next snapshot makes the vehicle, and those simulated beside it, solids again. */
  leave() {
    if (this.id !== 0) this.motor.stopCar(this.id)
    this.beside = new Set()
    this.rams = []
    if (this.breached !== null) this.forget()
    this.previous = null
    this.current = null
    this.id = 0
    this.errorPosition.set(0, 0, 0)
    this.errorRotation.identity()
  }

  /** Gives up on the holes the server has not confirmed: they close in the physics, and whoever draws them is told. */
  private forget() {
    this.breached = null
    this.motor.forgetRams()
    this.onForget()
  }

  /** The pose to draw, between the last two ticks, plus the decaying correction. */
  pose(alpha: number, dtSeconds: number): DrivenPose | null {
    const { current } = this
    if (this.id === 0 || !current) return null
    const previous = this.previous ?? current
    const decay = Math.exp(-ERROR_DECAY * dtSeconds)
    this.errorPosition.multiplyScalar(decay)
    this.errorRotation.slerp(IDENTITY, 1 - decay)
    const position = new Vector3(...previous.position).lerp(new Vector3(...current.position), alpha).add(this.errorPosition)
    const rotation = new Quaternion(...previous.rotation).slerp(new Quaternion(...current.rotation), alpha)
    rotation.premultiply(this.errorRotation)
    return { position: [position.x, position.y, position.z], rotation: [rotation.x, rotation.y, rotation.z, rotation.w], steer: current.steerFraction }
  }

  dispose() {
    this.leave()
  }
}

const IDENTITY = new Quaternion()
