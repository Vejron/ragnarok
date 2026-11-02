import { describe, expect, it } from 'vitest'
import { chunkBlades, grassMask } from '@/render/grass'
import { mistAt } from '@/render/marshmist'
import { inMarshPool, marshField, marshPoolLevel, marshWetness, packMarsh, wetnessPixels } from '@/render/marshnoise'
import { rasterSurface, SURFACE_MUD, SURFACE_MUD_DEEP, SURFACE_MUD_FAINT, surfaceWetness } from '@/sim/surface'
import { COVER_MARSH, COVER_MARSH_DEEP, COVER_MARSH_FAINT, COVER_PARK, COVER_WOOD, ROAD_PATH, type Building, type Point2 } from '@/world'

const square = (x0: number, z0: number, x1: number, z1: number): Point2[] => [[x0, z0], [x1, z0], [x1, z1], [x0, z1]]

describe('marsh', () => {
  it('is wetter by grade, and nothing else is wet', () => {
    expect(marshWetness(COVER_MARSH_DEEP)).toBeGreaterThan(marshWetness(COVER_MARSH))
    expect(marshWetness(COVER_MARSH)).toBeGreaterThan(marshWetness(COVER_MARSH_FAINT))
    expect(marshWetness(COVER_MARSH_FAINT)).toBeGreaterThan(0)
    expect(marshWetness(COVER_WOOD)).toBe(0)
  })

  // The shader's `marshField` (engine/src/materials/marsh.wgsl.ts) is the same arithmetic in f32: if
  // these change, the sedge and the splashes no longer sit where the pools are drawn.
  it('lies the same everywhere, every time', () => {
    expect(marshField(0, 0)).toBeCloseTo(0.1187, 4)
    expect(marshField(12.5, -7.25)).toBeCloseTo(0.2613, 4)
    expect(marshField(-301.3, 845.9)).toBeCloseTo(0.5045, 4)
    const values = Array.from({ length: 4000 }, (_, i) => marshField((i % 63) * 0.71, Math.floor(i / 63) * 0.67 - 20))
    expect(Math.min(...values)).toBeGreaterThanOrEqual(0)
    expect(Math.max(...values)).toBeLessThanOrEqual(1)
    const mean = values.reduce((a, b) => a + b) / values.length
    expect(mean).toBeGreaterThan(0.4)
    expect(mean).toBeLessThan(0.6)
    // A bog is mostly pools, damp ground hardly any.
    const pools = (wet: number) => values.filter((v) => v < marshPoolLevel(wet)).length / values.length
    expect(pools(1)).toBeGreaterThan(0.3)
    expect(pools(0.35)).toBeLessThan(0.05)
    expect(inMarshPool(0, 0, 0)).toBe(false)
  })

  it('is mud underfoot, by grade', () => {
    const tile = {
      cover: [
        { kind: COVER_MARSH, rings: [square(0, 0, 20, 20)] },
        { kind: COVER_MARSH_DEEP, rings: [square(20, 0, 40, 20)] },
        { kind: COVER_MARSH_FAINT, rings: [square(40, 0, 60, 20)] },
      ],
      roads: [],
      water: new Float32Array(),
    }
    const surface = rasterSurface(tile, 0, 0, 100)
    const at = (x: number, z: number) => surface.cells[Math.floor(z / 2) * surface.side + Math.floor(x / 2)]!
    expect(at(10, 10)).toBe(SURFACE_MUD)
    expect(at(30, 10)).toBe(SURFACE_MUD_DEEP)
    expect(at(50, 10)).toBe(SURFACE_MUD_FAINT)
    expect(surfaceWetness(at(30, 10))).toBe(1)
    expect(surfaceWetness(at(80, 80))).toBe(0)
  })

  it('grows sedge, but not on a path through it or under a building', () => {
    const tile = {
      cover: [{ kind: COVER_PARK, rings: [square(0, 0, 100, 100)] }, { kind: COVER_MARSH, rings: [square(0, 0, 60, 60)] }],
      roads: [{ kind: ROAD_PATH, width: 2, points: [[0, 30], [60, 30]] as Point2[] }],
      water: new Float32Array(),
      buildings: [{ footprint: square(40, 40, 50, 50) } as Building],
      streams: [],
    }
    const mask = grassMask(tile, 0, 0, 100)
    const at = (x: number, z: number) => mask.cells[Math.floor(z / 2) * mask.side + Math.floor(x / 2)]
    expect(at(10, 10)).toBe(5)
    expect(at(80, 80)).toBe(2)
    expect(at(10, 30)).toBe(0)
    expect(at(45, 45)).toBe(0)
  })

  it('keeps its sedge out of the pools, with reeds round a bog', () => {
    const flat = () => 0
    const bog = chunkBlades(2, 5, 12, 1, () => 6, flat)
    expect(bog.length).toBeGreaterThan(0)
    for (const blade of bog) expect(inMarshPool(blade.x, blade.z, 1)).toBe(false)
    expect(bog.some((blade) => blade.height > 1.2)).toBe(true)
    const marsh = chunkBlades(2, 5, 12, 1, () => 5, flat)
    expect(marsh.some((blade) => blade.cotton)).toBe(true)
    expect(chunkBlades(2, 5, 12, 1, () => 1, flat).some((blade) => blade.cotton)).toBe(false)
  })

  it('packs its wetness into the relief map, dry off the marsh and wet inside it', () => {
    expect(wetnessPixels({ cover: [{ kind: COVER_WOOD, rings: [square(0, 0, 50, 50)] }] }, 0, 0, 100, 100)).toBeNull()
    const wet = wetnessPixels({ cover: [{ kind: COVER_MARSH_DEEP, rings: [square(20, 20, 60, 60)] }, { kind: COVER_WOOD, rings: [square(50, 50, 70, 70)] }] }, 0, 0, 100, 100)!
    expect(wet[40 * 100 + 40]).toBe(255)
    expect(wet[5 * 100 + 5]).toBe(0)
    // A dry cover painted after dries what is under it.
    expect(wet[55 * 100 + 55]).toBe(0)
    // One texel of blur at the edge.
    expect(wet[40 * 100 + 20]).toBeGreaterThan(0)
    expect(wet[40 * 100 + 20]).toBeLessThan(255)

    const relief = { data: new Uint8Array(100 * 100 * 4), width: 100, height: 100 }
    relief.data.set([10, 20, 30, 40], (40 * 100 + 40) * 4)
    const packed = packMarsh(relief, wet)
    expect([...packed.data.subarray((40 * 100 + 40) * 4, (40 * 100 + 41) * 4)]).toEqual([10, 30, 255, 40])
  })

  it('mists thickest at sunrise and sunset, least at noon', () => {
    expect(mistAt(0.25)).toBeCloseTo(1)
    expect(mistAt(0.75)).toBeCloseTo(1)
    expect(mistAt(0)).toBeCloseTo(0.55)
    expect(mistAt(0.5)).toBeLessThan(0.1)
  })
})
