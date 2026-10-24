import { describe, expect, it } from 'vitest'
import { STAMINA_FULL } from '@/sim/consts'
import { SWAY_MAX, easeSway, sway, swayAmplitude } from '@/sway'

describe('aim sway', () => {
  it('is still when rested and widest out of breath', () => {
    expect(swayAmplitude(0, false)).toBe(0)
    expect(swayAmplitude(STAMINA_FULL / 2, false)).toBeCloseTo(SWAY_MAX / 4)
    expect(swayAmplitude(STAMINA_FULL, false)).toBeCloseTo(SWAY_MAX)
    expect(swayAmplitude(STAMINA_FULL, true)).toBeCloseTo(SWAY_MAX * 2)
  })

  it('stays within its amplitude and moves smoothly', () => {
    const amplitude = SWAY_MAX * 2
    let last = sway(0, amplitude)
    for (let t = 0; t < 30; t += 1 / 120) {
      const now = sway(t, amplitude)
      expect(Math.abs(now[0])).toBeLessThanOrEqual(amplitude)
      expect(Math.abs(now[1])).toBeLessThanOrEqual(amplitude)
      expect(Math.abs(now[0] - last[0])).toBeLessThan(amplitude * 0.05)
      expect(Math.abs(now[1] - last[1])).toBeLessThan(amplitude * 0.05)
      last = now
    }
    expect(sway(12.3, 0).map(Math.abs)).toEqual([0, 0])
  })

  it('eases towards a new size rather than jumping', () => {
    const next = easeSway(0, SWAY_MAX, 1 / 60)
    expect(next).toBeGreaterThan(0)
    expect(next).toBeLessThan(SWAY_MAX * 0.1)
    expect(easeSway(0, SWAY_MAX, 10)).toBe(SWAY_MAX)
  })
})
