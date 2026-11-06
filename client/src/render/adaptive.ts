/** Frame intervals kept for guessing the display's refresh. */
const HISTORY = 120
/** Frames are judged in windows this long, ms. */
const WINDOW_MS = 500
/** Slower than the display by this factor is slow... */
const SLOW = 1.2
/** ...and faster than this is fast enough to try a sharper picture. */
const FAST = 1.05
/** Steps of the render scale, down and up. */
const STEP_DOWN = 0.1
const STEP_UP = 0.05
/** Least time between steps, ms. */
const COOLDOWN_MS = 1000
/** Fast for this long before a step up, ms; doubled each time a step up had to be taken back. */
const UP_AFTER_MS = 3000
const UP_AFTER_MAX_MS = 60000
/** A step down this soon after a step up takes it back. */
const BOUNCE_MS = 5000
/** An interval this long is a hitch or a hidden tab, not the frame rate, ms. */
const HITCH_MS = 250
/** The GPU is the bottleneck when its time is at least this share of the frame. */
const GPU_BOUND = 0.8
/**
 * How far each window's guess at the display's interval moves the kept one when it is slower: a
 * little, so a spell of slow frames is not taken for a slow display, yet a device that never goes
 * faster settles in a minute or so. A quicker guess is taken at once.
 */
const REFRESH_DRIFT = 0.01

/**
 * Picks a render scale from how long frames take: lower while they are slower than the display, higher
 * again after a while at full speed. The display's own interval is guessed from the quickest recent
 * frames seen, so a 120 Hz screen aims at 120. Where the GPU's time is known and it is not the bottleneck,
 * fewer pixels would not help, so the scale stays.
 */
export class AdaptiveScale {
  scale = 1
  private readonly intervals: number[] = []
  private sum = 0
  private gpuSum = 0
  private gpuCount = 0
  private count = 0
  private windowFrom = 0
  private changedAt = -Infinity
  private upAt = -Infinity
  private fastSince: number | null = null
  private upAfter = UP_AFTER_MS
  private pausedUntil = 0
  private refresh = Infinity

  constructor(public floor = 0.7) {}

  /** Ignores frames for a while: after a resize, a quality change or a load, which stall a frame or two. */
  pause(now: number, ms = 2000) {
    this.pausedUntil = Math.max(this.pausedUntil, now + ms)
    this.resetWindow(now)
    this.fastSince = null
  }

  /** Starts again at full scale with `floor`, as after a change of quality. */
  reset(now: number, floor = this.floor) {
    this.floor = floor
    this.scale = 1
    this.upAfter = UP_AFTER_MS
    this.changedAt = -Infinity
    this.pause(now)
  }

  /**
   * One frame: `interval` since the last, ms, and the GPU's time for it if known. Returns whether
   * `scale` changed.
   */
  frame(now: number, interval: number, gpuMs?: number): boolean {
    if (interval <= 0 || interval > HITCH_MS || now < this.pausedUntil) {
      if (interval > HITCH_MS) this.pause(now, 500)
      return false
    }
    this.intervals.push(interval)
    if (this.intervals.length > HISTORY) this.intervals.shift()
    this.sum += interval
    this.count++
    if (gpuMs !== undefined && gpuMs > 0) {
      this.gpuSum += gpuMs
      this.gpuCount++
    }
    if (now - this.windowFrom < WINDOW_MS) return false
    const average = this.sum / this.count
    const gpu = this.gpuCount ? this.gpuSum / this.gpuCount : undefined
    this.resetWindow(now)
    const display = this.display()
    if (display === null) return false
    const settled = now - this.changedAt >= COOLDOWN_MS

    if (average > display * SLOW) {
      this.fastSince = null
      if (!settled || this.scale <= this.floor) return false
      if (gpu !== undefined && gpu < average * GPU_BOUND) return false
      if (now - this.upAt < BOUNCE_MS) this.upAfter = Math.min(this.upAfter * 2, UP_AFTER_MAX_MS)
      return this.set(Math.max(this.floor, this.scale - STEP_DOWN), now)
    }
    if (average < display * FAST) {
      this.fastSince ??= now
      if (this.scale < 1 && settled && now - this.fastSince >= this.upAfter) {
        this.fastSince = now
        this.upAt = now
        return this.set(Math.min(1, this.scale + STEP_UP), now)
      }
      return false
    }
    this.fastSince = null
    return false
  }

  /** The display's frame interval, from the quickest tenth of recent frames; null until there are enough. */
  private display(): number | null {
    if (this.intervals.length < 30) return null
    const sorted = [...this.intervals].sort((a, b) => a - b)
    const guess = sorted[Math.floor(sorted.length * 0.1)]!
    this.refresh = guess < this.refresh ? guess : this.refresh + (guess - this.refresh) * REFRESH_DRIFT
    return this.refresh
  }

  private set(scale: number, now: number) {
    scale = Math.round(scale * 100) / 100
    if (scale === this.scale) return false
    this.scale = scale
    this.changedAt = now
    return true
  }

  private resetWindow(now: number) {
    this.windowFrom = now
    this.sum = 0
    this.count = 0
    this.gpuSum = 0
    this.gpuCount = 0
  }
}
