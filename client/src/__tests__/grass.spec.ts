import { describe, expect, it } from 'vitest'
import { bladeGeometry, chunkBlades, GRASS_CHUNK, GRASS_FULL, grassMask, keptAt, patchAt, tierFor } from '@/render/grass'
import { COVER_PARK, COVER_URBAN, COVER_WATER, ROAD_MINOR, type Building, type Point2 } from '@/world'

const square = (x0: number, z0: number, x1: number, z1: number): Point2[] => [[x0, z0], [x1, z0], [x1, z1], [x0, z1]]

describe('grass', () => {
  const tile = {
    cover: [
      { kind: COVER_WATER, rings: [square(60, 60, 80, 80)] },
      { kind: COVER_URBAN, rings: [square(0, 80, 40, 100)] },
      { kind: COVER_PARK, rings: [square(0, 0, 40, 40)] },
    ],
    roads: [{ kind: ROAD_MINOR, width: 6, points: [[0, 50], [100, 50]] as Point2[] }],
    water: new Float32Array(),
    buildings: [{ footprint: square(10, 10, 20, 20) } as Building],
    streams: [{ width: 2, points: [[90, 0], [90, 40]] as Point2[] }],
  }
  const mask = grassMask(tile, 0, 0, 100)
  const at = (x: number, z: number) => mask.cells[Math.floor(z / 2) * mask.side + Math.floor(x / 2)]

  it('grows on green ground, and not on roads, water, streams, built-up land or under buildings', () => {
    expect(at(95, 95)).toBe(1)
    expect(at(30, 30)).toBe(2)
    expect(at(15, 15)).toBe(0)
    expect(at(50, 50)).toBe(0)
    expect(at(70, 70)).toBe(0)
    expect(at(20, 90)).toBe(0)
    expect(at(90, 20)).toBe(0)
  })

  it('grows the same blades in a chunk every time, a smaller share the first blades of a larger one', () => {
    const everywhere = () => 1
    const flat = () => 0
    const a = chunkBlades(3, -2, 8, 1, everywhere, flat)
    expect(a.length).toBeGreaterThan(0)
    expect(chunkBlades(3, -2, 8, 1, everywhere, flat)).toEqual(a)
    expect(chunkBlades(3, -2, 8, 0.25, everywhere, flat)).toEqual(a.filter((b) => b.rank < 0.25))
    for (const blade of a) {
      expect(blade.x).toBeGreaterThanOrEqual(3 * GRASS_CHUNK)
      expect(blade.x).toBeLessThan(4 * GRASS_CHUNK)
      expect(blade.z).toBeGreaterThanOrEqual(-2 * GRASS_CHUNK)
      expect(blade.z).toBeLessThan(-1 * GRASS_CHUNK)
    }
  })

  it('skips blades where the mask is bare or the ground is not loaded', () => {
    const blades = chunkBlades(0, 0, 40, 1, (x) => (x < 8 ? 1 : 0), (x, z) => (z < 8 ? 2 : null))
    expect(blades.length).toBeGreaterThan(0)
    for (const blade of blades) {
      expect(blade.x).toBeLessThan(8)
      expect(blade.z).toBeLessThan(8)
      expect(blade.y).toBe(2)
    }
  })

  it('builds each blade about its root, with its height above it in custom', () => {
    const blades = chunkBlades(0, 0, 1, 1, () => 1, () => 1)
    const geometry = bladeGeometry(blades)
    expect(geometry.vertexCount).toBe(blades.length * 5)
    const tip = 4
    expect(geometry.custom![tip * 4]).toBeCloseTo(blades[0]!.x)
    expect(geometry.custom![tip * 4 + 2]).toBeCloseTo(blades[0]!.height)
    expect(geometry.positions[tip * 3 + 1]).toBeCloseTo(1 + blades[0]!.height)
  })

  it('grows in patches, with bare ground between', () => {
    let thick = 0, bare = 0
    for (let x = 0; x < 400; x += 2) for (let z = 0; z < 400; z += 2) {
      const p = patchAt(x, z)
      if (p > 0.9) thick++
      if (p < 0.05) bare++
    }
    expect(thick).toBeGreaterThan(2000)
    expect(bare).toBeGreaterThan(5000)
  })

  it('holds enough blades in a chunk for what its distance shows', () => {
    for (const d of [0, 10, 22, 30, 50, 80, 110]) {
      expect(tierFor(GRASS_FULL, d)).toBeGreaterThanOrEqual(keptAt(GRASS_FULL, d))
      expect(tierFor(GRASS_FULL, d)).toBeLessThanOrEqual(1)
    }
    expect(tierFor(GRASS_FULL, 5)).toBe(1)
    expect(tierFor(GRASS_FULL, 100)).toBeLessThan(0.1)
  })
})
