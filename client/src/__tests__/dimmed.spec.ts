import { describe, expect, it } from 'vitest'
import { dimmed } from '@/render/particles'

describe('dimmed', () => {
  it('leaves daylight colours alone and darkens them at night', () => {
    expect(dimmed(0xcbc2b0, 1)).toBe(0xcbc2b0)
    const night = dimmed(0xcbc2b0, 0)
    expect((night >> 16) & 0xff).toBeLessThan(0x80)
    expect(night & 0xff).toBeLessThan(0xb0 * 0.4)
  })
})
