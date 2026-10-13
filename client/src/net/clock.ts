import { SNAPSHOT_EVERY, TICK_RATE } from '@/sim/consts'

/** Weight of each new offset sample. Small, so one late packet barely moves the clock. */
const OFFSET_SMOOTHING = 0.05
/** Late packets (negative deviation) get a smaller weight than early ones: the clock should track
 *  the fastest path, not the average delay. */
const LATE_SMOOTHING = 0.01
const JITTER_SMOOTHING = 0.1
/** Weight of each new offset sample while the server is falling behind our clock. */
const DRIFT_SMOOTHING = 0.2
/** Weight of each sample in the running average of signed deviation: about a second of snapshots. */
const BIAS_SMOOTHING = 0.03
/** Arrivals this many ticks late on average, for about a second, are not jitter: the server's
 *  ticks are not keeping pace with our clock (a loaded server), and the offset must follow. */
const DRIFT_TICKS = SNAPSHOT_EVERY
/** Deviation beyond this many ticks means the server restarted or the tab slept: resync. */
const RESYNC_TICKS = 60
/** Render-delay bounds, in ticks. */
const MIN_DELAY = SNAPSHOT_EVERY * 1.5
const MAX_DELAY = 30

/**
 * Estimates the server tick "as currently arriving" from snapshot arrival times, and an adaptive
 * interpolation delay from their jitter (Quake 3 / Source style).
 */
export class ServerClock {
  /** serverTick ≈ localMs * rate / 1000 + offset */
  private offset: number | null = null
  /** Smoothed absolute deviation of arrivals, in ticks. */
  jitter = 0
  /** Smoothed signed deviation of arrivals, in ticks: negative when they keep coming late. */
  private bias = 0

  observe(serverTick: number, localMs: number) {
    const sample = serverTick - (localMs * TICK_RATE) / 1000
    if (this.offset === null || Math.abs(sample - this.offset) > RESYNC_TICKS) {
      this.offset = sample
      this.jitter = 0
      this.bias = 0
      return
    }
    const deviation = sample - this.offset
    this.bias += (deviation - this.bias) * BIAS_SMOOTHING
    // Late packets normally move the offset slowly, so a burst of them widens the delay rather
    // than shifting the clock. A server that ticks slower than nominal is late every time, though,
    // and at the slow weight the offset trails it by hundreds of milliseconds, for as long as it
    // stays loaded: follow it quickly once lateness is the rule, not the exception.
    const drifting = this.bias < -DRIFT_TICKS
    const weight = drifting ? DRIFT_SMOOTHING : deviation > 0 ? OFFSET_SMOOTHING : LATE_SMOOTHING
    this.offset += deviation * weight
    this.jitter += (Math.abs(deviation) - this.jitter) * JITTER_SMOOTHING
  }

  get synced() {
    return this.offset !== null
  }

  /** Estimated newest server tick at local time `localMs`. */
  tickAt(localMs: number) {
    return (localMs * TICK_RATE) / 1000 + (this.offset ?? 0)
  }

  /** How far behind `tickAt` to render remote entities: two snapshot intervals plus jitter. */
  get interpDelay() {
    return Math.min(MAX_DELAY, Math.max(MIN_DELAY, SNAPSHOT_EVERY * 2 + this.jitter * 2))
  }

  reset() {
    this.offset = null
    this.jitter = 0
    this.bias = 0
  }
}

/** Round-trip time from ping/pong, smoothed for display. */
export class RttEstimator {
  private sent = new Map<number, number>()
  private nonce = 0
  rtt: number | null = null

  next(localMs: number) {
    this.nonce = (this.nonce + 1) >>> 0
    this.sent.set(this.nonce, localMs)
    if (this.sent.size > 16) this.sent.delete(this.sent.keys().next().value!)
    return this.nonce
  }

  pong(nonce: number, localMs: number) {
    const sentAt = this.sent.get(nonce)
    if (sentAt === undefined) return
    this.sent.delete(nonce)
    const sample = localMs - sentAt
    this.rtt = this.rtt === null ? sample : this.rtt + (sample - this.rtt) * 0.3
  }
}
