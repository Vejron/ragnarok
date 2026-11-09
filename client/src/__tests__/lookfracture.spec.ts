import { describe, expect, it } from 'vitest'
import { Shell, type Tri } from '@/sim/carve'
import { lookFracture, openingStands } from '@/render/lookfracture'
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

  it('draws a whole building as its own triangles, with holes cut for its windows and a room inside', () => {
    const soup = lookFracture(look, range, shell, new Set(), new Map(), white, white)
    const openings = openingsOf(look, range)
    // Rays straight at the front wall (z = 0): through a window's hole there is nothing in the wall's plane, beside it the wall.
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
    const window = openings.find((o) => !o.door && o.normal[2] < -0.9)!
    const [wa, wb, , wd] = window.corners
    expect(covered((wa[0] + wb[0]) / 2, (wa[1] + wd[1]) / 2)).toBe(false)
    expect(covered(wa[0] - 0.2, (wa[1] + wd[1]) / 2)).toBe(true)
    expect(covered(0.2, 0.2)).toBe(true)
    // The room inside: faces a wall's thickness behind the front wall, dim, and a floor on a plinth.
    let inner = 0
    let floor = 0
    for (let v = 0; v < soup.positions.length / 3; v++) {
      const [y, z] = [soup.positions[v * 3 + 1]!, soup.positions[v * 3 + 2]!]
      if (Math.abs(z - 0.2) < 1e-4 && soup.colors[v * 3]! < 0.5) inner++
      if (Math.abs(y - 0.45) < 1e-4) floor++
    }
    expect(inner).toBeGreaterThan(0)
    expect(floor).toBeGreaterThan(0)
    // The walls and roof, plus the frames.
    const panes = openings.reduce((sum, o) => sum + Math.hypot(o.corners[1][0] - o.corners[0][0], o.corners[1][2] - o.corners[0][2]) * (o.corners[3][1] - o.corners[0][1]), 0)
    expect(area(soup.positions)).toBeGreaterThan(2 * (2 * (5 + 4) * 3 + 5 * 4) - panes)
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

  it('keeps a window\'s frame while the wall stands round it, and drops it when that wall is gone', () => {
    const openings = openingsOf(look, range)
    const window = openings.find((o) => !o.door && o.normal[2] < -0.9)!
    const [wa, wb, , wd] = window.corners
    const [mx, my] = [(wa[0] + wb[0]) / 2, (wa[1] + wd[1]) / 2]
    // Frame quads stand a little proud of the wall (z < 0) round the window.
    const frames = (soup: { positions: Float32Array }) => {
      let n = 0
      for (let v = 0; v < soup.positions.length / 3; v++) {
        const [x, y, z] = [soup.positions[v * 3]!, soup.positions[v * 3 + 1]!, soup.positions[v * 3 + 2]!]
        if (z < -0.02 && z > -0.04 && Math.abs(x - mx) < 0.7 && Math.abs(y - my) < 0.9) n++
      }
      return n
    }
    const whole = lookFracture(look, range, shell, new Set(), new Map(), white, white)
    expect(frames(whole)).toBeGreaterThan(0)
    // A hole two cells to the side leaves the window and its frame be.
    const beside = shell.cellAt(mx + 1.6, my, 0.01)
    expect(openingStands(shell, new Set([beside]), window)).toBe(true)
    expect(frames(lookFracture(look, range, shell, new Set([beside]), new Map(), white, white))).toBe(frames(whole))
    // The wall under the window gone: nothing holds the frame, so it goes.
    const under = shell.cellAt(mx, wa[1] - 0.1, 0.01)
    expect(openingStands(shell, new Set([under]), window)).toBe(false)
    expect(frames(lookFracture(look, range, shell, new Set([under]), new Map(), white, white))).toBe(0)
  })

  it('leaves nothing in the air when a wall goes with its windows, nor a floor over where the house stood', () => {
    const openings = openingsOf(look, range)
    const window = openings.find((o) => !o.door && o.normal[2] < -0.9)!
    const [wa, wb, , wd] = window.corners
    const vertices = (soup: { positions: Float32Array }) => {
      const out: [number, number, number][] = []
      for (let v = 0; v < soup.positions.length / 3; v++) out.push([soup.positions[v * 3]!, soup.positions[v * 3 + 1]!, soup.positions[v * 3 + 2]!])
      return out
    }
    // The front of the house gone, wall and roof: no edge is drawn round the hole its window was, and the floor along the front goes, the floor further in stays.
    const front = new Set([...shell.cells].filter((id) => shell.centre(id)[2] / 100 < 1.5))
    const inWindow = vertices(lookFracture(look, range, shell, front, new Map(), white, white)).filter(([x, y, z]) => z > -0.5 && z < 0.4 && x > wa[0] + 0.05 && x < wb[0] - 0.05 && y > wa[1] + 0.05 && y < wd[1] - 0.05)
    expect(inWindow).toEqual([])
    const floor = vertices(lookFracture(look, range, shell, front, new Map(), white, white)).filter(([, y]) => Math.abs(y - 0.45) < 0.01)
    expect(floor.some(([, , z]) => z > 2)).toBe(true)
    expect(floor.filter(([x, , z]) => z < 0.5 && x > 1.5 && x < 3.5)).toEqual([])
    // Every cell gone: only the hole's edge along the ground is left to draw.
    const all = new Set(shell.cells)
    const left = vertices(lookFracture(look, range, shell, all, new Map(), white, white))
    expect(left.length).toBeGreaterThan(0)
    expect(left.every(([, y]) => y < 0.4)).toBe(true)
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
    expect(buildingSurfaces(tile([0xeee9dc, 0x6d7a8c]), 0, 0)).toEqual({ wall: Surface.Lap, roof: Surface.Sheet })
    expect(buildingSurfaces(tile([0xeee9dc, 0x4f5d6b]), 0, 0).roof).toBe(Surface.Tiles)
    expect(buildingSurfaces(tile([0x8f3a2c, 0xb0b4ba], ROOF_FLAT), 0, 0)).toEqual({ wall: Surface.Brick, roof: Surface.Felt })
    // Painted by the importer (France): render, or brick if a dark red.
    expect(buildingSurfaces(tile([0xd8d0c0, 0x9c4a3c]), 0, 0).wall).toBe(Surface.Render)
    expect(buildingSurfaces(tile([0x7a3020, 0x9c4a3c]), 0, 0).wall).toBe(Surface.Brick)
    // France's palette: limestone is cut stone, its brick brick, and slate laid in rows.
    expect(buildingSurfaces(tile([0xf0e6cc, 0x56606e]), 0, 0)).toEqual({ wall: Surface.Ashlar, roof: Surface.Tiles })
    expect(buildingSurfaces(tile([0xc0785a, 0xa4acb4]), 0, 0)).toEqual({ wall: Surface.Brick, roof: Surface.Sheet })
  })

  it('shows broken rafters in a roof\'s hole, splinters at a wooden wall\'s, and cracks across a brick one that stay on it', () => {
    const grey = [0.5, 0.5, 0.5] as const
    const count = (soup: { colors: Float32Array }, r: number, b: number) => {
      let n = 0
      for (let i = 0; i < soup.colors.length; i += 3) if (Math.abs(soup.colors[i]! - r) < 1e-3 && Math.abs(soup.colors[i + 2]! - b) < 1e-3) n++
      return n
    }
    const roofHole = new Set([shell.cellAt(2.5, 2.9, 2), shell.cellAt(1.8, 2.9, 2), shell.cellAt(2.5, 2.9, 1.3)])
    expect(count(lookFracture(look, range, shell, roofHole, new Map(), grey, grey), 0.13, 0.04)).toBeGreaterThan(0)
    const wallHole = new Set([hole, shell.cellAt(2.5, 0.8, 0.01), shell.cellAt(1.8, 1.5, 0.01)])
    expect(count(lookFracture(look, range, shell, wallHole, new Map(), grey, grey, { wall: Surface.Upright, roof: Surface.Tiles }), 0.5, 0.2)).toBeGreaterThan(0)
    const brick = lookFracture(look, range, shell, wallHole, new Map(), grey, grey, { wall: Surface.Brick, roof: Surface.Tiles })
    const plain = lookFracture(look, range, shell, wallHole, new Map(), grey, grey)
    expect(brick.positions.length).toBeGreaterThan(plain.positions.length)
    // Every crack corner (in the crack's colour, grey: the door's leaf is as red but browner) is on the front wall, a hair proud of it, within its bounds.
    const crack = 0.5 * 0.22
    for (let i = 0; i < brick.colors.length; i += 3) {
      if (Math.abs(brick.colors[i]! - crack) > 1e-4 || Math.abs(brick.colors[i + 1]! - crack) > 1e-4 || Math.abs(brick.colors[i + 2]! - crack) > 1e-4) continue
      const [x, y, z] = [brick.positions[i]!, brick.positions[i + 1]!, brick.positions[i + 2]!]
      expect(Math.abs(z)).toBeLessThan(0.02)
      expect(x).toBeGreaterThan(-0.03)
      expect(x).toBeLessThan(5.03)
      expect(y).toBeGreaterThan(-0.03)
      expect(y).toBeLessThan(3.03)
    }
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
