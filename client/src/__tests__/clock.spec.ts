import { describe, expect, it } from 'vitest'
import { ServerClock } from '@/net/clock'
import { SNAPSHOT_EVERY, TICK_MS } from '@/sim/consts'

describe('ServerClock', () => {
  it('tracks a steady stream and keeps the minimum delay', () => {
    const clock = new ServerClock()
    for (let tick = 100; tick < 400; tick += SNAPSHOT_EVERY) clock.observe(tick, 5000 + (tick - 100) * TICK_MS)
    expect(clock.tickAt(5000 + 300 * TICK_MS)).toBeCloseTo(400, 0)
    expect(clock.interpDelay).toBeCloseTo(SNAPSHOT_EVERY * 2, 0)
  })

  it('widens the delay under jitter', () => {
    const clock = new ServerClock()
    let seed = 1
    const random = () => ((seed = (seed * 16807) % 2147483647) / 2147483647)
    for (let tick = 0; tick < 600; tick += SNAPSHOT_EVERY) clock.observe(tick, tick * TICK_MS + random() * 80)
    expect(clock.interpDelay).toBeGreaterThan(SNAPSHOT_EVERY * 2 + 1)
  })

  it('resyncs after a large jump', () => {
    const clock = new ServerClock()
    clock.observe(100, 0)
    clock.observe(10_000, 10)
    expect(clock.tickAt(10)).toBeCloseTo(10_000, 0)
  })

  it('follows a server that ticks slower than nominal instead of widening the delay without bound', () => {
    const clock = new ServerClock()
    // A loaded server managing 50 of its 60 ticks a second: snapshots arrive every 40 ms, not 33.
    const interval = (SNAPSHOT_EVERY / 50) * 1000
    let ms = 0
    for (let tick = 0; tick < 60 * 40; tick += SNAPSHOT_EVERY, ms += interval) clock.observe(tick, ms)
    expect(clock.interpDelay).toBeLessThanOrEqual(SNAPSHOT_EVERY * 2 + 5)
  })

  it('recovers its minimum delay after a stall on the client', () => {
    const clock = new ServerClock()
    let tick = 0
    let ms = 0
    for (; tick < 600; tick += SNAPSHOT_EVERY) clock.observe(tick, (ms = tick * TICK_MS))
    // The main thread hangs for 400 ms; the queued snapshots are all handled at the end of it.
    ms += 400
    for (let i = 0; i < 12; i++, tick += SNAPSHOT_EVERY) clock.observe(tick, ms)
    for (let i = 0; i < 600; i++, tick += SNAPSHOT_EVERY) clock.observe(tick, (ms += SNAPSHOT_EVERY * TICK_MS))
    expect(clock.interpDelay).toBeLessThan(SNAPSHOT_EVERY * 2 + 1)
  })
})
