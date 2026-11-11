import { describe, expect, it } from 'vitest'
import { Geometry } from '../src/scene/geometry'
import { levelAt, type DetailLevel } from '../src/scene/mesh'

const levels: DetailLevel[] = [
  { geometry: new Geometry({ positions: new Float32Array(9) }), from: 50 },
  { geometry: new Geometry({ positions: new Float32Array(9) }), from: 160 },
]

describe('levelAt', () => {
  it('draws the mesh\'s own geometry up to the first level\'s distance, then each level from its own', () => {
    expect(levelAt(0, levels)).toBe(0)
    expect(levelAt(49 * 49, levels)).toBe(0)
    expect(levelAt(50 * 50, levels)).toBe(1)
    expect(levelAt(159 * 159, levels)).toBe(1)
    expect(levelAt(160 * 160, levels)).toBe(2)
    expect(levelAt(1e9, levels)).toBe(2)
  })

  it('brings the coarser levels nearer when the detail is turned down', () => {
    expect(levelAt(30 * 30, levels, 0.5)).toBe(1)
    expect(levelAt(24 * 24, levels, 0.5)).toBe(0)
    expect(levelAt(80 * 80, levels, 0.5)).toBe(2)
  })

  it('is always the mesh\'s own geometry when there are no levels', () => {
    expect(levelAt(1e9, [])).toBe(0)
  })
})
