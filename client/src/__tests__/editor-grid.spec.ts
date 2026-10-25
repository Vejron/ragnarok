import { describe, expect, it } from 'vitest'
import { blockOf, project, rectBounds, rectCorners, tileAt, unproject, type GridInfo } from '../editor/grid'
import { forward, inverse } from '../editor/lambert93'
import { SimMotor } from '@/sim/motor'
import { manifestBytes } from './worlds'

// The values `GET /api/grid` returns: the importer's fixed global grids.
const common = { tile: 256, cells: [1, 2, 4], blockTiles: 8, metresPerDegLat: 110540, metresPerDegLon: 111320 }
const grid: GridInfo = { region: 'sweden', frame: { kind: 'equirect', origin: [63.8225, 20.235], scaleLatitude: 63.8225 }, origin: [63.8225, 20.235], ...common }
const france: GridInfo = { region: 'france', frame: { kind: 'lambert93', e0: 700000, n0: 6600000 }, origin: [46.5, 3], ...common }

describe('editor grid', () => {
  it('puts the origin on a corner of four tiles, as osm-import does', () => {
    expect(tileAt(grid, 63.8225 + 1e-6, 20.235 + 1e-6)).toEqual({ gx: 0, gy: -1 })
    expect(tileAt(grid, 63.8225 - 1e-6, 20.235 - 1e-6)).toEqual({ gx: -1, gy: 0 })
  })

  it('finds a tile inside its own bounds', () => {
    const [s, w, n, e] = rectBounds(grid, 7, -13)
    expect(tileAt(grid, (s + n) / 2, (w + e) / 2)).toEqual({ gx: 7, gy: -13 })
    expect(n - s).toBeCloseTo(256 / 110540, 9)
  })

  it('projects to Lambert-93 as the importer does', () => {
    // The projection's false origin, and the round trip.
    const [e, n] = forward(46.5, 3)
    expect(e).toBeCloseTo(700000, 3)
    expect(n).toBeCloseTo(6600000, 3)
    const [lat, lon] = inverse(...forward(48.8566, 2.3522))
    expect(lat).toBeCloseTo(48.8566, 9)
    expect(lon).toBeCloseTo(2.3522, 9)
    expect(project(france, 46.5, 3)).toEqual([expect.closeTo(0, 3), expect.closeTo(0, 3)])
    expect(unproject(france, 256, 256)[0]).toBeLessThan(46.5)
  })

  it('finds a French tile inside its own corners', () => {
    const corners = rectCorners(france, -400, -300)
    const lat = corners.reduce((sum, c) => sum + c[0], 0) / 4
    const lon = corners.reduce((sum, c) => sum + c[1], 0) / 4
    expect(tileAt(france, lat, lon)).toEqual({ gx: -400, gy: -300 })
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
