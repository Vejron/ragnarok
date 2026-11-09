import type { CollisionMesh } from '@/world'

/*
 * Windows and a door on a building's walls, worked out from its look (`Tile.look`), so a house reads
 * as a house: rows of windows a storey apart along each flat wall, and a door in the middle of the
 * ground row of its longest wall. Each is a white frame with a dark pane on it, a little proud of the
 * wall. Only drawn; the same for the same building on every client.
 */

/** A building's triangles in the look: `walls` from `first`, then `roof`. */
export interface LookRange {
  first: number
  walls: number
  roof: number
}

type P = [number, number, number]

/** One window or door: its four corners on the wall (counter-clockwise seen from outside), and the wall's outward normal. */
export interface Opening {
  corners: [P, P, P, P]
  normal: P
  door: boolean
}

const STOREY = 2.8
const SILL = 0.9
const WINDOW_HEIGHT = 1.3
const WINDOW_WIDTH = 1.0
/** Gap between window middles along a wall, about, metres. */
const PITCH = 2.8
/** Space kept clear round a window inside its wall, metres. */
const MARGIN = 0.35
const DOOR_WIDTH = 1.0
const DOOR_HEIGHT = 2.1
/** How far frames and panes stand out from the wall, metres, and how much wider a frame is than its pane. */
const FRAME_OUT = 0.025
export const PANE_OUT = 0.04
export const FRAME = 0.09
/** How deep a window's reveal goes into the wall, metres: the wall's thickness (`facade.ts`). */
export const REVEAL = 0.2

/** Linear colours: frames, a door's leaf, and the reveal round a window (the frame's, in shade). */
export const FRAME_RGB = [0.86, 0.84, 0.78] as const
export const DOOR_RGB = [0.11, 0.06, 0.035] as const
export const REVEAL_RGB = [0.7, 0.68, 0.63] as const

interface Plane {
  normal: P
  /** Along the wall, horizontal. */
  u: P
  tris: [number, number][][]
  u0: number
  u1: number
  v0: number
  v1: number
}

/** The windows and door of building `range` of `look`. */
export function openingsOf(look: CollisionMesh, range: LookRange): Opening[] {
  const { vertices, triangles } = look
  const corner = (v: number): P => [vertices[v * 3]!, vertices[v * 3 + 1]!, vertices[v * 3 + 2]!]
  const planes = new Map<string, Plane & { d: number }>()
  for (let t = range.first; t < range.first + range.walls; t++) {
    // Wound outwards: the look is wound inwards.
    const [a, b, c] = [corner(triangles[t * 3]!), corner(triangles[t * 3 + 2]!), corner(triangles[t * 3 + 1]!)]
    const e = [b[0] - a[0], b[1] - a[1], b[2] - a[2]]
    const f = [c[0] - a[0], c[1] - a[1], c[2] - a[2]]
    let n: P = [e[1]! * f[2]! - e[2]! * f[1]!, e[2]! * f[0]! - e[0]! * f[2]!, e[0]! * f[1]! - e[1]! * f[0]!]
    const l = Math.hypot(...n)
    if (l < 1e-6) continue
    n = [n[0] / l, n[1] / l, n[2] / l]
    // Upright walls only.
    if (Math.abs(n[1]) > 0.1) continue
    const h = Math.hypot(n[0], n[2])
    n = [n[0] / h, 0, n[2] / h]
    const d = n[0] * a[0] + n[2] * a[2]
    const key = `${Math.round(Math.atan2(n[2], n[0]) * 100)},${Math.round(d * 10)}`
    let plane = planes.get(key)
    if (!plane) {
      plane = { normal: n, u: [-n[2], 0, n[0]], tris: [], u0: Infinity, u1: -Infinity, v0: Infinity, v1: -Infinity, d }
      planes.set(key, plane)
    }
    const flat = [a, b, c].map((p) => [p[0] * plane!.u[0] + p[2] * plane!.u[2], p[1]] as [number, number])
    plane.tris.push(flat)
    for (const [u, v] of flat) {
      plane.u0 = Math.min(plane.u0, u)
      plane.u1 = Math.max(plane.u1, u)
      plane.v0 = Math.min(plane.v0, v)
      plane.v1 = Math.max(plane.v1, v)
    }
  }
  const inside = (plane: Plane, u: number, v: number) =>
    plane.tris.some(([a, b, c]) => {
      const s = (p: [number, number], q: [number, number]) => (q[0] - p[0]) * (v - p[1]) - (q[1] - p[1]) * (u - p[0])
      const [x, y, z] = [s(a!, b!), s(b!, c!), s(c!, a!)]
      return (x >= 0 && y >= 0 && z >= 0) || (x <= 0 && y <= 0 && z <= 0)
    })
  const fits = (plane: Plane, u0: number, u1: number, v0: number, v1: number) =>
    [
      [u0 - MARGIN, v0 - MARGIN],
      [u1 + MARGIN, v0 - MARGIN],
      [u1 + MARGIN, v1 + MARGIN],
      [u0 - MARGIN, v1 + MARGIN],
    ].every(([u, v]) => inside(plane, u!, v!))
  const sorted = [...planes.values()].sort((a, b) => (a.u1 - a.u0 === b.u1 - b.u0 ? a.d - b.d : b.u1 - b.u0 - (a.u1 - a.u0)))
  const out: Opening[] = []
  sorted.forEach((plane, rank) => {
    const length = plane.u1 - plane.u0
    if (length < WINDOW_WIDTH + 2 * MARGIN) return
    const count = Math.max(1, Math.round(length / PITCH))
    const step = length / count
    const toWorld = (u: number, v: number): P => [plane.u[0] * u + plane.normal[0] * plane.d, v, plane.u[2] * u + plane.normal[2] * plane.d]
    const push = (u0: number, u1: number, v0: number, v1: number, door: boolean) =>
      out.push({ corners: [toWorld(u0, v0), toWorld(u1, v0), toWorld(u1, v1), toWorld(u0, v1)], normal: plane.normal, door })
    // Rows a storey apart, as high as the wall reaches (`fits` keeps them off a gable's slopes).
    for (let storey = 0; plane.v0 + SILL + storey * STOREY + WINDOW_HEIGHT + MARGIN <= plane.v1; storey++) {
      const v0 = plane.v0 + SILL + storey * STOREY
      for (let i = 0; i < count; i++) {
        const mid = plane.u0 + (i + 0.5) * step
        // The longest wall's door: the middle of its ground row.
        if (rank === 0 && storey === 0 && i === Math.floor(count / 2) && fits(plane, mid - DOOR_WIDTH / 2, mid + DOOR_WIDTH / 2, plane.v0 + MARGIN, plane.v0 + DOOR_HEIGHT)) {
          push(mid - DOOR_WIDTH / 2, mid + DOOR_WIDTH / 2, plane.v0 + 0.02, plane.v0 + DOOR_HEIGHT, true)
          continue
        }
        const [u0, u1] = [mid - WINDOW_WIDTH / 2, mid + WINDOW_WIDTH / 2]
        if (fits(plane, u0, u1, v0, v0 + WINDOW_HEIGHT)) push(u0, u1, v0, v0 + WINDOW_HEIGHT, false)
      }
    }
  })
  return out
}

const memo = new WeakMap<CollisionMesh, Map<number, readonly Opening[]>>()

/** `openingsOf`, remembered for the look and the building: windows are looked up by their index in it. */
export function openingsFor(look: CollisionMesh, range: LookRange): readonly Opening[] {
  let byBuilding = memo.get(look)
  if (!byBuilding) {
    byBuilding = new Map()
    memo.set(look, byBuilding)
  }
  let openings = byBuilding.get(range.first)
  if (!openings) {
    openings = openingsOf(look, range)
    byBuilding.set(range.first, openings)
  }
  return openings
}

/** The triangles `openingQuads` draws for `openings`: a window's frame ring and reveal, a door's frame and leaf. */
export function openingTriangles(openings: readonly Opening[]): number {
  let n = 0
  for (const o of openings) n += o.door ? 4 : 16
  return n
}

/**
 * Calls `quad` for each quad of `openings` to draw, with the normal it faces: a window's frame, a ring
 * standing a little out from the wall round the hole cut for it, and the reveal going into the wall
 * from the hole's four sides; a door's frame and its leaf, standing out from the wall. The glass is
 * drawn apart (`glass.ts`), and the hole itself is cut out of the wall (`facade.ts`).
 */
export function openingQuads(openings: readonly Opening[], quad: (corners: [P, P, P, P], rgb: readonly [number, number, number], normal: P) => void) {
  for (const { corners, normal, door } of openings) {
    const [a, , c] = corners
    // Unit along the wall and up, from the corners.
    const along: P = [corners[1][0] - a[0], 0, corners[1][2] - a[2]]
    const al = Math.hypot(along[0], along[2]) || 1
    const [ux, uz] = [along[0] / al, along[2] / al]
    const grow = (out: number, by: number, bottom = by): [P, P, P, P] => {
      const shift = (p: P, du: number, dv: number): P => [p[0] + ux * du + normal[0] * out, p[1] + dv, p[2] + uz * du + normal[2] * out]
      return [shift(a, -by, -bottom), shift(corners[1], by, -bottom), shift(c, by, by), shift(corners[3], -by, by)]
    }
    if (door) {
      quad(grow(FRAME_OUT, FRAME, 0), FRAME_RGB, normal)
      quad(grow(FRAME_OUT + 0.015, 0), DOOR_RGB, normal)
      continue
    }
    const outer = grow(FRAME_OUT, FRAME)
    const inner = grow(FRAME_OUT, 0)
    for (let k = 0; k < 4; k++) quad([outer[k]!, outer[(k + 1) % 4]!, inner[(k + 1) % 4]!, inner[k]!], FRAME_RGB, normal)
    // The reveal: each side of the hole, going into the wall, facing into the opening.
    const back = (p: P): P => [p[0] - normal[0] * REVEAL, p[1], p[2] - normal[2] * REVEAL]
    const faces: P[] = [[0, 1, 0], [-ux, 0, -uz], [0, -1, 0], [ux, 0, uz]]
    for (let k = 0; k < 4; k++) {
      const [p, q] = [corners[k]!, corners[(k + 1) % 4]!]
      quad([p, q, back(q), back(p)], REVEAL_RGB, faces[k]!)
    }
  }
}

/** `corners` ordered so the soup's normal, `(c - b) × (a - b)` of each fan triangle, faces along `normal`. */
export function facing(corners: [P, P, P, P], normal: readonly number[]): [P, P, P, P] {
  const [a, b, c] = corners
  const e = [c[0] - b[0], c[1] - b[1], c[2] - b[2]]
  const f = [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
  const n = [e[1]! * f[2]! - e[2]! * f[1]!, e[2]! * f[0]! - e[0]! * f[2]!, e[0]! * f[1]! - e[1]! * f[0]!]
  return n[0]! * normal[0]! + n[1]! * normal[1]! + n[2]! * normal[2]! >= 0 ? corners : [corners[3], corners[2], corners[1], corners[0]]
}
