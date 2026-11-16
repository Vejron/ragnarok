import { describe, expect, it } from 'vitest'
import { serverOpenings, type Tri } from '@/sim/carve'
import { HEAD_ARCH, HEAD_ROUND, HEAD_SQUARE, drawnOpenings, openingQuads, openingTriangles, openingsOf } from '@/render/windows'
import { BUILDING_BARN, BUILDING_CHURCH, BUILDING_FARMHOUSE, BUILDING_FIRE_STATION, BUILDING_HALL, BUILDING_OUTBUILDING, BUILDING_PUBLIC, BUILDING_SHOP, BUILDING_VILLA, type CollisionMesh } from '@/world'

/*
 * The server places windows with its own port of `openingsOf` (`shared::windows`), and cuts them out
 * of the walls it collides with: the two must agree to the millimetre, or a round would fly through a
 * pane the client draws shut, or stop on glass that is drawn open.
 */

function face(corners: [number, number, number][], outward: [number, number, number]): Tri[] {
  const [a, b, c] = corners
  const n = [(b![1] - a![1]) * (c![2] - a![2]) - (b![2] - a![2]) * (c![1] - a![1]), (b![2] - a![2]) * (c![0] - a![0]) - (b![0] - a![0]) * (c![2] - a![2]), (b![0] - a![0]) * (c![1] - a![1]) - (b![1] - a![1]) * (c![0] - a![0])]
  const ring = n[0]! * outward[0] + n[1]! * outward[1] + n[2]! * outward[2] > 0 ? [...corners].reverse() : corners
  const out: Tri[] = []
  for (let i = 1; i + 1 < ring.length; i++) out.push([...ring[0]!, ...ring[i]!, ...ring[i + 1]!] as Tri)
  return out
}

/** A gabled house 6 × 4 m with 3 m walls and a ridge 4.5 m up, at an odd offset so nothing lands on a round number. */
function gable(): Tri[] {
  const [W, D, H, R, X, Z] = [600, 400, 300, 450, 137, -2231]
  const p = (x: number, y: number, z: number): [number, number, number] => [X + x, y, Z + z]
  return [
    ...face([p(0, 0, 0), p(W, 0, 0), p(W, H, 0), p(0, H, 0)], [0, 0, -1]),
    ...face([p(0, 0, D), p(W, 0, D), p(W, H, D), p(0, H, D)], [0, 0, 1]),
    ...face([p(0, 0, 0), p(0, 0, D), p(0, H, D), p(0, R, D / 2), p(0, H, 0)], [-1, 0, 0]),
    ...face([p(W, 0, 0), p(W, 0, D), p(W, H, D), p(W, R, D / 2), p(W, H, 0)], [1, 0, 0]),
  ]
}

/** A long low shed 11.3 × 3.7 m turned 25°, so the walls are not along the axes. */
function turned(): Tri[] {
  const [W, D, H] = [1130, 370, 290]
  const [c, s] = [Math.cos(0.436), Math.sin(0.436)]
  const p = (x: number, y: number, z: number): [number, number, number] => [Math.round(x * c - z * s), y, Math.round(x * s + z * c)]
  return [
    ...face([p(0, 0, 0), p(W, 0, 0), p(W, H, 0), p(0, H, 0)], [-s, 0, -c]),
    ...face([p(W, 0, 0), p(W, 0, D), p(W, H, D), p(W, H, 0)], [c, 0, s]),
    ...face([p(W, 0, D), p(0, 0, D), p(0, H, D), p(W, H, D)], [s, 0, c]),
    ...face([p(0, 0, D), p(0, 0, 0), p(0, H, 0), p(0, H, D)], [-c, 0, -s]),
  ]
}

/** The gabled house with a dormer's front on its south slope: a wall 1.5 m tall, 3 m wide, its foot 3.4 m up. */
function dormered(): Tri[] {
  const [X, Z] = [137, -2231]
  const p = (x: number, y: number, z: number): [number, number, number] => [X + x, y, Z + z]
  return [...gable(), ...face([p(150, 340, 60), p(450, 340, 60), p(450, 490, 60), p(150, 490, 60)], [0, 0, -1])]
}

/**
 * A church turned 17°: a nave 30 × 12 m with walls 9 m high and its ridge 17 m up, and a sacristy
 * 6 × 5 m, 4 m tall, against its far wall near one end. As `church` in `windows.rs`.
 */
function church(): Tri[] {
  const [W, D, E, R] = [3000, 1200, 900, 1700]
  const [c, s] = [Math.cos(0.3), Math.sin(0.3)]
  const p = (x: number, y: number, z: number): [number, number, number] => [Math.round(x * c - z * s) + 511, y, Math.round(x * s + z * c) - 977]
  const n = (x: number, z: number): [number, number, number] => [x * c - z * s, 0, x * s + z * c]
  return [
    ...face([p(0, 0, 0), p(W, 0, 0), p(W, E, 0), p(0, E, 0)], n(0, -1)),
    ...face([p(W, 0, 0), p(W, 0, D), p(W, E, D), p(W, R, D / 2), p(W, E, 0)], n(1, 0)),
    // The west end in two pieces, cut at the ridge and a centimetre apart, as the importer leaves it.
    ...face([p(0, 0, D), p(0, 0, D / 2), p(0, R, D / 2), p(0, E, D)], n(-1, 0)),
    ...face([p(1, 0, D / 2), p(1, 0, 0), p(1, E, 0), p(1, R, D / 2)], n(-1, 0)),
    ...face([p(2000, 0, D), p(0, 0, D), p(0, E, D), p(2000, E, D)], n(0, 1)),
    ...face([p(W, 0, D), p(2600, 0, D), p(2600, E, D), p(W, E, D)], n(0, 1)),
    ...face([p(2600, 400, D), p(2000, 400, D), p(2000, E, D), p(2600, E, D)], n(0, 1)),
    ...face([p(2600, 0, D), p(2600, 0, D + 500), p(2600, 400, D + 500), p(2600, 400, D)], n(1, 0)),
    ...face([p(2600, 0, D + 500), p(2000, 0, D + 500), p(2000, 400, D + 500), p(2600, 400, D + 500)], n(0, 1)),
    ...face([p(2000, 0, D + 500), p(2000, 0, D), p(2000, 400, D), p(2000, 400, D + 500)], n(-1, 0)),
  ]
}

/** A hall 31.3 × 16.7 m with walls 6.4 m high, turned 40°: a barn, a hall or a shop by the kind it is given. */
function hall(): Tri[] {
  const [W, D, H] = [3130, 1670, 640]
  const [c, s] = [Math.cos(0.7), Math.sin(0.7)]
  const p = (x: number, y: number, z: number): [number, number, number] => [Math.round(x * c - z * s) - 3319, y, Math.round(x * s + z * c) + 871]
  return [
    ...face([p(0, 0, 0), p(W, 0, 0), p(W, H, 0), p(0, H, 0)], [-s, 0, -c]),
    ...face([p(W, 0, 0), p(W, 0, D), p(W, H, D), p(W, H, 0)], [c, 0, s]),
    ...face([p(W, 0, D), p(0, 0, D), p(0, H, D), p(W, H, D)], [s, 0, c]),
    ...face([p(0, 0, D), p(0, 0, 0), p(0, H, 0), p(0, H, D)], [-c, 0, -s]),
  ]
}

function lookOf(tris: Tri[]): CollisionMesh {
  const vertices = new Float32Array(tris.length * 9)
  tris.forEach((t, i) => t.forEach((v, k) => (vertices[i * 9 + k] = v / 100)))
  return { vertices, triangles: Uint32Array.from(tris.flatMap((_, i) => [i * 3, i * 3 + 1, i * 3 + 2])) }
}

describe('window placement', () => {
  it('is the same on the server as on the client, house for house', () => {
    for (const [walls, kind] of [[gable(), 0], [turned(), 0], [dormered(), 0], [church(), BUILDING_CHURCH], [gable(), BUILDING_CHURCH], [hall(), BUILDING_BARN], [hall(), BUILDING_HALL], [hall(), BUILDING_SHOP], [hall(), BUILDING_OUTBUILDING], [gable(), BUILDING_BARN], [turned(), BUILDING_OUTBUILDING], [dormered(), BUILDING_HALL], [church(), BUILDING_SHOP], [hall(), BUILDING_PUBLIC], [church(), BUILDING_PUBLIC], [dormered(), BUILDING_PUBLIC], [gable(), BUILDING_FARMHOUSE], [dormered(), BUILDING_FARMHOUSE], [turned(), BUILDING_VILLA], [gable(), BUILDING_VILLA], [hall(), BUILDING_FIRE_STATION], [gable(), BUILDING_FIRE_STATION]] as const) {
      const ours = openingsOf(lookOf(walls), { first: 0, walls: walls.length, roof: 0, kind })
      const theirs = serverOpenings(walls, kind)
      expect(theirs.length).toBe(ours.length)
      expect(ours.length).toBeGreaterThan(0)
      ours.forEach((o, i) => {
        const t = theirs[i]!
        expect(t.door).toBe(o.door)
        expect(t.head).toBe(o.head)
        for (let k = 0; k < 4; k++) for (let a = 0; a < 3; a++) expect(t.corners[k]![a]).toBeCloseTo(o.corners[k]![a]!, 4)
        for (let a = 0; a < 3; a++) expect(t.normal[a]).toBeCloseTo(o.normal[a]!, 5)
      })
    }
  })

  it('is the same on the server as on the client for a building by a road, whichever way the road lies', () => {
    const fronts = [[0, 127], [0, -127], [127, 0], [-127, 0], [90, 90], [-110, 64], [34, -122]] as const
    const cases = [[gable(), 0], [turned(), 0], [dormered(), 0], [hall(), BUILDING_BARN], [hall(), BUILDING_HALL], [turned(), BUILDING_SHOP], [gable(), BUILDING_OUTBUILDING], [hall(), BUILDING_FIRE_STATION], [church(), BUILDING_FIRE_STATION], [church(), BUILDING_CHURCH], [dormered(), BUILDING_VILLA]] as const
    for (const [walls, kind, front] of cases.flatMap(([walls, kind]) => fronts.map((front) => [walls, kind, front] as const))) {
      const ours = openingsOf(lookOf(walls), { first: 0, walls: walls.length, roof: 0, kind, front })
      const theirs = serverOpenings(walls, kind, front)
      expect(theirs.length).toBe(ours.length)
      expect(ours.length).toBeGreaterThan(0)
      ours.forEach((o, i) => {
        const t = theirs[i]!
        expect(t.door).toBe(o.door)
        expect(t.head).toBe(o.head)
        for (let k = 0; k < 4; k++) for (let a = 0; a < 3; a++) expect(t.corners[k]![a]).toBeCloseTo(o.corners[k]![a]!, 4)
        for (let a = 0; a < 3; a++) expect(t.normal[a]).toBeCloseTo(o.normal[a]!, 5)
      })
    }
  })

  it('puts a small window in a dormer, and no door', () => {
    const openings = openingsOf(lookOf(dormered()), { first: 0, walls: dormered().length, roof: 0 })
    // The dormer's front is 0.6 m in from the south wall.
    const high = openings.filter((o) => Math.abs(o.corners[0][2] - (-2231 + 60) / 100) < 1e-3)
    // Two, a metre and a half apart, across its 3 m.
    expect(high.length).toBe(2)
    for (const o of high) {
      expect(o.door).toBe(false)
      expect(o.corners[3][1] - o.corners[0][1]).toBeCloseTo(0.7, 5)
      expect(o.corners[0][1]).toBeCloseTo(3.6, 5)
    }
    expect(openings.filter((o) => o.door).length).toBe(1)
  })

  it('gives a church tall arched windows, and a door and a round window in its end', () => {
    const openings = openingsOf(lookOf(church()), { first: 0, walls: church().length, roof: 0, kind: BUILDING_CHURCH })
    const height = (o: (typeof openings)[number]) => o.corners[3][1] - o.corners[0][1]
    // Five along the south wall and three clear of the sacristy on the north, each 5.6 m tall from 2.2 m up.
    const arched = openings.filter((o) => o.head === HEAD_ARCH)
    expect(arched.length).toBe(8)
    for (const o of arched) {
      expect(o.corners[0][1]).toBeCloseTo(2.2, 5)
      expect(height(o)).toBeCloseTo(5.6, 5)
    }
    // One door, 2 m wide and 3.2 m tall, and a round window in each end.
    const doors = openings.filter((o) => o.door)
    expect(doors.length).toBe(1)
    expect(doors[0]!.corners[3][1]).toBeCloseTo(3.2, 5)
    const round = openings.filter((o) => o.head === HEAD_ROUND)
    expect(round.length).toBe(2)
    expect(round.some((o) => o.normal.every((v, k) => Math.abs(v - doors[0]!.normal[k]!) < 1e-6))).toBe(true)
    // The sacristy keeps a house's windows, in one row.
    const plain = openings.filter((o) => o.head === HEAD_SQUARE && !o.door)
    expect(plain.length).toBeGreaterThan(0)
    for (const o of plain) expect(o.corners[3][1]).toBeLessThan(4)
  })

  it('draws a round head as wall filled in over the hole, inside its rectangle, and counts its triangles', () => {
    const openings = openingsOf(lookOf(church()), { first: 0, walls: church().length, roof: 0, kind: BUILDING_CHURCH })
    for (const head of [HEAD_ARCH, HEAD_ROUND]) {
      const o = openings.find((x) => x.head === head)!
      let [quads, masonry] = [0, 0]
      const [low, high] = [o.corners[0][1], o.corners[3][1]]
      openingQuads([o], (corners, _rgb, _normal, wall) => {
        quads++
        if (!wall) return
        masonry++
        // The filled-in wall stays within the hole's rectangle: it hides glass, and covers no wall.
        for (const p of corners) expect(p[1]).toBeGreaterThanOrEqual(low - 1e-6)
        for (const p of corners) expect(p[1]).toBeLessThanOrEqual(high + 1e-6)
      })
      expect(quads * 2).toBe(openingTriangles([o]))
      expect(masonry).toBeGreaterThan(10)
    }
  })

  it('draws a barn\'s gate as its frame only, and a house\'s open door with its leaf swung in', () => {
    for (const [kind, leaves, side] of [[BUILDING_BARN, 0, 1], [0, 1, -1]] as const) {
      const walls = hall()
      const door = openingsOf(lookOf(walls), { first: 0, walls: walls.length, roof: 0, kind }).find((o) => o.door)!
      const open = drawnOpenings([door], true)
      const sides: number[] = []
      let quads = 0
      openingQuads(open, (corners, _rgb, normal) => {
        quads++
        // A leaf stands across the wall: its face is along it, not out from it.
        if (Math.abs(normal[0] * door.normal[0] + normal[2] * door.normal[2]) > 0.5) return
        const mid = corners.reduce((sum, p) => sum + ((p[0] - door.corners[0][0]) * door.normal[0] + (p[2] - door.corners[0][2]) * door.normal[2]) / 4, 0)
        sides.push(Math.sign(mid))
      })
      expect(quads * 2).toBe(openingTriangles(open))
      // Each leaf is drawn from both sides.
      expect(sides.length).toBe(leaves * 2)
      expect(sides.every((s) => s === side)).toBe(true)
    }
  })
})
