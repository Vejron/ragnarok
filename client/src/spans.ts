/*
 * Named spans of main-thread time for `scripts/frame-bench.mjs`, kept only when the page was loaded
 * with `?bench`; otherwise each helper is a plain call or a constant. This module imports nothing, so
 * anything may time itself with it, the modules a worker shares included (a worker's `location` has no
 * `?bench`, so it never times anything).
 */

/** Whether the page was loaded with `?bench`. */
export const benching = typeof location !== 'undefined' && new URLSearchParams(location.search).has('bench')

/** The time in each span in the current frame, and between frames (fetches, worker replies), by name, ms. */
const inFrame = new Map<string, number>()
const outside = new Map<string, number>()
/** Whether a frame's callback is running: spans then count to the frame, else to the time between. */
let framing = false

/** Adds `ms` to span `name`, in the frame or between frames. */
function addTo(name: string, ms: number) {
  const spans = framing ? inFrame : outside
  spans.set(name, (spans.get(name) ?? 0) + ms)
}

/** Runs `fn` and, under `?bench`, adds the time it took to span `name`. */
export function span<T>(name: string, fn: () => T): T {
  if (!benching) return fn()
  const start = performance.now()
  try {
    return fn()
  } finally {
    addTo(name, performance.now() - start)
  }
}

/** The time now under `?bench`, ms, for `lap` to measure from; 0 otherwise. */
export function clock(): number {
  return benching ? performance.now() : 0
}

/**
 * Under `?bench`, adds the time since `since` (a `clock` or an earlier `lap`) to span `name` and returns
 * the time now, so a run of steps is timed one after another: `t = lap('a', t); …; t = lap('b', t)`.
 */
export function lap(name: string, since: number): number {
  if (!benching) return 0
  const now = performance.now()
  addTo(name, now - since)
  return now
}

/** A frame's callback starts: spans from here count to it. */
export function frameBegins() {
  framing = benching
}

/**
 * A frame's callback ends: returns the time in each span within it and in each between it and the
 * frame before, and starts both afresh.
 */
export function frameEnds(): { spans: Record<string, number>; between: Record<string, number> } {
  framing = false
  const taken = { spans: Object.fromEntries(inFrame), between: Object.fromEntries(outside) }
  inFrame.clear()
  outside.clear()
  return taken
}
