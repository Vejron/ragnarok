import { describe, expect, it } from 'vitest'
import { coverMask } from '@/render/marshnoise'
import { GARDEN_SHADE, grassMask } from '@/render/grassmask'
import { hasRock } from '@/render/stones'
import { APRON_WIDTH, layerNoise } from '@/render/terrainmesh'
import { SURFACE_CELL } from '@/sim/surface'
import { COVER_GARDEN, COVER_ROCK, COVER_WOOD, type Point2 } from '@/world'

const square = (x0: number, z0: number, x1: number, z1: number): Point2[] => [[x0, z0], [x1, z0], [x1, z1], [x0, z1]]
const rock = (cover: number) => (cover === COVER_ROCK ? 1 : 0)

describe('ground cover', () => {
  it('a painted layer repeats at its edges and keeps about the middle', () => {
    const pixels = 64
    const noise = layerNoise(8, 3, pixels)
    const mean = noise.reduce((a, b) => a + b) / noise.length
    expect(mean).toBeGreaterThan(0.4)
    expect(mean).toBeLessThan(0.6)
    // The texel past the last one is the first again: the step over the edge is as small as any inside.
    let inside = 0, over = 0
    for (let row = 0; row < pixels; row++) {
      for (let column = 1; column < pixels; column++) inside = Math.max(inside, Math.abs(noise[row * pixels + column]! - noise[row * pixels + column - 1]!))
      over = Math.max(over, Math.abs(noise[row * pixels]! - noise[row * pixels + pixels - 1]!))
    }
    expect(over).toBeLessThanOrEqual(inside)
    expect(layerNoise(8, 3, pixels)).toEqual(noise)
  })

  it('bare rock is masked for the shader where it lies, and not under a cover painted over it', () => {
    const tile = { cover: [{ kind: COVER_ROCK, rings: [square(0, 0, 64, 64)] }, { kind: COVER_WOOD, rings: [square(32, 0, 64, 64)] }] }
    expect(hasRock(tile)).toBe(true)
    const mask = coverMask(tile, 0, 0, 128, 64, rock)!
    const at = (x: number, z: number) => mask[Math.floor(z / 2) * 64 + Math.floor(x / 2)]!
    expect(at(10, 30)).toBe(255)
    expect(at(50, 30)).toBe(0)
    expect(at(100, 100)).toBe(0)
    expect(coverMask({ cover: [{ kind: COVER_WOOD, rings: [square(0, 0, 64, 64)] }] }, 0, 0, 128, 64, rock)).toBeNull()
    expect(hasRock({ cover: [] })).toBe(false)
  })

  it('a garden is lawn up to the gravel round its house', () => {
    const tile = { cover: [{ kind: COVER_GARDEN, rings: [square(0, 0, 64, 64)] }], roads: [], water: [], streams: [], buildings: [{ footprint: square(24, 24, 40, 40) }] }
    const mask = grassMask(tile as never, 0, 0, 64)
    const at = (x: number, z: number) => mask.cells[Math.floor(z / SURFACE_CELL) * mask.side + Math.floor(x / SURFACE_CELL)]!
    expect(at(5, 5)).toBe(GARDEN_SHADE)
    expect(at(32, 32)).toBe(0)
    // A cell whose centre is on the gravel, just outside the wall.
    expect(at(24 - APRON_WIDTH / 2, 32)).toBe(0)
    expect(at(24 - APRON_WIDTH - SURFACE_CELL * 1.5, 32)).toBe(GARDEN_SHADE)
  })
})
