import { describe, expect, it } from 'vitest'
import { Color } from '@ragnarok/engine'
import { tinted } from '@/render/groundlight'
import { skyAt } from '@/render/daynight'
import { CLASSIC } from '@/render/look'
import { NIGHT_FLOOR } from '@/render/particles'

describe('tinted', () => {
  it('leaves daylight colours alone and darkens and blues them at night', () => {
    expect(tinted(0xcbc2b0, new Color(1, 1, 1))).toBe(0xcbc2b0)
    const night = Color.hex(tinted(0xcbc2b0, skyAt(0, CLASSIC).tint, NIGHT_FLOOR))
    const day = Color.hex(0xcbc2b0)
    expect(night.r).toBeLessThan(day.r * 0.2)
    expect(night.b / day.b).toBeGreaterThan(night.r / day.r)
  })

  it('never goes below its floor', () => {
    const black = Color.hex(tinted(0xffffff, new Color(0, 0, 0), 0.1))
    expect(black.r).toBeCloseTo(0.1, 2)
  })
})
