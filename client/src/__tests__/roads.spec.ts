import { describe, expect, it } from 'vitest'
import { PROFILE_SCALE, paintProfile, roadRibbons } from '@/render/roads'
import { GroundShade } from '@/render/shade'
import { MARK_CENTRE, MARK_EDGE, ROAD_MAJOR, ROAD_MINOR, ROAD_PATH, type Road } from '@/world'

const flat = () => 2
const buildRoads = (roads: Road[], border: readonly [number, number, number, number]) => roadRibbons(roads, new GroundShade(flat, 1, border), border)

/** The ribbon's vertices' x at its farthest either side, in the cross-section nearest `z`. */
const sidesAt = (positions: Float32Array, z: number) => {
  let nearest = Infinity
  for (let i = 0; i < positions.length; i += 3) nearest = Math.min(nearest, Math.abs(positions[i + 2]! - z))
  const xs: number[] = []
  for (let i = 0; i < positions.length; i += 3) if (Math.abs(Math.abs(positions[i + 2]! - z) - nearest) < 1e-4) xs.push(positions[i]!)
  return [Math.min(...xs), Math.max(...xs)]
}

describe('roads', () => {
  it('marks a path worn, with where across it each vertex is', () => {
    const [path] = buildRoads([{ kind: ROAD_PATH, width: 1.6, carried: false, lanes: 0, forward: 0, marks: 0, along: 0, points: [[50, 10], [50, 90]] }], [0, 0, 100, 100])
    const { positions, custom } = path!.buffers
    expect(custom).toBeDefined()
    expect(custom!.length).toBe((positions.length / 3) * 4)
    for (let i = 0; i < custom!.length; i += 4) {
      expect(Math.abs(custom![i]!)).toBeLessThanOrEqual(1)
      expect(custom![i + 1]).toBeCloseTo(0.6)
    }
  })

  it('draws a paved road as asphalt at its own width, on a gravel shoulder', () => {
    const [shoulder, fill] = buildRoads([{ kind: ROAD_MINOR, width: 6, carried: false, lanes: 0, forward: 0, marks: 0, along: 0, points: [[50, 10], [50, 90]] }], [0, 0, 100, 100])
    expect(fill!.look).toBe('asphalt')
    const { positions, custom } = fill!.buffers
    expect(custom!.length).toBe((positions.length / 3) * 4)
    for (let i = 0; i < custom!.length; i += 4) expect(Math.abs(custom![i]!)).toBeLessThanOrEqual(1)
    const [left, right] = sidesAt(positions, 30)
    expect(right! - left!).toBeCloseTo(6)
    // The shoulder is worn gravel a little wider all along, its width not wandering as a path's does.
    expect(shoulder!.look).toBe('worn')
    for (const z of [20, 50, 80]) {
      const [l, r] = sidesAt(shoulder!.buffers.positions, z)
      expect(r! - l!).toBeCloseTo(7.2)
    }
  })

  it('measures across a road by how far from it each vertex is, at its joints too, and places it on the tile’s map', () => {
    // A joint at (50, 50): the disc there must fade its edges as the strips either side do, not in a ring.
    const points: [number, number][] = [[50, 10], [50, 50], [50, 90]]
    const [, fill] = buildRoads([{ kind: ROAD_MINOR, width: 6, carried: false, lanes: 0, forward: 0, marks: 0, along: 0, points }], [0, 0, 100, 100])
    const { positions, custom, uvs } = fill!.buffers
    const toRoad = (x: number, z: number) =>
      Math.min(
        ...points.slice(1).map(([bx, bz], i) => {
          const [ax, az] = points[i]!
          const t = Math.max(0, Math.min(1, ((x - ax) * (bx - ax) + (z - az) * (bz - az)) / ((bx - ax) ** 2 + (bz - az) ** 2)))
          return Math.hypot(x - ax - (bx - ax) * t, z - az - (bz - az) * t)
        }),
      )
    for (let v = 0; v < positions.length / 3; v++) {
      const [x, z] = [positions[v * 3]!, positions[v * 3 + 2]!]
      expect(Math.abs(custom![v * 4]!)).toBeCloseTo(Math.min(1, toRoad(x, z) / 3), 3)
      expect(custom![v * 4 + 3]).toBeCloseTo(3)
      expect(uvs![v * 2]).toBeCloseTo(x / 100)
      expect(uvs![v * 2 + 1]).toBeCloseTo(z / 100)
    }
  })

  it('draws a road’s edge lying inside another road at a junction as the middle of the road', () => {
    // A side road ending on a main road: its end's edge lies well inside the main road's asphalt.
    const ribbons = buildRoads([
      { kind: ROAD_MINOR, width: 6, carried: false, lanes: 0, forward: 0, marks: 0, along: 0, points: [[10, 50], [90, 50]] },
      { kind: ROAD_MINOR, width: 6, carried: false, lanes: 0, forward: 0, marks: 0, along: 0, points: [[50, 50], [50, 90]] },
    ], [0, 0, 100, 100])
    const { positions, custom } = ribbons[1]!.buffers
    for (let v = 0; v < positions.length / 3; v++) {
      const [x, z] = [positions[v * 3]!, positions[v * 3 + 2]!]
      // Within a metre and a half of the main road's centre line, nothing is an edge.
      if (Math.abs(z - 50) < 1.5 && x > 20 && x < 80) expect(custom![v * 4]).toBeLessThan(0.5)
    }
  })

  it('varies a path’s width along it, the same on both sides of a tile border', () => {
    const widths = new Set<number>()
    const [path] = buildRoads([{ kind: ROAD_PATH, width: 1.6, carried: false, lanes: 0, forward: 0, marks: 0, along: 0, points: [[50, 0], [50, 100]] }], [0, 0, 100, 100])
    for (let z = 0; z <= 100; z += 10) {
      const [left, right] = sidesAt(path!.buffers.positions, z)
      widths.add(Math.round((right! - left!) * 100))
    }
    expect(widths.size).toBeGreaterThan(3)
    // Where it leaves this tile and where the next tile's piece starts, it is as wide.
    const [below] = buildRoads([{ kind: ROAD_PATH, width: 1.6, carried: false, lanes: 0, forward: 0, marks: 0, along: 0, points: [[50, 100], [50, 200]] }], [0, 100, 100, 200])
    const [a0, a1] = sidesAt(path!.buffers.positions, 100)
    const [b0, b1] = sidesAt(below!.buffers.positions, 100)
    expect(a0).toBeCloseTo(b0!, 1)
    expect(a1).toBeCloseTo(b1!, 1)
  })

  it('carries where each asphalt vertex is along the road and how far right of its middle', () => {
    // Running north (-z) from z = 90 to z = 10: right of the way it runs is east, +x.
    const road: Road = { kind: ROAD_MAJOR, width: 8, carried: false, lanes: 2, forward: 1, marks: MARK_EDGE | MARK_CENTRE, along: 500, points: [[50, 90], [50, 10]] }
    const [, fill] = buildRoads([road], [0, 0, 100, 100])
    const { positions, custom } = fill!.buffers
    for (let v = 0; v < positions.length / 3; v++) {
      const [x, z] = [positions[v * 3]!, positions[v * 3 + 2]!]
      expect(custom![v * 4 + 1]).toBeCloseTo(500 + (90 - z), 3)
      expect(custom![v * 4 + 2]).toBeCloseTo(x - 50, 3)
      expect(custom![v * 4 + 3]).toBeCloseTo(4 + PROFILE_SCALE * paintProfile(road), 2)
    }
  })

  it('runs a dashed line on unbroken across a tile border', () => {
    const piece = (points: [number, number][], along: number): Road => ({ kind: ROAD_MAJOR, width: 8, carried: false, lanes: 2, forward: 1, marks: MARK_CENTRE, along, points })
    const [, here] = buildRoads([piece([[50, 10], [50, 100]], 0)], [0, 0, 100, 100])
    const [, there] = buildRoads([piece([[50, 100], [50, 150]], 90)], [0, 100, 100, 200])
    // Both pieces reach a little past the border, and where they lap each says the same distance along.
    const alongAt = (ribbon: typeof here, z: number) => {
      const { positions, custom } = ribbon!.buffers
      for (let v = 0; v < positions.length / 3; v++) if (Math.abs(positions[v * 3 + 2]! - z) < 1e-3 && Math.abs(positions[v * 3]! - 50) < 1e-3) return custom![v * 4 + 1]!
      return NaN
    }
    const lap = here!.buffers.positions.reduce((far, value, i) => (i % 3 === 2 ? Math.max(far, value) : far), 0)
    expect(lap).toBeGreaterThan(100)
    expect(alongAt(here, lap)).toBeCloseTo(lap - 10, 3)
    const start = there!.buffers.positions.reduce((near, value, i) => (i % 3 === 2 ? Math.min(near, value) : near), Infinity)
    expect(start).toBeLessThan(100)
    expect(alongAt(there, start)).toBeCloseTo(90 + start - 100, 3)
  })

  it('lays a paved road’s shoulder along its edges only, not under its middle', () => {
    const [shoulder] = buildRoads([{ kind: ROAD_MINOR, width: 6, carried: false, lanes: 2, forward: 1, marks: 0, along: 0, points: [[50, 10], [50, 50], [50, 90]] }], [0, 0, 100, 100])
    const { positions } = shoulder!.buffers
    // No triangle's middle lies within the asphalt, away from its crumbling edge.
    const { indices } = shoulder!.buffers
    for (let t = 0; t < indices.length; t += 3) {
      const x = (positions[indices[t]! * 3]! + positions[indices[t + 1]! * 3]! + positions[indices[t + 2]! * 3]!) / 3
      const z = (positions[indices[t]! * 3 + 2]! + positions[indices[t + 1]! * 3 + 2]! + positions[indices[t + 2]! * 3 + 2]!) / 3
      expect(Math.hypot(x - 50, z - Math.max(10, Math.min(90, z)))).toBeGreaterThan(2.5)
    }
  })

  it('mitres a bend, the strips either side meeting edge to edge with nothing drawn over the corner', () => {
    // A 6 m road turning 45 degrees at (50, 50), well inside the tile.
    const road = { kind: ROAD_MINOR, width: 6, carried: false, lanes: 0, forward: 0, marks: 0, along: 0 }
    const [, bent] = buildRoads([{ ...road, points: [[50, 10], [50, 50], [80, 80]] }], [0, 0, 100, 100])
    const straight = (points: [number, number][]) => buildRoads([{ ...road, points }], [0, 0, 100, 100])[1]!.buffers.positions.length
    // No more vertices than its two strips have alone, less the round cap each would have at the bend.
    const cap = straight([[50, 10], [50, 50]]) + straight([[50, 50], [80, 80]]) - bent!.buffers.positions.length
    expect(cap).toBeGreaterThan(0)
    // The outer corner of the bend is reached, where the two outer edges meet on the line that halves
    // it, and that is the farthest any of the road lies from its line.
    const { positions } = bent!.buffers
    const toSegment = (x: number, z: number, [ax, az]: number[], [bx, bz]: number[]) => {
      const t = Math.max(0, Math.min(1, ((x - ax!) * (bx! - ax!) + (z - az!) * (bz! - az!)) / ((bx! - ax!) ** 2 + (bz! - az!) ** 2)))
      return Math.hypot(x - ax! - (bx! - ax!) * t, z - az! - (bz! - az!) * t)
    }
    let farthest = 0
    for (let v = 0; v < positions.length; v += 3) {
      const [x, z] = [positions[v]!, positions[v + 2]!]
      // Away from the two ends, which are round.
      if (z > 15 && z < 75) farthest = Math.max(farthest, Math.min(toSegment(x, z, [50, 10], [50, 50]), toSegment(x, z, [50, 50], [80, 80])))
    }
    expect(farthest).toBeCloseTo(3 / Math.cos(Math.PI / 8), 1)
  })

  it('narrows a wide road to a narrower one that carries on, and rounds one that ends alone', () => {
    const road = { kind: ROAD_MINOR, carried: false, lanes: 0, forward: 0, marks: 0, along: 0 }
    const wide = { ...road, width: 10, points: [[50, 10], [50, 50]] as [number, number][] }
    const narrow = { ...road, width: 5, points: [[50, 50], [50, 90]] as [number, number][] }
    const beyond = (roads: Road[]) => {
      // How far past the wide road's end, and outside the narrow road's width, anything is drawn.
      const { positions } = buildRoads(roads, [0, 0, 100, 100])[1]!.buffers
      let far = 0
      for (let v = 0; v < positions.length; v += 3) if (Math.abs(positions[v]! - 50) > 2.6) far = Math.max(far, positions[v + 2]! - 50)
      return far
    }
    expect(beyond([wide, narrow])).toBeLessThan(0.01)
    // It is its full width 10 m back, and half way in between 4 m back.
    const { positions } = buildRoads([wide, narrow], [0, 0, 100, 100])[1]!.buffers
    expect(sidesAt(positions, 40)[1]! - sidesAt(positions, 40)[0]!).toBeCloseTo(10)
    expect(sidesAt(positions, 46)[1]! - sidesAt(positions, 46)[0]!).toBeCloseTo(7.5)
    // Alone, its end is a round cap half its width out.
    expect(beyond([wide])).toBeGreaterThan(2)
  })
})
