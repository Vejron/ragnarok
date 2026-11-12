import { describe, expect, it } from 'vitest'
import { weatherAt } from '@/render/weather'

describe('weather', () => {
  it('is the same for everyone at the same moment', () => {
    for (const t of [0, 1234.5, 98765]) expect(weatherAt(t, false)).toEqual(weatherAt(t, false))
  })

  it('rains now and then, not most of the time, and is wet whenever it rains', () => {
    let raining = 0
    const samples = 20000
    for (let i = 0; i < samples; i++) {
      const w = weatherAt(i * 37, false)
      if (w.rain > 0.1) {
        raining++
        expect(w.wet).toBeGreaterThan(0)
        expect(w.cover).toBeGreaterThan(0.7)
      }
    }
    expect(raining / samples).toBeGreaterThan(0.03)
    expect(raining / samples).toBeLessThan(0.3)
  })

  it('flashes with lightning only in heavy rain, and shows rainbows only out of it', () => {
    let flashes = 0
    let rainbows = 0
    for (let i = 0; i < 400000; i++) {
      const w = weatherAt(i * 0.9, false)
      if (w.flash > 0.01) {
        flashes++
        expect(w.rain).toBeGreaterThan(0.6)
      }
      if (w.rainbow > 0.01) {
        rainbows++
        expect(w.rain).toBeLessThan(0.3)
      }
    }
    expect(flashes).toBeGreaterThan(0)
    expect(rainbows).toBeGreaterThan(0)
  })

  it('never rains on a winter world, under a heavier sky', () => {
    for (let i = 0; i < 5000; i++) {
      const w = weatherAt(i * 61, true)
      expect(w.rain).toBe(0)
      expect(w.flash).toBe(0)
      expect(w.cover).toBeGreaterThanOrEqual(0.6)
    }
  })

  it('changes smoothly, apart from the lightning', () => {
    for (let i = 0; i < 20000; i++) {
      const a = weatherAt(i * 5, false)
      const b = weatherAt(i * 5 + 1, false)
      expect(Math.abs(a.cover - b.cover)).toBeLessThan(0.01)
      expect(Math.abs(a.rain - b.rain)).toBeLessThan(0.02)
    }
  })
})
