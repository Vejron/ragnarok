import { describe, expect, it } from 'vitest'
import { Geometry } from '../src/scene/geometry'
import { FADE_MARGIN, fadingAt, levelAt, type DetailLevel } from '../src/scene/mesh'

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

describe('fadingAt', () => {
  const fading: DetailLevel[] = [
    { geometry: new Geometry({ positions: new Float32Array(9) }), from: 40, fade: 10 },
    { geometry: new Geometry({ positions: new Float32Array(9) }), from: 130 },
  ]

  it('draws an instance in a fade band at both levels, and anywhere else at its own alone', () => {
    expect(fadingAt(20 * 20, fading)).toBe(-1)
    expect(fadingAt(29 * 29, fading)).toBe(-1)
    expect(fadingAt(31 * 31, fading)).toBe(1)
    expect(fadingAt(39.9 * 39.9, fading)).toBe(1)
    // Just past the change the solid is still listed, by the margin, so the two never both miss a pixel.
    expect(fadingAt((40 + FADE_MARGIN / 2) ** 2, fading)).toBe(0)
    expect(fadingAt(42 * 42, fading)).toBe(-1)
    // The change without a band is instant.
    expect(fadingAt(129 * 129, fading)).toBe(-1)
    expect(fadingAt(131 * 131, fading)).toBe(-1)
  })

  it('brings the band nearer with the detail', () => {
    expect(fadingAt(16 * 16, fading, 0.5)).toBe(1)
    expect(fadingAt(14 * 14, fading, 0.5)).toBe(-1)
  })
})
