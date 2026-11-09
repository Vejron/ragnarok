import type { CollisionMesh } from '@/world'
import { add, centreOf, dot, keyOf, normalOf, splitPlane, sub, triangulate, unit, type P, type Poly, type RGB } from './geom'
import { REVEAL, openingsFor, type LookRange, type Opening } from './windows'

/*
 * A looked building's faces as they are drawn, worked out once from its look (`Tile.look`): the
 * walls with the windows cut out of them, the roof, and inside them a room to see into through the
 * glass: the walls' and roof's inner faces a wall's thickness in, dim, and a floor. Pure, so it runs
 * in the tile worker too.
 */

/** How thick a wall is drawn, metres: its inner face is this far in, as deep as a window's reveal. */
export const WALL_THICKNESS = REVEAL
/** The share of its colour an inner face keeps: a room is dim beside the sunlit outside. */
export const INTERIOR_SHADE = 0.18
/** The floor: dark boards. */
export const FLOOR_RGB: RGB = [0.07, 0.05, 0.035]
/** How far above the building's lowest corner the floor lies: on a plinth, as the importer builds houses, so sloping ground inside the footprint stays under it. */
const FLOOR_UP = 0.45

export interface Facade {
  /** Outward-wound, the windows cut out. */
  walls: Poly[]
  roof: Poly[]
  /** Facing up, at the foot of the walls. */
  floor: Poly[]
}

/** The faces of building `range` of `look`. */
export function facadeOf(look: CollisionMesh, range: LookRange): Facade {
  const { vertices, triangles } = look
  const corner = (v: number): P => [vertices[v * 3]!, vertices[v * 3 + 1]!, vertices[v * 3 + 2]!]
  // Wound outwards: the look is wound inwards.
  const poly = (t: number): Poly => [corner(triangles[t * 3]!), corner(triangles[t * 3 + 2]!), corner(triangles[t * 3 + 1]!)]
  let walls: Poly[] = []
  for (let t = range.first; t < range.first + range.walls; t++) walls.push(poly(t))
  const roof: Poly[] = []
  for (let t = range.first + range.walls; t < range.first + range.walls + range.roof; t++) roof.push(poly(t))
  for (const o of openingsFor(look, range)) if (!o.door) walls = cutWindow(walls, o)
  return { walls, roof, floor: floorOf(walls) }
}

/** `walls` with window `o` cut out of the ones in its plane. */
function cutWindow(walls: Poly[], o: Opening): Poly[] {
  const [a, b, , d] = o.corners
  const n = o.normal
  const along = unit([b[0] - a[0], 0, b[2] - a[2]])
  const planeD = dot(a, n)
  const [u0, u1] = [dot(a, along), dot(b, along)]
  const [v0, v1] = [a[1], d[1]]
  const out: Poly[] = []
  for (const wall of walls) {
    const wn = normalOf(wall)
    if (dot(wn, n) < 0.999 || Math.abs(dot(wall[0]!, n) - planeD) > 0.03) {
      out.push(wall)
      continue
    }
    let pieces: Poly[] = [wall]
    for (const [pn, pd] of [[along, u0], [along, u1], [[0, 1, 0] as P, v0], [[0, 1, 0] as P, v1]] as [P, number][]) {
      const next: Poly[] = []
      for (const piece of pieces) for (const part of splitPlane(piece, pn, pd)) if (part.length >= 3) next.push(part)
      pieces = next
    }
    for (const piece of pieces) {
      const c = centreOf(piece)
      const [u, v] = [dot(c, along), c[1]]
      if (u > u0 && u < u1 && v > v0 && v < v1) continue
      out.push(piece)
    }
  }
  return out
}

/** The floor under `walls`: the ring their bottom edges make, triangulated, facing up. */
function floorOf(walls: readonly Poly[]): Poly[] {
  let low = Infinity
  for (const wall of walls) for (const p of wall) low = Math.min(low, p[1])
  if (!Number.isFinite(low)) return []
  // The bottom edges, each corner leading to the next along the wall's winding, chained into rings.
  const next = new Map<string, P>()
  const points = new Map<string, P>()
  for (const wall of walls) for (let i = 0; i < wall.length; i++) {
    const [p, q] = [wall[i]!, wall[(i + 1) % wall.length]!]
    if (Math.abs(p[1] - low) > 0.05 || Math.abs(q[1] - low) > 0.05) continue
    if (Math.hypot(q[0] - p[0], q[2] - p[2]) < 0.01) continue
    const key = keyOf(p)
    if (next.has(key)) continue
    next.set(key, q)
    points.set(key, p)
  }
  const floor: Poly[] = []
  const seen = new Set<string>()
  for (const start of next.keys()) {
    if (seen.has(start)) continue
    const ring: P[] = []
    let key = start
    let closed = false
    while (!seen.has(key)) {
      seen.add(key)
      ring.push(points.get(key)!)
      const q = next.get(key)
      if (!q) break
      key = keyOf(q)
      if (key === start) closed = true
    }
    if (!closed || ring.length < 3) continue
    const ids = triangulate(ring.map((q) => [q[0], q[2]] as const))
    for (let i = 0; i + 2 < ids.length; i += 3) {
      const tri: Poly = [ids[i]!, ids[i + 1]!, ids[i + 2]!].map((k) => add(ring[k]!, [0, 1, 0], low - ring[k]![1] + FLOOR_UP))
      if (normalOf(tri)[1] < 0) tri.reverse()
      floor.push(tri)
    }
  }
  return floor
}

/** `poly` moved `WALL_THICKNESS` against its normal and turned over: the inner face of a wall or roof. */
export function innerOf(poly: Poly, normal: P): Poly {
  return poly.map((p) => sub(p, [normal[0] * WALL_THICKNESS, normal[1] * WALL_THICKNESS, normal[2] * WALL_THICKNESS])).reverse()
}

const memo = new WeakMap<CollisionMesh, Map<number, Facade>>()

/** `facadeOf`, remembered for the look and the building. */
export function facadeFor(look: CollisionMesh, range: LookRange): Facade {
  let byBuilding = memo.get(look)
  if (!byBuilding) {
    byBuilding = new Map()
    memo.set(look, byBuilding)
  }
  let facade = byBuilding.get(range.first)
  if (!facade) {
    facade = facadeOf(look, range)
    byBuilding.set(range.first, facade)
  }
  return facade
}
