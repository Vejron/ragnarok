import { describe, expect, it } from 'vitest'
import { circleOnSmall, insideWorld, mapToWorld, PIP_INSET, viewWindow, worldToMap } from '@/sim/minimap'

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

  it('draws the backpack\'s circle around where it is on the small map', () => {
    // 600 m across 180 px: 0.3 px a metre.
    const near = circleOnSmall(30, -60, 75, 0, 0, 600, 180)
    expect(near.left).toBeCloseTo(99)
    expect(near.top).toBeCloseTo(72)
    expect(near.r).toBeCloseTo(22.5)
    expect(near.pip).toBeNull()
    // Its middle out of view but its edge in it: still drawn, no pip.
    expect(circleOnSmall(340, 0, 75, 0, 0, 600, 180).pip).toBeNull()
  })

  it('points to the circle from the edge when none of it is in view', () => {
    const east = circleOnSmall(1000, 0, 10, 0, 0, 600, 180)
    expect(east.pip!.left).toBeCloseTo(180 - PIP_INSET)
    expect(east.pip!.top).toBeCloseTo(90)
    const corner = circleOnSmall(-800, -800, 10, 0, 0, 600, 180)
    expect(corner.pip!.left).toBeCloseTo(PIP_INSET)
    expect(corner.pip!.top).toBeCloseTo(PIP_INSET)
  })

  it('maps image fractions back to the world', () => {
    expect(mapToWorld(0.5, 0.5, 4300)).toEqual([0, 0])
    expect(mapToWorld(...worldToMap(-1200, 3000, 4300), 4300)).toEqual([-1200, 3000])
  })

  it('knows which points are in the world', () => {
    // A 2 × 2 grid of 100 m tiles; only the top-left and bottom-right belong.
    const world = { halfExtent: 100, tileSize: 100, inside: [true, false, false, true] }
    expect(insideWorld(-50, -50, world)).toBe(true)
    expect(insideWorld(50, -50, world)).toBe(false)
    expect(insideWorld(50, 50, world)).toBe(true)
    expect(insideWorld(101, 0, world)).toBe(false)
    expect(insideWorld(50, -50, { ...world, inside: null })).toBe(true)
  })
})
