import { BUILDING_CABIN, BUILDING_FARMHOUSE, BUILDING_PLAIN, BUILDING_VILLA, type CollisionMesh } from '@/world'
import type { P, RGB } from './geom'
import { FRAME_RGB, type LookRange, type Opening } from './windows'

/*
 * The small parts that make a house a house, worked out from its look (`Tile.look`) and its
 * openings: a chimney on its ridge, and over its front door a porch on two posts (an old house's, a
 * cabin's) or a flat canopy (a villa's). Only drawn, the same on every client; nothing collides
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

/** What a quad of a detail is painted as: brick, the house's roof, or white boards. */
export type DetailPaint = 'chimney' | 'roof' | 'trim'

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
