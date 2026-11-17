import { describe, expect, it } from 'vitest'
import { bladeGeometry, chunkBlades, cropMask, GRASS_CHUNK, GRASS_FULL, grassMask, keptAt, patchAt, STUBBLE, tierFor, WHEAT, WHEAT_DENSITY } from '@/render/grass'
import { CROP_HEIGHT } from '@/sim/consts.generated'
import { COVER_PARK, COVER_URBAN, COVER_WATER, CROP_NONE, CROP_WHEAT, ROAD_MINOR, type Building, type Point2 } from '@/world'

const square = (x0: number, z0: number, x1: number, z1: number): Point2[] => [[x0, z0], [x1, z0], [x1, z1], [x0, z1]]

describe('grass', () => {
  const tile = {
    cover: [
      { kind: COVER_WATER, rings: [square(60, 60, 80, 80)] },
      { kind: COVER_URBAN, rings: [square(0, 80, 40, 100)] },
      { kind: COVER_PARK, rings: [square(0, 0, 40, 40)] },
    ],
    roads: [{ kind: ROAD_MINOR, width: 6, carried: false, points: [[0, 50], [100, 50]] as Point2[] }],
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

  it('grows wheat evenly over a field, every stalk about as high as the crop and with an ear', () => {
    const flat = () => 0
    const wheat = chunkBlades(1, 1, WHEAT_DENSITY.perMetre, 1, () => 1, flat, WHEAT)
    // No bare patches: every draw is a stalk.
    expect(wheat.length).toBe(WHEAT_DENSITY.perMetre * GRASS_CHUNK * GRASS_CHUNK)
    for (const stalk of wheat) {
      expect(stalk.height).toBeGreaterThanOrEqual(CROP_HEIGHT * 0.9 - 1e-6)
      expect(stalk.height).toBeLessThanOrEqual(CROP_HEIGHT * 1.1 + 1e-6)
      expect(stalk.head).toBe(true)
    }
    // A smaller share is still the first stalks of the whole field, as with grass.
    expect(chunkBlades(1, 1, WHEAT_DENSITY.perMetre, 0.25, () => 1, flat, WHEAT)).toEqual(wheat.filter((stalk) => stalk.rank < 0.25))
    // Twice as wide as a blade of grass, and the ear is the ear's colour.
    const one = wheat.slice(0, 1)
    const stalk = bladeGeometry(one, false, WHEAT), blade = bladeGeometry(one, false)
    const width = (g: typeof stalk) => Math.hypot(g.positions[3]! - g.positions[0]!, g.positions[5]! - g.positions[2]!)
    expect(width(stalk)).toBeGreaterThan(width(blade) * 1.4)
    const tip = 4
    expect(stalk.colors![tip * 4]! / stalk.colors![0]!).toBeGreaterThan(1)
  })

  it('leaves a winter field short stubble, thinner than the wheat was', () => {
    const flat = () => 0
    const stubble = chunkBlades(1, 1, 8, 1, () => 1, flat, STUBBLE)
    expect(stubble.length).toBeGreaterThan(0)
    expect(stubble.length).toBeLessThan(8 * GRASS_CHUNK * GRASS_CHUNK)
    for (const stalk of stubble) expect(stalk.height).toBeLessThan(0.3)
  })

  it('masks the cells of a tile where crops stand, and none of a tile without', () => {
    // A 8 m tile of 4 × 4 crop cells, wheat in its western half.
    const crops = Uint8Array.from({ length: 16 }, (_, k) => (k % 4 < 2 ? CROP_WHEAT : CROP_NONE))
    const mask = cropMask(crops, 100, 200, 8)
    expect([mask.x0, mask.z0, mask.side]).toEqual([100, 200, 4])
    expect(Array.from(mask.cells)).toEqual([1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0])
    expect(cropMask(undefined, 0, 0, 8).side).toBe(0)
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
