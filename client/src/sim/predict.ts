import { TICK_DT } from './consts'
import { stepCharacter, type CharacterMotor, type CharacterState, type Vec3 } from './physics'
import type { InputFrame } from '@/net/codec'

/** Inputs older than this are discarded even if never acknowledged (the server dropped them). */
const MAX_PENDING = 120
/** A correction larger than this is a teleport (spawn, respawn), not a misprediction: snap. */
const SNAP_DISTANCE = 3
/** How fast the visual error left by a correction decays, per second. */
const ERROR_DECAY = 15

const cloneState = (s: CharacterState): CharacterState => ({ ...s, position: [...s.position], knock: [...s.knock] })

/**
 * Client-side prediction for the local player.
 *
 * Every tick the input is applied locally right away and kept until the server acknowledges it.
 * When a snapshot arrives, the authoritative state replaces ours, the acknowledged inputs are
 * dropped and the rest are replayed on top. Whatever difference remains is not snapped but kept
 * as a visual offset that decays, so small corrections never pop.
 */
export class Predictor {
  /** State after the newest input. */
  state: CharacterState
  /** State one tick earlier, so rendering can blend between ticks. */
  previous: CharacterState
  private pending: InputFrame[] = []
  /** Visual-only offset from corrections, decaying to zero. */
  readonly error: Vec3 = [0, 0, 0]
  /** Size of the last correction, metres, for the HUD. */
  lastCorrection = 0
  /** Walking speed multiplier for an input, e.g. slower aiming a gun. The game sets it from the hotbar. */
  speedScale: (input: InputFrame) => number = () => 1

  constructor(private readonly motor: CharacterMotor, initial: CharacterState) {
    this.state = cloneState(initial)
    this.previous = cloneState(initial)
  }

  /** Applies one new input locally. */
  apply(input: InputFrame) {
    this.previous = this.state
    this.state = stepCharacter(this.motor, this.state, input, TICK_DT, this.speedScale(input))
    this.pending.push(input)
    if (this.pending.length > MAX_PENDING) this.pending.shift()
  }

  /**
   * Remembers an input without simulating it, while the player is in a car and the stickman does
   * not move. If the player steps out before the server's ack catches up, these inputs are
   * replayed over the state it reports.
   */
  record(input: InputFrame) {
    this.pending.push(input)
    if (this.pending.length > MAX_PENDING) this.pending.shift()
  }

  /** Drops inputs the server has applied, without touching the simulated state. */
  acknowledge(ack: number) {
    this.pending = this.pending.filter((input) => input.seq > ack)
  }

  /** Forgets unsent or unacknowledgeable inputs, e.g. those made while offline. */
  clearPending() {
    this.pending = []
  }

  get pendingCount() {
    return this.pending.length
  }

  /** Rewinds to the server's state as of input `ack`, then replays everything newer. */
  reconcile(ack: number, authoritative: CharacterState) {
    this.pending = this.pending.filter((input) => input.seq > ack)
    const before = this.state.position
    let state = cloneState(authoritative)
    let previous = state
    for (const input of this.pending) {
      previous = state
      state = stepCharacter(this.motor, state, input, TICK_DT, this.speedScale(input))
    }
    const delta: Vec3 = [before[0] - state.position[0], before[1] - state.position[1], before[2] - state.position[2]]
    this.lastCorrection = Math.hypot(...delta)
    if (this.lastCorrection > SNAP_DISTANCE) {
      this.error.fill(0)
      previous = state
    } else {
      for (let i = 0; i < 3; i++) this.error[i]! += delta[i]!
    }
    this.state = state
    this.previous = previous
  }

  /** Position to draw: blended between the last two ticks, plus the decaying correction offset. */
  renderPosition(alpha: number, dtSeconds: number): Vec3 {
    const decay = Math.exp(-ERROR_DECAY * dtSeconds)
    for (let i = 0; i < 3; i++) this.error[i]! *= decay
    const a = this.previous.position
    const b = this.state.position
    return [0, 1, 2].map((i) => a[i]! + (b[i]! - a[i]!) * alpha + this.error[i]!) as Vec3
  }
}
