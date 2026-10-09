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
})
