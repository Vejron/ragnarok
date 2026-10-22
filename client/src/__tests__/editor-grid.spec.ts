import { describe, expect, it } from 'vitest'
import { blockOf, rectBounds, tileAt, type GridInfo } from '../editor/grid'
import { SimMotor } from '@/sim/motor'
import { manifestBytes } from './worlds'

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
  // The walls are the server's (`boundary_walls`, tested there); this checks the client's physics
  // raises them from the manifest's tiles.
  it('are the square for a world that fills it', async () => {
    const motor = await SimMotor.create(manifestBytes(100, 100))
    expect(motor.castRay([-50, 1, -50], [1, 0, 0], 500)!.t).toBeCloseTo(150 - 0.5, 3)
  })

  it('follow the notch of an L', async () => {
    // 2 x 2 grid of 100 m tiles, the south-east one (col 1, row 1) missing.
    const motor = await SimMotor.create(manifestBytes(100, 100, 0, [[0, 0], [1, 0], [0, 1]]))
    // Across the north row to the outer wall; from the south-west tile into the notch; from the north-east tile into it.
    expect(motor.castRay([-50, 1, -50], [1, 0, 0], 500)!.t).toBeCloseTo(150 - 0.5, 3)
    expect(motor.castRay([-50, 1, 50], [1, 0, 0], 500)!.t).toBeCloseTo(50 - 0.5, 3)
    expect(motor.castRay([50, 1, -50], [0, 0, 1], 500)!.t).toBeCloseTo(50 - 0.5, 3)
  })
})
