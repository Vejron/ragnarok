import { BUILDING_CABIN, BUILDING_FARMHOUSE, BUILDING_PLAIN, BUILDING_SHELTER, BUILDING_VILLA, type CollisionMesh } from '@/world'
import type { P, RGB } from './geom'
import { FRAME_RGB, type LookRange, type Opening } from './windows'

/*
 * The small parts that make a house a house, worked out from its look (`Tile.look`) and its
 * openings: a chimney on its ridge, and over its front door a porch on two posts (an old house's, a
 * cabin's) or a flat canopy (a villa's); in a shelter a plank bed, and a fire ring in front of it. Only drawn, the same on every client; nothing collides
 * with them. A hurt house keeps each while the cell it hangs from (`anchor`) stands. Pure, so it
 * runs in the tile worker too.
 */

/** A chimney: how far it stands over the roof under it and reaches into the roof under its lowest corner, and half its length along the ridge and half its width across, metres. */
const CHIMNEY_UP = 0.9
const CHIMNEY_DOWN = 0.6
const CHIMNEY_HALF: readonly [number, number] = [0.4, 0.3]
/** How far under the roof the cell it stands on is judged, metres: inside the top cell. */
const CHIMNEY_ROOTED = 0.15
/** Where along the ridge it stands, from one end. */
const CHIMNEY_ALONG = 0.35
/** How much roof it keeps around it, metres, so it never stands out past the wall under an eave. */
const CHIMNEY_CLEAR = 0.15
/** The steps it is moved down the roof in, and the furthest, metres, when its ridge is the roof's edge (a shed roof's high eave). */
const CHIMNEY_STEP = 0.1
const CHIMNEY_SLIDE = 3.0
/** A roof flatter than this from its lowest corner to its highest, metres, has no ridge to put one on. */
const PITCHED = 1.0
/** A porch: its width, how far it stands out from the wall, how far over the door's top it meets the wall, how far it slopes down to its front, and its posts' thickness, metres. */
const PORCH: readonly [number, number, number, number, number] = [2.0, 1.4, 0.45, 0.25, 0.12]
/** A canopy: its width, depth and height over the door's top, and its thickness, metres. */
const CANOPY: readonly [number, number, number, number] = [1.8, 0.9, 0.15, 0.1]

/** A shelter's bed: how high its planks stand over the floor, and how much of the floor it fills from the back wall. */
const BED: readonly [number, number] = [0.4, 0.65]
/** How far the bed keeps in from the outside of the walls, metres: past their thickness. */
const BED_CLEAR = 0.2
/** A shelter's fire ring: how far its middle is out from the open side, its radius, how many stones, and a stone's half size, metres. */
const FIRE: readonly [number, number, number, number] = [1.7, 0.55, 9, 0.11]

/** What a quad of a detail is painted as: brick, the house's roof, white boards, bare planks, stone, or charred wood. */
export type DetailPaint = 'chimney' | 'roof' | 'trim' | 'planks' | 'stone' | 'char'

export interface DetailQuad {
  corners: [P, P, P, P]
  /** The way it faces. */
  normal: P
  paint: DetailPaint
  /** A point just inside the house's shell that the detail stands on: the roof under a chimney, the wall over a door. It goes when that cell does. */
  anchor: P
}

/** Linear colours: a chimney's brick, and the trim (posts, the underside of a porch). */
export const CHIMNEY_RGB: RGB = [0.19, 0.075, 0.05]
export const DETAIL_TRIM_RGB: RGB = FRAME_RGB
const PLANKS_RGB: RGB = [0.2, 0.12, 0.06]
const STONE_RGB: RGB = [0.16, 0.16, 0.15]
const CHAR_RGB: RGB = [0.025, 0.02, 0.018]

/**
 * How a detail quad painted `paint` is drawn: its colour (linear) and, where it has one, its pattern
 * (`Surface` in `buildingmesh.ts`: `brick`, `planks`, or the house's `roof`, else 0 for plain).
 */
export function detailColour(paint: DetailPaint, roof: RGB, surfaces: { roof: number; brick: number; planks: number }): { rgb: RGB; surface: number } {
  switch (paint) {
    case 'chimney':
      return { rgb: CHIMNEY_RGB, surface: surfaces.brick }
    case 'roof':
      return { rgb: roof, surface: surfaces.roof }
    case 'planks':
      return { rgb: PLANKS_RGB, surface: surfaces.planks }
    case 'stone':
      return { rgb: STONE_RGB, surface: 0 }
    case 'char':
      return { rgb: CHAR_RGB, surface: 0 }
    default:
      return { rgb: DETAIL_TRIM_RGB, surface: 0 }
  }
}

const CHIMNEYED: ReadonlySet<number> = new Set([BUILDING_PLAIN, BUILDING_FARMHOUSE, BUILDING_VILLA, BUILDING_CABIN])

/** The five faces of a box that show: its four sides and its top. `u` is the unit along it over the ground. */
function box(out: DetailQuad[], centre: readonly [number, number], u: readonly [number, number], halfAlong: number, halfAcross: number, y0: number, y1: number, paint: DetailPaint, anchor: P) {
  const v = [-u[1], u[0]] as const
  const at = (a: number, b: number, y: number): P => [centre[0] + u[0] * a + v[0] * b, y, centre[1] + u[1] * a + v[1] * b]
  const sides: [number, number, number, number, P][] = [
    [-halfAlong, -halfAcross, halfAlong, -halfAcross, [-v[0], 0, -v[1]]],
    [halfAlong, halfAcross, -halfAlong, halfAcross, [v[0], 0, v[1]]],
    [halfAlong, -halfAcross, halfAlong, halfAcross, [u[0], 0, u[1]]],
    [-halfAlong, halfAcross, -halfAlong, -halfAcross, [-u[0], 0, -u[1]]],
  ]
  for (const [a0, b0, a1, b1, normal] of sides) out.push({ corners: [at(a0, b0, y0), at(a1, b1, y0), at(a1, b1, y1), at(a0, b0, y1)], normal, paint, anchor })
  out.push({ corners: [at(-halfAlong, -halfAcross, y1), at(halfAlong, -halfAcross, y1), at(halfAlong, halfAcross, y1), at(-halfAlong, halfAcross, y1)], normal: [0, 1, 0], paint, anchor })
}

/** The height of the roof triangles `roof` (flat x, y, z corners, three to a triangle) over the ground point (`x`, `z`), the highest where they overlap, or null where there is no roof. */
function roofAt(roof: readonly P[], x: number, z: number): number | null {
  let best: number | null = null
  for (let i = 0; i + 2 < roof.length; i += 3) {
    const [a, b, c] = [roof[i]!, roof[i + 1]!, roof[i + 2]!]
    const det = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2])
    if (Math.abs(det) < 1e-9) continue
    const l0 = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / det
    const l1 = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / det
    const l2 = 1 - l0 - l1
    if (l0 < -1e-6 || l1 < -1e-6 || l2 < -1e-6) continue
    const y = l0 * a[1] + l1 * b[1] + l2 * c[1]
    if (best === null || y > best) best = y
  }
  return best
}

/** The chimney, porch and canopy of building `range` of `look`, whose openings are `openings`. */
export function detailsOf(look: CollisionMesh, range: LookRange, openings: readonly Opening[]): DetailQuad[] {
  const out: DetailQuad[] = []
  const kind = range.kind ?? BUILDING_PLAIN
  if (kind === BUILDING_SHELTER) return shelterDetails(look, range)
  if (!CHIMNEYED.has(kind)) return out
  // The ridge: the roof's highest corners, and the two of them furthest apart.
  const { vertices, triangles } = look
  let [low, high] = [Infinity, -Infinity]
  const corners: P[] = []
  for (let t = range.first + range.walls; t < range.first + range.walls + range.roof; t++) {
    for (let k = 0; k < 3; k++) {
      const v = triangles[t * 3 + k]! * 3
      const p: P = [vertices[v]!, vertices[v + 1]!, vertices[v + 2]!]
      corners.push(p)
      low = Math.min(low, p[1])
      high = Math.max(high, p[1])
    }
  }
  if (high - low >= PITCHED) {
    const top = corners.filter((p) => p[1] > high - 0.05)
    let [from, to] = [top[0]!, top[0]!]
    let longest = 0
    for (const p of top) {
      for (const q of top) {
        const d = Math.hypot(q[0] - p[0], q[2] - p[2])
        // Of two as long, the same pair whatever order the corners come in.
        if (d > longest + 1e-6) [from, to, longest] = [p, q, d]
      }
    }
    if (from[0] > to[0] || (from[0] === to[0] && from[2] > to[2])) [from, to] = [to, from]
    const u: readonly [number, number] = longest > 0.5 ? [(to[0] - from[0]) / longest, (to[2] - from[2]) / longest] : [1, 0]
    const along = longest > 2 * CHIMNEY_HALF[0] + 1 ? CHIMNEY_ALONG * longest : longest / 2
    const v = [-u[1], u[0]] as const
    // The roof under each corner of its foot (with a little roof kept around it), or null where one is off the roof.
    const [ha, hb] = [CHIMNEY_HALF[0] + CHIMNEY_CLEAR, CHIMNEY_HALF[1] + CHIMNEY_CLEAR]
    const under = (c: readonly [number, number]): number[] | null => {
      const ys: number[] = []
      for (const [a, b] of [[0, 0], [-ha, -hb], [ha, -hb], [ha, hb], [-ha, hb]] as const) {
        const y = roofAt(corners, c[0] + u[0] * a + v[0] * b, c[1] + u[1] * a + v[1] * b)
        if (y === null) return null
        ys.push(y)
      }
      return ys
    }
    // On the ridge; where the ridge is the roof's edge (a shed roof's), down the roof until it stands wholly on it.
    const ridge = [from[0] + u[0] * along, from[2] + u[1] * along] as const
    let placed: { centre: readonly [number, number]; ys: number[] } | null = null
    for (let slide = 0; !placed && slide <= CHIMNEY_SLIDE + 1e-6; slide += CHIMNEY_STEP) {
      for (const side of slide === 0 ? [0] : [1, -1]) {
        const centre = [ridge[0] + v[0] * slide * side, ridge[1] + v[1] * slide * side] as const
        const ys = under(centre)
        if (ys) {
          placed = { centre, ys }
          break
        }
      }
    }
    if (placed) {
      const { centre, ys } = placed
      // It rises over the roof's highest point under it, and reaches into the roof under its lowest corner.
      box(out, centre, u, CHIMNEY_HALF[0], CHIMNEY_HALF[1], Math.min(...ys) - CHIMNEY_DOWN, Math.max(...ys) + CHIMNEY_UP, 'chimney', [centre[0], ys[0]! - CHIMNEY_ROOTED, centre[1]])
    }
  }
  // Over the front door.
  const door = openings.find((o) => o.door)
  if (!door || kind === BUILDING_PLAIN) return out
  const [a, b, , d] = door.corners
  const width = Math.hypot(b[0] - a[0], b[2] - a[2]) || 1
  const [ux, uz] = [(b[0] - a[0]) / width, (b[2] - a[2]) / width]
  const n = door.normal
  // `s` along the wall from the door's middle, `o` out from the wall.
  const at = (s: number, o: number, y: number): P => [(a[0] + b[0]) / 2 + ux * s + n[0] * o, y, (a[2] + b[2]) / 2 + uz * s + n[2] * o]
  // Both hang from the wall over the door, a hair inside it.
  const anchor = at(0, -0.01, d[1] + 0.1)
  if (kind === BUILDING_VILLA) {
    const [w, depth, over, thick] = CANOPY
    const [y0, y1] = [d[1] + over, d[1] + over + thick]
    out.push({ corners: [at(-w / 2, 0, y1), at(w / 2, 0, y1), at(w / 2, depth, y1), at(-w / 2, depth, y1)], normal: [0, 1, 0], paint: 'trim', anchor })
    out.push({ corners: [at(-w / 2, 0, y0), at(w / 2, 0, y0), at(w / 2, depth, y0), at(-w / 2, depth, y0)], normal: [0, -1, 0], paint: 'trim', anchor })
    out.push({ corners: [at(-w / 2, depth, y0), at(w / 2, depth, y0), at(w / 2, depth, y1), at(-w / 2, depth, y1)], normal: [n[0], 0, n[2]], paint: 'trim', anchor })
    return out
  }
  const [w, depth, over, drop, post] = PORCH
  const [back, front] = [d[1] + over, d[1] + over - drop]
  out.push({ corners: [at(-w / 2, 0, back), at(w / 2, 0, back), at(w / 2, depth, front), at(-w / 2, depth, front)], normal: [0, 1, 0], paint: 'roof', anchor })
  out.push({ corners: [at(-w / 2, 0, back - 0.06), at(w / 2, 0, back - 0.06), at(w / 2, depth, front - 0.06), at(-w / 2, depth, front - 0.06)], normal: [0, -1, 0], paint: 'trim', anchor })
  out.push({ corners: [at(-w / 2, depth, front - 0.06), at(w / 2, depth, front - 0.06), at(w / 2, depth, front), at(-w / 2, depth, front)], normal: [n[0], 0, n[2]], paint: 'trim', anchor })
  // Its two posts, from under the door's foot (the ground may fall away) to the roof.
  for (const side of [-1, 1]) {
    const p = at((side * (w - post)) / 2, depth - post / 2, 0)
    box(out, [p[0], p[2]], [ux, uz], post / 2, post / 2, a[1] - 0.6, front - 0.06, 'trim', anchor)
  }
  return out
}

/** The triangles `details` take to draw. */
export function detailTriangles(details: readonly DetailQuad[]): number {
  return details.length * 2
}

/**
 * A shelter's plank bed along its back wall and its fire ring out in front of its open side
 * (`range.front`). The bed stands on the floor and hangs from the back wall's lowest cell; the ring
 * goes with the shelter, from the same cell.
 */
function shelterDetails(look: CollisionMesh, range: LookRange): DetailQuad[] {
  const out: DetailQuad[] = []
  const footprint = range.footprint
  const front = range.front ?? [0, 0]
  const length = Math.hypot(front[0], front[1])
  if (!footprint || footprint.length < 6 || length === 0) return out
  const n = [front[0] / length, front[1] / length] as const
  const u = [-n[1], n[0]] as const
  // The floor: the walls' lowest corner.
  let floor = Infinity
  for (let t = range.first; t < range.first + range.walls; t++) {
    for (let k = 0; k < 3; k++) floor = Math.min(floor, look.vertices[look.triangles[t * 3 + k]! * 3 + 1]!)
  }
  if (!Number.isFinite(floor)) return out
  // The footprint along the way it faces (`n`) and across it (`u`), metres.
  let [back, open, left, right] = [Infinity, -Infinity, Infinity, -Infinity]
  for (let i = 0; i + 1 < footprint.length; i += 2) {
    const [x, z] = [footprint[i]! / 100, footprint[i + 1]! / 100]
    const along = x * n[0] + z * n[1]
    const across = x * u[0] + z * u[1]
    ;[back, open, left, right] = [Math.min(back, along), Math.max(open, along), Math.min(left, across), Math.max(right, across)]
  }
  const at = (along: number, across: number) => [n[0] * along + u[0] * across, n[1] * along + u[1] * across] as const
  const mid = (left + right) / 2
  const anchorAt = at(back + 0.05, mid)
  const anchor: P = [anchorAt[0], floor + 0.3, anchorAt[1]]
  // The bed: planks over the floor from the back wall out over most of it, inside the walls
  // whatever shape the footprint is, the outline cut off towards the open side.
  const [bedHigh, bedShare] = BED
  const outline: [number, number][] = []
  for (let i = 0; i + 1 < footprint.length; i += 2) outline.push([footprint[i]! / 100, footprint[i + 1]! / 100])
  const bed = shrink(cutOff(outline, n, back + (open - back) * bedShare), BED_CLEAR)
  if (bed.length >= 3) {
    const top = floor + bedHigh
    const corner = (p: readonly [number, number], y: number): P => [p[0], y, p[1]]
    for (let k = 1; k + 1 < bed.length; k++) {
      out.push({ corners: [corner(bed[0]!, top), corner(bed[k]!, top), corner(bed[k + 1]!, top), corner(bed[k + 1]!, top)], normal: [0, 1, 0], paint: 'planks', anchor })
    }
    const centre = bed.reduce<[number, number]>((c, p) => [c[0] + p[0] / bed.length, c[1] + p[1] / bed.length], [0, 0])
    for (let k = 0; k < bed.length; k++) {
      const [a, b] = [bed[k]!, bed[(k + 1) % bed.length]!]
      const length = Math.hypot(b[0] - a[0], b[1] - a[1])
      if (length < 1e-3) continue
      let side: P = [(b[1] - a[1]) / length, 0, (a[0] - b[0]) / length]
      if (side[0] * ((a[0] + b[0]) / 2 - centre[0]) + side[2] * ((a[1] + b[1]) / 2 - centre[1]) < 0) side = [-side[0], 0, -side[2]]
      out.push({ corners: [corner(a, floor), corner(b, floor), corner(b, top), corner(a, top)], normal: side, paint: 'planks', anchor })
    }
  }
  // The fire ring: stones round, and two charred logs across each other in it.
  const [ahead, radius, stones, half] = FIRE
  const ring = at(open + ahead, mid)
  for (let k = 0; k < stones; k++) {
    const a = (k / stones) * Math.PI * 2
    const c = [ring[0] + Math.cos(a) * radius, ring[1] + Math.sin(a) * radius] as const
    box(out, c, [Math.cos(a), Math.sin(a)], half, half * 1.2, floor - 0.4, floor + half * 1.4, 'stone', anchor)
  }
  for (const turn of [Math.PI / 4, -Math.PI / 4]) {
    box(out, ring, [Math.cos(turn), Math.sin(turn)], radius * 0.7, 0.06, floor - 0.4, floor + 0.12, 'char', anchor)
  }
  return out
}

/** The convex polygon `ring` (`x, z`) cut off where it reaches past `limit` along the unit `n`. */
function cutOff(ring: readonly (readonly [number, number])[], n: readonly [number, number], limit: number): [number, number][] {
  const out: [number, number][] = []
  const along = (p: readonly [number, number]) => p[0] * n[0] + p[1] * n[1] - limit
  for (let k = 0; k < ring.length; k++) {
    const [a, b] = [ring[k]!, ring[(k + 1) % ring.length]!]
    const [da, db] = [along(a), along(b)]
    if (da <= 0) out.push([a[0], a[1]])
    if ((da < 0 && db > 0) || (da > 0 && db < 0)) {
      const t = da / (da - db)
      out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t])
    }
  }
  return out
}

/** The convex polygon `ring` with every side moved `by` metres in, or nothing where it is too small for that. */
function shrink(ring: readonly (readonly [number, number])[], by: number): [number, number][] {
  const n = ring.length
  if (n < 3) return []
  let area = 0
  for (let k = 0; k < n; k++) area += ring[k]![0] * ring[(k + 1) % n]![1] - ring[(k + 1) % n]![0] * ring[k]![1]
  const turn = area > 0 ? 1 : -1
  // Each side as a line moved in: a point on it and its direction.
  const lines = ring.map((a, k) => {
    const b = ring[(k + 1) % n]!
    const length = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1
    const d = [(b[0] - a[0]) / length, (b[1] - a[1]) / length] as const
    // Inwards is to the left of a counter-clockwise ring.
    const inward = [-d[1] * turn, d[0] * turn] as const
    return { p: [a[0] + inward[0] * by, a[1] + inward[1] * by] as const, d }
  })
  const out: [number, number][] = []
  for (let k = 0; k < n; k++) {
    const { p: p0, d: d0 } = lines[(k + n - 1) % n]!
    const { p: p1, d: d1 } = lines[k]!
    const cross = d0[0] * d1[1] - d0[1] * d1[0]
    if (Math.abs(cross) < 1e-6) {
      out.push([p1[0], p1[1]])
      continue
    }
    const t = ((p1[0] - p0[0]) * d1[1] - (p1[1] - p0[1]) * d1[0]) / cross
    out.push([p0[0] + d0[0] * t, p0[1] + d0[1] * t])
  }
  // Too small: the sides moved in have crossed over and turned the ring inside out.
  let after = 0
  for (let k = 0; k < n; k++) after += out[k]![0] * out[(k + 1) % n]![1] - out[(k + 1) % n]![0] * out[k]![1]
  return after * area > 0 ? out : []
}
