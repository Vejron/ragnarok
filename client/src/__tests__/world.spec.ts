import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { encode } from '@msgpack/msgpack'
import { GRAVE_CROSS, GRAVE_KINDS, RAIL_TRAIN, RAIL_TRAM, ROOF_GABLE, WORLD_BASE, decodeManifest, decodeTile, worldHash } from '@/world'
import { buildingColors } from '@/render/buildingmesh'
import { NO_CELLS } from './worlds'

// The committed synthetic world (`fixtures/world/`); the real one is generated and git-ignored.
const read = (path: string) => new Uint8Array(readFileSync(resolve(process.cwd(), '../fixtures/world', path)))

describe('world files', () => {
  it('is served from the site root unless VITE_WORLD_BASE says otherwise', () => {
    expect(WORLD_BASE).toBe('/')
  })

  it('hashes with FNV-1a like the server', () => {
    expect(worldHash(new Uint8Array())).toBe(0x811c9dc5)
    expect(worldHash(new TextEncoder().encode('a'))).toBe(0xe40c292c)
  })

  it('decodes the generated manifest', () => {
    const world = decodeManifest(read('world.msgpack'))
    expect(world.halfExtent).toBeGreaterThan(50)
    expect(world.tileSize).toBeGreaterThan(10)
    expect(world.spawns.length).toBeGreaterThan(0)
    expect(world.tiles.length).toBeGreaterThan(0)
  })

  it('decodes every tile, and each is the one the manifest hashes', () => {
    const world = decodeManifest(read('world.msgpack'))
    let buildings = 0
    let watery = 0
    let streams = 0
    let bridges = 0
    let rails = 0
    const graves = new Set<number>()
    for (const ref of world.tiles) {
      const bytes = read(`tiles/${ref.col}_${ref.row}.msgpack`)
      expect(worldHash(bytes), `tile ${ref.col}_${ref.row}`).toBe(ref.hash)
      const tile = decodeTile(bytes)
      expect(tile.collision.triangles.length % 3).toBe(0)
      if (tile.collision.triangles.length > 0) expect(Math.max(...tile.collision.triangles)).toBeLessThan(tile.collision.vertices.length / 3)
      // Everything in a tile lies within it (roads may reach their own border).
      const x0 = -world.halfExtent + ref.col * world.tileSize
      const z0 = -world.halfExtent + ref.row * world.tileSize
      for (const road of tile.roads) {
        for (const [x, z] of road.points) {
          expect(x).toBeGreaterThanOrEqual(x0 - 0.02)
          expect(x).toBeLessThanOrEqual(x0 + world.tileSize + 0.02)
          expect(z).toBeGreaterThanOrEqual(z0 - 0.02)
          expect(z).toBeLessThanOrEqual(z0 + world.tileSize + 0.02)
        }
      }
      for (const rail of tile.rails) {
        expect(rail.kind).toBe(RAIL_TRAM)
        expect(rail.gauge).toBeCloseTo(1.44)
        for (const [x, z] of rail.points) {
          expect(x).toBeGreaterThanOrEqual(x0 - 0.02)
          expect(x).toBeLessThanOrEqual(x0 + world.tileSize + 0.02)
          expect(z).toBeGreaterThanOrEqual(z0 - 0.02)
          expect(z).toBeLessThanOrEqual(z0 + world.tileSize + 0.02)
        }
      }
      for (const grave of tile.graves ?? []) {
        // A tombstone stands in its own tile.
        expect(grave.x).toBeGreaterThanOrEqual(x0)
        expect(grave.x).toBeLessThanOrEqual(x0 + world.tileSize)
        expect(grave.z).toBeGreaterThanOrEqual(z0)
        expect(grave.z).toBeLessThanOrEqual(z0 + world.tileSize)
        graves.add(grave.kind)
      }
      buildings += tile.buildings.length
      streams += tile.streams.length
      rails += tile.rails.length
      for (const bridge of tile.bridges) {
        // A bridge's triangles lie inside the tile's mesh.
        expect(bridge.first + bridge.deck + bridge.steel).toBeLessThanOrEqual(tile.collision.triangles.length / 3)
        bridges++
      }
      if (tile.water.length > 0) {
        watery++
        // Water comes with ground, sample for sample, and rises above the bed somewhere.
        expect(tile.water.length).toBe(tile.ground.length)
        expect(tile.water.some((level, at) => level > tile.ground[at]!)).toBe(true)
      }
    }
    expect(buildings).toBeGreaterThan(0)
    expect(watery).toBeGreaterThan(0)
    expect(streams).toBeGreaterThan(0)
    expect(bridges).toBeGreaterThan(0)
    expect(rails).toBeGreaterThan(0)
    // The synthetic city's graveyards hold every kind of stone.
    expect([...graves].sort()).toEqual(GRAVE_KINDS.map((def) => def.kind))
  })

  /** A tile file with every field empty, `fields` put in by index (`Tile` in `world.rs`). */
  const tileWith = (fields: Record<number, unknown>) =>
    encode(Object.assign([[], [], [[], []], [2, new Uint8Array()], [], [], new Uint8Array(), [], [], new Uint8Array(), [], [], [], new Uint8Array(), [[], [], []], []], fields))

  it('decodes ground written as residuals of a plane', () => {
    // The bytes `world.rs` pins (`ground_is_written_rounded_as_residuals_of_a_plane_through_its_neighbours`):
    // 2 cm steps, residuals 50, 10, 20, 1 zigzagged, low bytes then high.
    const packed = decodeTile(tileWith({ 3: [2, new Uint8Array([100, 20, 40, 2, 0, 0, 0, 0])] }))
    expect(Array.from(packed.ground)).toEqual([1, 1.2, 1.4, 1.62].map(Math.fround))
    // Residuals whose high byte is set: -200 steps zigzags to 399 (0x18f); the last sample adds it to the plane, -200.
    const steep = decodeTile(tileWith({ 3: [2, new Uint8Array([0, 0x8f, 0, 0x8f, 0, 1, 0, 1])] }))
    expect(Array.from(steep.ground)).toEqual([0, -4, 0, -8].map(Math.fround))
  })

  it('decodes tracks', () => {
    expect(decodeTile(tileWith({})).rails).toEqual([])
    const tile = decodeTile(tileWith({ 10: [[144, RAIL_TRAIN, [[0, 0], [1000, -250]]]] }))
    expect(tile.rails).toEqual([{ gauge: 1.44, kind: RAIL_TRAIN, points: [[0, 0], [10, -2.5]] }])
  })

  it('decodes tombstones, in metres and radians', () => {
    expect(decodeTile(tileWith({})).graves).toEqual([])
    const tile = decodeTile(tileWith({ 15: [[1250, -300, 480, -1571, GRAVE_CROSS]] }))
    expect(tile.graves).toEqual([{ x: 12.5, z: -3, base: 4.8, yaw: -1.571, kind: GRAVE_CROSS }])
  })

  it('decodes a building\'s paint, and draws it in its own colours', () => {
    const house = [600, [[0, 0], [100, 0], [100, 100]], 0, 0, 0, 0, ROOF_GABLE, 0, NO_CELLS]
    const tile = decodeTile(tileWith({ 0: [[...house, null], [...house, [0xf0e6cc, 0x56606e]]] }))
    expect(tile.buildings.map((b) => b.paint)).toEqual([null, [0xf0e6cc, 0x56606e]])
    expect(buildingColors(tile, 1, 3)).toEqual({ wall: 0xf0e6cc, roof: 0x56606e })
    // Without paint the client picks from its palettes.
    expect(buildingColors(tile, 0, 3).roof).not.toBe(0x56606e)
  })
})
