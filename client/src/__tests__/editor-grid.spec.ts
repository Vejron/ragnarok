import { describe, expect, it } from 'vitest'
import { blockOf, rectBounds, tileAt, type GridInfo } from '../editor/grid'

// The values `GET /api/grid` returns: the importer's fixed global grid.
const grid: GridInfo = { origin: [63.8225, 20.235], tile: 250, blockTiles: 8, metresPerDegLat: 110540, metresPerDegLon: 111320 }

describe('editor grid', () => {
  it('puts the origin on a corner of four tiles, as osm-import does', () => {
    expect(tileAt(grid, 63.8225 + 1e-6, 20.235 + 1e-6)).toEqual({ gx: 0, gy: -1 })
    expect(tileAt(grid, 63.8225 - 1e-6, 20.235 - 1e-6)).toEqual({ gx: -1, gy: 0 })
  })

  it('finds a tile inside its own bounds', () => {
    const [s, w, n, e] = rectBounds(grid, 7, -13)
    expect(tileAt(grid, (s + n) / 2, (w + e) / 2)).toEqual({ gx: 7, gy: -13 })
    expect(n - s).toBeCloseTo(250 / 110540, 9)
  })

  it('groups tiles into blocks with negative indices', () => {
    expect(blockOf(grid, 7, 0)).toEqual([0, 0])
    expect(blockOf(grid, 8, -1)).toEqual([1, -1])
    expect(blockOf(grid, -1, -8)).toEqual([-1, -1])
  })
})

describe('boundary walls of a shaped world', () => {
  it('are the square for a world that fills it', async () => {
    const { boundaryWalls } = await import('../sim/physics')
    expect(boundaryWalls({ halfExtent: 100, tileSize: 100, inside: null })).toHaveLength(4)
  })

  it('follow the notch of an L (mirrors the server test)', async () => {
    const { boundaryWalls } = await import('../sim/physics')
    // 2 x 2 grid of 100 m tiles, south-east tile missing.
    const walls = boundaryWalls({ halfExtent: 100, tileSize: 100, inside: [true, true, true, false] })
    // Outer north, west, east (north row only), south (west tile only), and the notch's two edges.
    expect(walls).toHaveLength(6)
    expect(walls).toContainEqual([50, 0, 50, 0.5])
    expect(walls).toContainEqual([0, 50, 0.5, 50])
  })
})
