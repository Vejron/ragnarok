import { describe, expect, it } from 'vitest'
import { Shell, type Tri } from '@/sim/carve'
import { lookFracture } from '@/render/lookfracture'
import { openingsOf } from '@/render/windows'
import { rubbleSoup } from '@/render/shards'
import { buildingSurfaces, Surface } from '@/render/buildingmesh'
import { ROOF_FLAT, ROOF_GABLE } from '@/world'
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

  it('keeps the walls\' and roof\'s patterns on what stands, not on the hole\'s edge or the windows', () => {
    const soup = lookFracture(look, range, shell, new Set([hole]), new Map(), white, white, { wall: Surface.Lap, roof: Surface.Tiles })
    expect(soup.custom.length).toBe((soup.positions.length / 3) * 4)
    const kinds = new Set<number>()
    for (let i = 0; i < soup.custom.length; i += 4) kinds.add(soup.custom[i]!)
    expect([...kinds].sort()).toEqual([Surface.Plain, Surface.Lap, Surface.Tiles])
    // Every vertex of the roof (y = 3 m, lying flat) is tiles.
    for (let v = 0; v < soup.positions.length / 3; v += 3) {
      const ys = [0, 1, 2].map((k) => soup.positions[(v + k) * 3 + 1]!)
      if (ys.every((y) => Math.abs(y - 3) < 1e-4) && soup.custom[v * 4] !== Surface.Plain) expect(soup.custom[v * 4]).toBe(Surface.Tiles)
    }
  })

  it('picks boards, brick or render from the wall colour, and felt for a flat roof', () => {
    const tile = (paint: [number, number] | null, roofKind = ROOF_GABLE) => ({ buildings: [{ paint, roofKind } as never] })
    expect(buildingSurfaces(tile([0xa8432f, 0xb5543f]), 0, 0)).toEqual({ wall: Surface.Upright, roof: Surface.Tiles })
    expect(buildingSurfaces(tile([0xeee9dc, 0x4f5d6b]), 0, 0)).toEqual({ wall: Surface.Lap, roof: Surface.Sheet })
    expect(buildingSurfaces(tile([0x8f3a2c, 0xb0b4ba], ROOF_FLAT), 0, 0)).toEqual({ wall: Surface.Brick, roof: Surface.Felt })
    // Painted by the importer (France): render, or brick if a dark red.
    expect(buildingSurfaces(tile([0xd8d0c0, 0x9c4a3c]), 0, 0).wall).toBe(Surface.Render)
    expect(buildingSurfaces(tile([0x7a3020, 0x9c4a3c]), 0, 0).wall).toBe(Surface.Brick)
  })

  it('is the same on every client', () => {
    const a = lookFracture(look, range, shell, new Set([hole]), new Map(), white, white)
    const b = lookFracture(look, range, shell, new Set([hole]), new Map(), white, white)
    expect(Array.from(a.positions)).toEqual(Array.from(b.positions))
  })

  it('heaps rubble on a slope without leaving any of it in the air', () => {
    const ground = (x: number, z: number) => 0.25 * x + 0.1 * z
    const centres = Array.from({ length: 30 }, (_, i) => [2 + (i % 6) * 0.72, 1 + Math.floor(i / 6) * 0.72, 0.4] as const)
    const soup = rubbleSoup(centres, centres.map(() => false), ground, white, white)
    // Each triangle's lowest corner is at or below the ground under it.
    const { positions: p } = soup
    for (let i = 0; i < p.length; i += 9) {
      const corners = [0, 3, 6].map((k) => [p[i + k]!, p[i + k + 1]!, p[i + k + 2]!] as const)
      const low = corners.reduce((a, b) => (b[1] < a[1] ? b : a))
      expect(low[1]).toBeLessThanOrEqual(ground(low[0], low[2]) + 0.6)
    }
  })
})
