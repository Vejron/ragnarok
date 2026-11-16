import { describe, expect, it } from 'vitest'
import { detailTriangles, detailsOf } from '@/render/details'
import { openingsOf } from '@/render/windows'
import { BUILDING_BARN, BUILDING_FARMHOUSE, BUILDING_PLAIN, BUILDING_VILLA, type CollisionMesh } from '@/world'

/** A gabled house 10 × 7 m along x, its walls 3 m high and its ridge 5 m up, as a look: 8 wall triangles, then 4 of roof. */
function gabled(): CollisionMesh {
  const quad = (a: number[], b: number[], c: number[], d: number[]) => [a, b, c, a, c, d]
  const walls = [
    ...quad([0, 0, 0], [10, 0, 0], [10, 3, 0], [0, 3, 0]),
    ...quad([10, 0, 0], [10, 0, 7], [10, 3, 7], [10, 3, 0]),
    ...quad([10, 0, 7], [0, 0, 7], [0, 3, 7], [10, 3, 7]),
    ...quad([0, 0, 7], [0, 0, 0], [0, 3, 0], [0, 3, 7]),
  ]
  const roof = [...quad([0, 3, 0], [10, 3, 0], [10, 5, 3.5], [0, 5, 3.5]), ...quad([10, 3, 7], [0, 3, 7], [0, 5, 3.5], [10, 5, 3.5])]
  const corners = [...walls, ...roof]
  return { vertices: Float32Array.from(corners.flat()), triangles: Uint32Array.from(corners.map((_, i) => i)) }
}

const RANGE = { first: 0, walls: 8, roof: 4 }

describe('the small parts of a house', () => {
  it('stands a chimney on the ridge of a house, and none on a barn', () => {
    const look = gabled()
    for (const kind of [BUILDING_PLAIN, BUILDING_FARMHOUSE, BUILDING_VILLA]) {
      const range = { ...RANGE, kind }
      const chimney = detailsOf(look, range, openingsOf(look, range)).filter((q) => q.paint === 'chimney')
      expect(chimney.length).toBe(5)
      const points = chimney.flatMap((q) => q.corners)
      // On the ridge (z = 3.5), a third of the way along it, and rising over it.
      for (const p of points) expect(Math.abs(p[2] - 3.5)).toBeLessThan(0.31)
      for (const p of points) expect(Math.abs(p[0] - 3.5)).toBeLessThan(0.41)
      expect(Math.max(...points.map((p) => p[1]))).toBeCloseTo(5.9, 5)
    }
    const barn = { ...RANGE, kind: BUILDING_BARN }
    expect(detailsOf(look, barn, openingsOf(look, barn))).toEqual([])
  })

  it('builds a porch on posts over an old house\'s door, a canopy over a villa\'s and nothing over a plain one\'s', () => {
    const look = gabled()
    const parts = (kind: number) => {
      const range = { ...RANGE, kind }
      const openings = openingsOf(look, range)
      const door = openings.find((o) => o.door)!
      return { door, rest: detailsOf(look, range, openings).filter((q) => q.paint !== 'chimney'), all: detailsOf(look, range, openings) }
    }
    expect(parts(BUILDING_PLAIN).rest).toEqual([])
    const old = parts(BUILDING_FARMHOUSE)
    // A roof, its underside and edge, and two posts of five faces.
    expect(old.rest.length).toBe(13)
    expect(old.rest.filter((q) => q.paint === 'roof').length).toBe(1)
    expect(detailTriangles(old.all)).toBe(2 * old.all.length)
    // Out from the wall the door is in, over its top, and down to the ground.
    const out = (p: readonly number[]) => (p[0]! - old.door.corners[0][0]) * old.door.normal[0] + (p[2]! - old.door.corners[0][2]) * old.door.normal[2]
    const points = old.rest.flatMap((q) => q.corners)
    expect(Math.min(...points.map(out))).toBeGreaterThan(-1e-6)
    expect(Math.max(...points.map(out))).toBeCloseTo(1.4, 5)
    expect(Math.max(...points.map((p) => p[1]))).toBeGreaterThan(old.door.corners[3][1])
    expect(Math.min(...points.map((p) => p[1]))).toBeLessThan(old.door.corners[0][1])
    const villa = parts(BUILDING_VILLA)
    expect(villa.rest.length).toBe(3)
    expect(Math.min(...villa.rest.flatMap((q) => q.corners).map((p) => p[1]))).toBeGreaterThan(villa.door.corners[3][1])
  })
})
