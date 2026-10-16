const UNITS = ['B', 'KB', 'MB', 'GB', 'TB']

/** Binary units, as the importer's cache reports them: 1536 → "1.5 KB", 312 MiB → "312 MB". */
export function formatBytes(bytes: number): string {
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024
    unit++
  }
  const digits = unit === 0 || value >= 10 ? 0 : 1
  return `${value.toFixed(digits)} ${UNITS[unit]}`
}

/** A rate in megabytes a second and in megabits (the network's unit): "2.4 MB/s · 19 Mbit/s". */
export function formatRate(bytesPerSecond: number): string {
  if (bytesPerSecond < 1) return 'idle'
  const megabits = (bytesPerSecond * 8) / 1_000_000
  return `${formatBytes(bytesPerSecond)}/s · ${megabits.toFixed(megabits < 10 ? 1 : 0)} Mbit/s`
}

/** A clock: "0:42", "12:05", "1:02:03". */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000))
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const seconds = String(total % 60).padStart(2, '0')
  return hours ? `${hours}:${String(minutes).padStart(2, '0')}:${seconds}` : `${minutes}:${seconds}`
}

/** Time left at the pace so far: `done` of `total` took `elapsedMs`. Zero before the first one is done. */
export function remaining(elapsedMs: number, done: number, total: number): number {
  return done > 0 ? (elapsedMs / done) * Math.max(0, total - done) : 0
}

/** A width for a progress bar, `done` of `total`. */
export function percent(done: number, total: number): string {
  return `${total > 0 ? Math.min(100, (100 * done) / total) : 0}%`
}

/**
 * The width of a stage's bar: its steps when it counts them, full once the job is done, empty when
 * it failed. A running stage that does not count has no width, so the bar's busy animation shows.
 */
export function stageWidth(step: number, steps: number, state: string): string | undefined {
  if (steps > 0) return percent(step, steps)
  if (state === 'running') return undefined
  return state === 'done' ? '100%' : '0%'
}
