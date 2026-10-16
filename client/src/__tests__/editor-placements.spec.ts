import { describe, expect, it } from 'vitest'
import { headingOf, nearest, yawOf } from '../editor/placements'

describe('editor placements', () => {
  it('turns a compass heading into the simulation yaw and back', () => {
    expect(yawOf(0)).toBe(0)
    expect(yawOf(2)).toBeCloseTo(-Math.PI / 2) // east is a right turn from north
    for (let h = 0; h < 8; h++) expect(headingOf(yawOf(h))).toBe(h)
  })

  it('picks the nearest point within reach', () => {
    const points = [{ x: 0, y: 0 }, { x: 10, y: 0 }]
    expect(nearest(points, 9, 1, 5)).toBe(1)
    expect(nearest(points, 50, 50, 5)).toBe(-1)
  })
})
