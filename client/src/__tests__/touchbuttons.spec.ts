import { describe, expect, it } from 'vitest'
import { touchButtons } from '@/touchbuttons'
import type { HudStats } from '@/hud'
import { BUTTON_ALT, BUTTON_AMMO } from '@/sim/consts'

const SEATS: HudStats['seat'][] = [null, 'driver', 'roadster', 'sled', 'helm', 'pilot', 'aviator', 'walker', 'hen', 'tupp', 'strv', 'mortar', 'drone', 'loiter', 'passenger', 'gunner', 'roofgunner']

describe('touch buttons', () => {
  it('gives every button a cell of its own, in every seat', () => {
    for (const seat of SEATS) {
      for (const context of [{ canSwitch: true, winter: true, ammo: true }, { canSwitch: false, winter: false, ammo: false }]) {
        const cells = touchButtons(seat, context).map((b) => `${b.col},${b.row}`)
        expect(new Set(cells).size, `${seat}`).toBe(cells.length)
      }
    }
  })

  it('lets a touch player aim on foot and at a gun, and swap rounds where there are two', () => {
    const off = { canSwitch: false, winter: false, ammo: false }
    expect(touchButtons(null, off).some((b) => b.mask === BUTTON_ALT && b.mode === 'hold')).toBe(true)
    expect(touchButtons('gunner', off).some((b) => b.mask === BUTTON_ALT)).toBe(true)
    expect(touchButtons('strv', { ...off, ammo: true }).some((b) => b.mask === BUTTON_AMMO)).toBe(true)
    expect(touchButtons('driver', off).some((b) => b.mask === BUTTON_AMMO)).toBe(false)
  })

  it('shows skis only in winter, and a seat switch only with a free seat', () => {
    expect(touchButtons(null, { canSwitch: false, winter: true, ammo: false }).some((b) => b.id === 'skis')).toBe(true)
    expect(touchButtons(null, { canSwitch: false, winter: false, ammo: false }).some((b) => b.id === 'skis')).toBe(false)
    expect(touchButtons('driver', { canSwitch: true, winter: false, ammo: false }).some((b) => b.id === 'seat')).toBe(true)
    expect(touchButtons('driver', { canSwitch: false, winter: false, ammo: false }).some((b) => b.id === 'seat')).toBe(false)
  })
})
