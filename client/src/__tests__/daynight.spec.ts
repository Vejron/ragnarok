import { describe, expect, it } from 'vitest'
import { Color } from '@ragnarok/engine'
import { DAY_SECONDS, dayPhase, skyAt } from '@/render/daynight'
import { CLASSIC, SOFT } from '@/render/look'

describe('day and night', () => {
  it('is the palette exactly at noon', () => {
    for (const palette of [CLASSIC, SOFT]) {
      const sky = skyAt(0.5, palette)
      expect(sky.sky.getHex()).toBe(palette.sky)
      expect(sky.hemiSky.getHex()).toBe(palette.hemiSky)
      expect(sky.hemiGround.getHex()).toBe(palette.hemiGround)
      expect(sky.lightColor.getHex()).toBe(palette.sun)
      expect(sky.lightIntensity).toBeCloseTo(palette.sunIntensity)
      expect(sky.hemiIntensity).toBeCloseTo(palette.hemiIntensity)
    }
  })

  it('keeps the light above the horizon all day, and darker at night', () => {
    for (let i = 0; i < 400; i++) {
      const sky = skyAt(i / 400, SOFT)
      expect(sky.lightDir[1]).toBeGreaterThanOrEqual(0)
      // Only a faint light skims the horizon.
      if (sky.lightDir[1] < 0.05) expect(sky.lightIntensity).toBeLessThan(0.5)
      expect(Math.hypot(...sky.lightDir)).toBeCloseTo(1)
    }
    expect(skyAt(0, SOFT).lightIntensity).toBeLessThan(SOFT.sunIntensity / 2)
    expect(skyAt(0, SOFT).hemiIntensity).toBeLessThan(SOFT.hemiIntensity / 2)
  })

  it('changes the light smoothly through sunrise and sunset', () => {
    const brightness = (c: Color) => c.r + c.g + c.b
    for (let i = 0; i < 2000; i++) {
      const a = skyAt(i / 2000, CLASSIC)
      const light = a.lightIntensity
      const sky = brightness(a.sky)
      const b = skyAt((i + 1) / 2000, CLASSIC)
      expect(Math.abs(b.lightIntensity - light)).toBeLessThan(0.05)
      expect(Math.abs(brightness(b.sky) - sky)).toBeLessThan(0.05)
    }
  })

  it('takes a whole cycle of server time', () => {
    const rate = 60
    expect(dayPhase(0, rate)).toBeCloseTo(dayPhase(DAY_SECONDS * rate, rate))
    expect(dayPhase(DAY_SECONDS * rate * 0.5, rate)).not.toBeCloseTo(dayPhase(0, rate))
  })
})
