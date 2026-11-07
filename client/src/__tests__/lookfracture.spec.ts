import { describe, expect, it } from 'vitest'
import { Shell, type Tri } from '@/sim/carve'
import { lookFracture } from '@/render/lookfracture'
import { openingsOf } from '@/render/windows'
import type { CollisionMesh } from '@/world'

/** A box house 5 × 4 × 3 m: walls, then a flat roof, centimetre triangles. */
function house(): { walls: Tri[]; roof: Tri[] } {
  const [W, D, H] = [500, 400, 300]
  const quad = (a: number[], b: number[], c: number[], e: number[]): Tri[] => [[...a, ...b, ...c] as Tri, [...a, ...c, ...e] as Tri]
  return {
    walls: [
      ...quad([0, 0, 0], [W, 0, 0], [W, H, 0], [0, H, 0]),
      ...quad([W, 0, 0], [W, 0, D], [W, H, D], [W, H, 0]),
      ...quad([W, 0, D], [0, 0, D], [0, H, D], [W, H, D]),
      ...quad([0, 0, D], [0, 0, 0], [0, H, 0], [0, H, D]),
    ],
    roof: [
      [0, H, 0, W, H, 0, W, H, D],
      [0, H, 0, W, H, D, 0, H, D],
    ],
  }
}

/** The house's triangles as a look mesh, metres, one vertex a corner. */
function lookOf(tris: Tri[]): CollisionMesh {
  const vertices = new Float32Array(tris.length * 9)
  tris.forEach((t, i) => t.forEach((v, k) => (vertices[i * 9 + k] = v / 100)))
  return { vertices, triangles: Uint32Array.from(tris.flatMap((_, i) => [i * 3, i * 3 + 1, i * 3 + 2])) }
}

/** Total area of a triangle soup, square metres. */
function area(positions: Float32Array): number {
  let sum = 0
  for (let i = 0; i < positions.length; i += 9) {
    const [ax, ay, az, bx, by, bz, cx, cy, cz] = positions.slice(i, i + 9) as unknown as number[]
    const [ux, uy, uz, vx, vy, vz] = [bx! - ax!, by! - ay!, bz! - az!, cx! - ax!, cy! - ay!, cz! - az!]
    sum += Math.hypot(uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx) / 2
  }
  return sum
}

describe('lookFracture', () => {
  const { walls, roof } = house()
  const shell = Shell.of(walls, roof)!
  const look = lookOf([...walls, ...roof])
  const range = { first: 0, walls: walls.length, roof: roof.length }
  const white = [1, 1, 1] as const
  // The cell in the middle of the front wall (z = 0), 1.5 m up.
  const hole = shell.cellAt(2.5, 1.5, 0.01)

  it('draws a whole building as its own triangles', () => {
    const soup = lookFracture(look, range, shell, new Set(), new Map(), white, white)
    const openings = openingsOf(look, range)
    expect(soup.positions.length / 9).toBe(walls.length + roof.length + openings.length * 4)
    // The walls and roof, plus each opening's frame and pane.
    const panes = openings.reduce((sum, o) => sum + Math.hypot(o.corners[1][0] - o.corners[0][0], o.corners[1][2] - o.corners[0][2]) * (o.corners[3][1] - o.corners[0][1]), 0)
    expect(area(soup.positions)).toBeGreaterThan(2 * (5 + 4) * 3 + 5 * 4 + panes)
  })

  it('puts windows on every wall and one door on the longest', () => {
    const openings = openingsOf(look, range)
    expect(openings.filter((o) => o.door)).toHaveLength(1)
    const door = openings.find((o) => o.door)!
    // The 5 m walls are the longest: their normal is along z.
    expect(Math.abs(door.normal[2])).toBeCloseTo(1)
    for (const axis of [0, 2]) for (const sign of [-1, 1]) expect(openings.some((o) => Math.sign(o.normal[axis]!) === sign && Math.abs(o.normal[axis]!) > 0.9)).toBe(true)
    // Each stands within its wall: 0..5 m along x, 0..4 m along z, under the 3 m eaves.
    for (const o of openings) for (const p of o.corners) {
      expect(p[0]).toBeGreaterThanOrEqual(-1e-6)
      expect(p[0]).toBeLessThanOrEqual(5 + 1e-6)
      expect(p[1]).toBeLessThanOrEqual(3)
    }
  })

  it('cuts a hole where a cell is gone, with an edge, sooted round it', () => {
    const soup = lookFracture(look, range, shell, new Set([hole]), new Map(), white, white)
    // Rays straight at the front wall: through the hole there is nothing at the wall's plane.
    const covered = (x: number, y: number) => {
      const { positions: p } = soup
      for (let i = 0; i < p.length; i += 9) {
        if (Math.abs(p[i + 2]!) > 1e-4 || Math.abs(p[i + 5]!) > 1e-4 || Math.abs(p[i + 8]!) > 1e-4) continue
        const [ax, ay, bx, by, cx, cy] = [p[i]!, p[i + 1]!, p[i + 3]!, p[i + 4]!, p[i + 6]!, p[i + 7]!]
        const d = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
        const u = ((x - ax) * (cy - ay) - (y - ay) * (cx - ax)) / d
        const v = ((bx - ax) * (y - ay) - (by - ay) * (x - ax)) / d
        if (u >= 0 && v >= 0 && u + v <= 1) return true
      }
      return false
    }
    const [cx, cy] = shell.centre(hole).map((v) => v / 100) as [number, number, number]
    expect(covered(cx, cy)).toBe(false)
    expect(covered(0.5, 0.3)).toBe(true)
    // The edge goes into the house, behind the wall's plane.
    let deepest = 0
    for (let i = 2; i < soup.positions.length; i += 3) deepest = Math.max(deepest, soup.positions[i]!)
    expect(deepest).toBeGreaterThan(0.2)
    // Soot: some corner near the hole is darker than the wall.
    expect(Math.min(...soup.colors)).toBeLessThan(0.5)
  })

  it('chips a wall where rounds struck, more as it wears', () => {
    const plain = lookFracture(look, range, shell, new Set(), new Map(), white, white)
    const light = lookFracture(look, range, shell, new Set(), new Map([[hole, 10]]), white, white)
    const heavy = lookFracture(look, range, shell, new Set(), new Map([[hole, 39]]), white, white)
    expect(light.positions.length).toBeGreaterThan(plain.positions.length)
    expect(heavy.positions.length).toBeGreaterThan(light.positions.length)
    expect(Math.min(...heavy.colors)).toBeLessThan(0.5)
  })

  it('is the same on every client', () => {
    const a = lookFracture(look, range, shell, new Set([hole]), new Map(), white, white)
    const b = lookFracture(look, range, shell, new Set([hole]), new Map(), white, white)
    expect(Array.from(a.positions)).toEqual(Array.from(b.positions))
  })
})
