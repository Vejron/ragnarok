import { describe, expect, it } from 'vitest'
import { roadRibbons } from '@/render/roads'
import { GroundShade } from '@/render/shade'
import { ROAD_MINOR, ROAD_PATH, type Road } from '@/world'

const flat = () => 2
const buildRoads = (roads: Road[], border: readonly [number, number, number, number]) => roadRibbons(roads, new GroundShade(flat, 1, border), border)

/** The ribbon's vertices' x either side of `x` where they cross `z`. */
const sidesAt = (positions: Float32Array, z: number) => {
  const xs: number[] = []
  for (let i = 0; i < positions.length; i += 3) if (Math.abs(positions[i + 2]! - z) < 1e-4) xs.push(positions[i]!)
  return [Math.min(...xs), Math.max(...xs)]
}

describe('roads', () => {
  it('marks a path worn, with where across it each vertex is', () => {
    const [path] = buildRoads([{ kind: ROAD_PATH, width: 1.6, points: [[50, 10], [50, 90]] }], [0, 0, 100, 100])
    const { positions, custom } = path!.buffers
    expect(custom).toBeDefined()
    expect(custom!.length).toBe((positions.length / 3) * 4)
    for (let i = 0; i < custom!.length; i += 4) {
      expect(Math.abs(custom![i]!)).toBeLessThanOrEqual(1)
      expect(custom![i + 1]).toBeCloseTo(0.6)
    }
  })

  it('draws a road whole and at its own width', () => {
    const ribbons = buildRoads([{ kind: ROAD_MINOR, width: 6, points: [[50, 10], [50, 90]] }], [0, 0, 100, 100])
    for (const { buffers } of ribbons) expect(buffers.custom).toBeUndefined()
    const [left, right] = sidesAt(ribbons[1]!.buffers.positions, 30)
    expect(right! - left!).toBeCloseTo(6)
  })

  it('varies a path’s width along it, the same on both sides of a tile border', () => {
    const widths = new Set<number>()
    const [path] = buildRoads([{ kind: ROAD_PATH, width: 1.6, points: [[50, 0], [50, 100]] }], [0, 0, 100, 100])
    for (let z = 0; z <= 100; z += 10) {
      const [left, right] = sidesAt(path!.buffers.positions, z)
      widths.add(Math.round((right! - left!) * 100))
    }
    expect(widths.size).toBeGreaterThan(3)
    // Where it leaves this tile and where the next tile's piece starts, it is as wide.
    const [below] = buildRoads([{ kind: ROAD_PATH, width: 1.6, points: [[50, 100], [50, 200]] }], [0, 100, 100, 200])
    const [a0, a1] = sidesAt(path!.buffers.positions, 100)
    const [b0, b1] = sidesAt(below!.buffers.positions, 100)
    expect(a0).toBeCloseTo(b0!)
    expect(a1).toBeCloseTo(b1!)
  })
})
