import { TICK_DT } from './consts'
import type { CharacterState, Stepper, Vec3 } from './motor'
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
  /** The inputs not yet acknowledged, each with the server tick it was first stepped at (null unknown). */
  private pending: { input: InputFrame; tick: number | null }[] = []
  /** The server tick `state` was stepped at, which the waves a swimmer floats on were at; null unknown. */
  stateTick: number | null = null
  /** The same for `previous`. */
  previousTick: number | null = null
  /** Sets the sim's clock before a step: the waves a swimmer floats on run on the server's tick. */
  setTick: (tick: number) => void = () => {}
  /** Visual-only offset from corrections, decaying to zero. */
  readonly error: Vec3 = [0, 0, 0]
  /** Size of the last correction, metres, for the HUD. */
  lastCorrection = 0
  /** Walking speed multiplier for an input, e.g. slower aiming a gun. The game sets it from the hotbar. */
  speedScale: (input: InputFrame) => number = () => 1
  /** Whether a gun is in hand for an input, which leaves the skis no poles. The game sets it from the hotbar. */
  armed: (input: InputFrame) => boolean = () => false

  constructor(private readonly step: Stepper, initial: CharacterState) {
    this.state = cloneState(initial)
    this.previous = cloneState(initial)
  }

  /** Applies one new input locally, at server tick `tick` (the server's tick about now), if known. */
  apply(input: InputFrame, tick: number | null = null) {
    this.previous = this.state
    this.previousTick = this.stateTick
    if (tick !== null) this.setTick(tick)
    this.state = this.step(this.state, input, TICK_DT, this.speedScale(input), this.armed(input))
    this.stateTick = tick
    this.pending.push({ input, tick })
    if (this.pending.length > MAX_PENDING) this.pending.shift()
  }

  /**
   * Remembers an input without simulating it, while the player is in a car and the soldier does
   * not move. If the player steps out before the server's ack catches up, these inputs are
   * replayed over the state it reports.
   */
  record(input: InputFrame, tick: number | null = null) {
    this.pending.push({ input, tick })
    if (this.pending.length > MAX_PENDING) this.pending.shift()
  }

  /** Drops inputs the server has applied, without touching the simulated state. */
  acknowledge(ack: number) {
    this.pending = this.pending.filter(({ input }) => input.seq > ack)
  }

  /** Forgets unsent or unacknowledgeable inputs, e.g. those made while offline. */
  clearPending() {
    this.pending = []
  }

  get pendingCount() {
    return this.pending.length
  }

  /**
   * Rewinds to the server's state as of input `ack`, at server tick `tick` if known, then replays
   * everything newer, each input at the tick it was first stepped at, so a swimmer floats on the same
   * waves as before rather than all on the snapshot's.
   */
  reconcile(ack: number, authoritative: CharacterState, tick: number | null = null) {
    this.pending = this.pending.filter(({ input }) => input.seq > ack)
    const before = this.state.position
    let state = cloneState(authoritative)
    let previous = state
    let stateTick = tick
    let previousTick = tick
    for (const { input, tick: at } of this.pending) {
      previous = state
      previousTick = stateTick
      const stepped = at ?? tick
      if (stepped !== null) this.setTick(stepped)
      state = this.step(state, input, TICK_DT, this.speedScale(input), this.armed(input))
      stateTick = stepped
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
    this.stateTick = stateTick
    this.previousTick = previous === state ? stateTick : previousTick
  }

  /** The server tick of the position `renderPosition(alpha)` draws, blended as it is; null unknown. */
  renderTick(alpha: number): number | null {
    const a = this.previousTick ?? this.stateTick
    const b = this.stateTick ?? a
    return a === null || b === null ? null : a + (b - a) * alpha
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
