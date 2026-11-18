import { BUILDING_BRIDGE, type CollisionMesh } from '@/world'
import { add, centreOf, dot, keyOf, normalOf, splitPlane, sub, triangulate, unit, type P, type Poly, type RGB } from './geom'
import { REVEAL, openingsFor, type LookRange, type Opening } from './windows'
import { Part, insideFor, type InsidePiece } from '@/sim/interior'

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

/** A doorway is cut this far below its door's foot and above its top, metres. Mirrors `DOOR_SILL` and `DOOR_RISE` in `facade.rs`. */
export const DOOR_SILL = 0.5
export const DOOR_RISE = 0.3
/** What stands inside a house, in linear colours: floor boards, the plastered underside of a floor, the underside of a flight of stairs and its steps. */
export const INSIDE_RGB: Record<number, RGB> = {
  [Part.Floor]: [0.2, 0.135, 0.08],
  [Part.Ceiling]: [0.42, 0.4, 0.35],
  [Part.Ramp]: [0.12, 0.085, 0.055],
  [Part.Step]: [0.26, 0.18, 0.11],
}

export interface Facade {
  /** Outward-wound, the windows cut out, and the door too when the house has rooms. */
  walls: Poly[]
  roof: Poly[]
  /** Facing up, at the foot of the walls: the floor seen through the windows of a house without rooms. */
  floor: Poly[]
  /** What stands inside it (`insideFor`): its floors and stairs. Empty for a house without. */
  inside: readonly InsidePiece[]
  /** How far each corner of the walls and roof moves in to their inner faces (`insetOf`). */
  inset: Inset
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
  const inside = insideFor(look, range).pieces
  for (const o of openingsFor(look, range)) {
    if (!o.door) walls = cutWindow(walls, o)
    else if (inside.length > 0) walls = cutWindow(walls, doorway(o))
  }
  // A house with rooms has its ground floor among them, on its plinth. A bridge is solid concrete
  // with no floor in it: one at the foot of its walls would lie at every height its underside does,
  // and be left in the air when it falls.
  return { walls, roof, floor: inside.length > 0 || range.kind === BUILDING_BRIDGE ? [] : floorOf(walls), inside, inset: insetOf([...walls, ...roof]) }
}

/** The hole door `o` is in a house with rooms: from under its foot to the height of a step over its top, as the server cuts it. */
export function doorway(o: Opening): Opening {
  const [a, b, c, d] = o.corners
  return { ...o, corners: [[a[0], a[1] - DOOR_SILL, a[2]], [b[0], b[1] - DOOR_SILL, b[2]], [c[0], c[1] + DOOR_RISE, c[2]], [d[0], d[1] + DOOR_RISE, d[2]]] }
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

/**
 * `poly` moved `WALL_THICKNESS` in and turned over: the inner face of a wall or roof. A corner
 * `inset` knows (a corner of the facade as it stands) moves as every face round it does, so the
 * inner faces meet where walls and roof meet; any other moves straight in against `normal`.
 * `place` moves a corner of a hurt building where it is drawn, before it is moved in.
 */
export function innerOf(poly: Poly, normal: P, inset?: Inset, place?: (p: P) => P): Poly {
  const straight: P = [normal[0] * WALL_THICKNESS, normal[1] * WALL_THICKNESS, normal[2] * WALL_THICKNESS]
  return poly.map((p) => sub(place ? place(p) : p, inset?.get(keyOf(p)) ?? straight)).reverse()
}

/** How far each corner of a facade moves in to the inner faces (by `keyOf`), metres. */
export type Inset = ReadonlyMap<string, P>

/** Two faces whose normals are this close are one plane at a corner (about 2.5°). */
const SAME_PLANE = 0.999
/** Furthest a corner moves in, in wall thicknesses: a sharp ridge or spire is not followed out to its point. */
const MOST_INSET = 3

/**
 * How far each corner of `faces` (wound outwards) moves in to the inner faces: the point
 * `WALL_THICKNESS` behind every face round it at once, so the inner faces of walls meeting at a
 * corner, inner or outer, or a wall meeting the roof, meet edge to edge and leave no slit to see
 * daylight through. Each face's normal is taken once for all its corners.
 */
export function insetOf(faces: readonly Poly[]): Map<string, P> {
  const planes = new Map<string, P[]>()
  for (const poly of faces) {
    const n = normalOf(poly)
    for (const p of poly) {
      const key = keyOf(p)
      const known = planes.get(key)
      if (!known) planes.set(key, [n])
      else if (!known.some((m) => dot(m, n) > SAME_PLANE)) known.push(n)
    }
  }
  const out = new Map<string, P>()
  for (const [key, normals] of planes) out.set(key, mitre(normals))
  return out
}

/** The shortest move `d` with `d · n = WALL_THICKNESS` for every one of `normals` (unit), as near as they allow, at most `MOST_INSET` thicknesses long. */
function mitre(normals: readonly P[]): P {
  const t = WALL_THICKNESS
  if (normals.length === 1) return [normals[0]![0] * t, normals[0]![1] * t, normals[0]![2] * t]
  // Least squares, (Nᵀ N + λ I) d = Nᵀ t: a hair of λ picks the shortest move when the normals
  // span only a plane, as two walls' do, and the mitre lies in it.
  const a = [[1e-6, 0, 0], [0, 1e-6, 0], [0, 0, 1e-6]]
  const b = [0, 0, 0]
  for (const n of normals) {
    for (let i = 0; i < 3; i++) {
      b[i]! += n[i]! * t
      for (let j = 0; j < 3; j++) a[i]![j]! += n[i]! * n[j]!
    }
  }
  const d = solve3(a, b)
  const length = Math.hypot(d[0], d[1], d[2])
  const most = MOST_INSET * t
  return length > most ? [d[0] * most / length, d[1] * most / length, d[2] * most / length] : d
}

/** `x` with `a x = b`, by Cramer's rule. */
function solve3(a: number[][], b: number[]): P {
  const det = (m: number[][]) => m[0]![0]! * (m[1]![1]! * m[2]![2]! - m[1]![2]! * m[2]![1]!) - m[0]![1]! * (m[1]![0]! * m[2]![2]! - m[1]![2]! * m[2]![0]!) + m[0]![2]! * (m[1]![0]! * m[2]![1]! - m[1]![1]! * m[2]![0]!)
  const whole = det(a)
  const column = (k: number) => a.map((row, i) => row.map((v, j) => (j === k ? b[i]! : v)))
  return [det(column(0)) / whole, det(column(1)) / whole, det(column(2)) / whole]
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
