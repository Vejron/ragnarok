import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { encode } from '@msgpack/msgpack'
import { RAIL_TRAIN, RAIL_TRAM, WORLD_BASE, decodeManifest, decodeTile, worldHash } from '@/world'

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
  })

  it('decodes tracks, and a tile written before them has none', () => {
    const fields = [[], [], [[], []], [], [], [], [], [], [], []]
    expect(decodeTile(encode(fields)).rails).toEqual([])
    const tile = decodeTile(encode([...fields, [[144, RAIL_TRAIN, [[0, 0], [1000, -250]]]]]))
    expect(tile.rails).toEqual([{ gauge: 1.44, kind: RAIL_TRAIN, points: [[0, 0], [10, -2.5]] }])
  })
})
