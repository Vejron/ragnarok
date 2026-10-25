import { Mesh } from '@ragnarok/engine'
import { describe, expect, it } from 'vitest'
import { groundOf, heightIn, Terrain, type TileGround } from '@/sim/terrain'
import type { TileData, WorldData } from '@/world'

/** `bumpy()` in crates/shared/src/terrain.rs: 9 × 9 samples, 5 m apart, from (0, 0). */
function bumpy(): TileGround {
  const heights = new Float32Array(81)
  for (let j = 0; j < 9; j++) for (let i = 0; i < 9; i++) heights[j * 9 + i] = (((i * 37 + j * 91) % 400) - 100) / 100
  return { x0: 0, z0: 0, cell: 5, side: 9, heights }
}

/** `TERRAIN_FIXTURE` there. */
const fixture: [number, number, number][] = [
  [3.3, 7.9, 0.682],
  [12.5, 12.5, 2.2],
  [0, 0, -1],
  [39.99, 20.01, 1.60108],
  [-5, 50, 2.28],
  [21.7, 33.3, 2.6664],
]

describe('terrain sampling', () => {
  it('agrees with the server on the same grid', () => {
    for (const [x, z, expected] of fixture) expect(heightIn(bumpy(), x, z), `(${x}, ${z})`).toBeCloseTo(expected, 3)
  })

  it('looks tiles up by position, and is flat where none is loaded', () => {
    const terrain = new Terrain(100, 40)
    // Tile (2, 2) covers [0, 40) in x and z.
    terrain.set({ col: 2, row: 2, hash: 0 }, bumpy())
    expect(terrain.heightAt(3.3, 7.9)).toBeCloseTo(0.682, 3)
    expect(terrain.heightAt(-30, -30)).toBe(0)
    terrain.delete({ col: 2, row: 2, hash: 0 })
    expect(terrain.heightAt(3.3, 7.9)).toBe(0)
  })

  it('places a tile by its column and row, and ignores flat worlds', () => {
    const world: WorldData = {
      frame: [0, 0, 0], halfExtent: 100, tileSize: 40, spawns: [], tiles: [], groundCell: 5,
      collision: { vertices: new Float32Array(), triangles: new Uint32Array() },
    }
    const tile: TileData = { buildings: [], roads: [], cover: [], trees: [], collision: world.collision, ground: bumpy().heights, water: new Float32Array(), streams: [], bridges: [], rails: [] }
    expect(groundOf(world, { col: 3, row: 1, hash: 0 }, tile)).toMatchObject({ x0: 20, z0: -60, cell: 5, side: 9 })
    expect(groundOf({ ...world, groundCell: 0 }, { col: 0, row: 0, hash: 0 }, tile)).toBeNull()
  })
})

describe('terrain mesh', () => {
  it('faces up and follows the heights', async () => {
    const { buildTile } = await import('@/render/world')
    const tile: TileData = { buildings: [], roads: [], cover: [], trees: [], collision: { vertices: new Float32Array(), triangles: new Uint32Array() }, ground: new Float32Array(), water: new Float32Array(), streams: [], bridges: [], rails: [] }
    const group = buildTile(tile, 0, bumpy())
    const mesh = group.children[0] as Mesh
    const { geometry } = mesh
    const position = geometry.positions
    const normal = geometry.normals!
    for (let i = 0; i < geometry.vertexCount; i++) expect(normal[i * 3 + 1], `vertex ${i}`).toBeGreaterThan(0)
    // Every vertex on the surface sits on its sample (skirt vertices hang below theirs).
    let onSamples = 0
    for (let i = 0; i < geometry.vertexCount; i++) {
      const [x, y, z] = [position[i * 3]!, position[i * 3 + 1]!, position[i * 3 + 2]!]
      const sample = bumpy().heights[(z / 5) * 9 + x / 5]!
      expect(y).toBeLessThanOrEqual(sample + 1e-5)
      if (Math.abs(y - sample) < 1e-5) onSamples++
    }
    expect(onSamples).toBeGreaterThan(4)
    // Shaded by its relief map, one texel a cell.
    expect(mesh.material.normalMap?.width).toBe(8)
    expect(mesh.material.normalMapTone).toBe(true)
  })
})
