import { describe, expect, it } from 'vitest'
import { viewWindow, worldToMap } from '@/sim/minimap'

describe('minimap', () => {
  it('maps the world corners and centre onto the image', () => {
    expect(worldToMap(0, 0, 4300)).toEqual([0.5, 0.5])
    expect(worldToMap(-4300, -4300, 4300)).toEqual([0, 0])
    expect(worldToMap(4300, 4300, 4300)).toEqual([1, 1])
  })

  it('centres the view on the player', () => {
    // 1 km across 200 px over an 8.6 km world: the image is 1720 px wide.
    const w = viewWindow(0, 0, 4300, 1000, 200)
    expect(w.size).toBeCloseTo(1720)
    expect(w.left).toBeCloseTo(100 - 860)
    expect(w.top).toBeCloseTo(100 - 860)
  })
})
