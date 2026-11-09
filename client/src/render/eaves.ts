import type { CollisionMesh } from '@/world'
import { add, centreOf, dot, edgeKey, keyOf, lerp, normalOf, sub, unit, type P, type RGB } from './geom'
import { FRAME_RGB, facing, type LookRange } from './windows'

/*
 * The overhang of a roof, worked out from a building's look (`Tile.look`): its roof ends flush with
 * the wall tops as the importer built it, so here each eave and gable verge is carried on past the
 * wall, continuing the roof's slope, and given the thickness of a real roof: a fascia board along its
 * outer edge and a soffit underneath. Only drawn, the same on every client; nothing collides with it.
 * Pure, so it runs in the tile worker too.
 */

/** How far a roof reaches past the wall under it, metres: along an eave, and at a gable's verge. */
export const EAVE = 0.4
export const VERGE = 0.25
/** The thickness shown at the roof's edge, metres. */
export const FASCIA = 0.15
/** Strips are cut to about this length along the edge, so a hole takes only its own stretch of the overhang. */
export const SEGMENT = 1.0
/** A building whose roof has more edges than this is a LiDAR facet soup: it gets no overhang. */
const MOST_STRIPS = 400
/** A roof edge is an eave or a verge only when a wall hangs at least this far below it, metres: a step up to a higher roof part is not. */
const WALL_BELOW = 0.3

/** Fascia and soffit: white boards, the soffit in shade. */
export const TRIM_RGB: RGB = FRAME_RGB
export const SOFFIT_RGB: RGB = [FRAME_RGB[0] * 0.8, FRAME_RGB[1] * 0.8, FRAME_RGB[2] * 0.8]

export type EavePart = 'top' | 'fascia' | 'soffit' | 'cap'

export interface EaveQuad {
  /** In order round the quad, wound so the soup's normal faces outwards (`facing`). */
  corners: [P, P, P, P]
  part: EavePart
}

/** One stretch of overhang along a roof edge from `a` to `b`. */
export interface EaveStrip {
  a: P
  b: P
  /** Horizontal unit normal, away from the building. */
  out: P
  /** The unit normal of the roof it continues. */
  roofNormal: P
  /** The middle of the roof edge: where the overhang hangs from, so the cell there says whether it stands. */
  mid: P
  quads: EaveQuad[]
}

interface Edge {
  a: P
  b: P
  out: P
  normal: P
  offset: number
}

/**
 * The overhang of building `range` of `look`: a strip for each stretch of roof edge that has a wall
 * hanging below it (so not a step between roof parts, nor a flat roof, which keeps its edge).
 */
export function eavesOf(look: CollisionMesh, range: LookRange): EaveStrip[] {
  const { vertices, triangles } = look
  const corner = (v: number): P => [vertices[v * 3]!, vertices[v * 3 + 1]!, vertices[v * 3 + 2]!]
  // Wound outwards: the look is wound inwards.
  const poly = (t: number): [P, P, P] => [corner(triangles[t * 3]!), corner(triangles[t * 3 + 2]!), corner(triangles[t * 3 + 1]!)]

  // The roof's sloping triangles, and the edges only one of them has: its outline.
  interface RoofTri { normal: P; centre: P }
  const edges = new Map<string, { a: P; b: P; tri: RoofTri; count: number }>()
  for (let t = range.first + range.walls; t < range.first + range.walls + range.roof; t++) {
    const p = poly(t)
    let normal = normalOf(p)
    // A flat roof (the importer's are exactly flat) keeps its edge; the shallowest fitted pitch is about 9°.
    if (Math.abs(normal[1]) > 0.9995 || Math.abs(normal[1]) < 0.05) continue
    // Up, whichever way the triangle winds.
    if (normal[1] < 0) normal = [-normal[0], -normal[1], -normal[2]]
    const tri = { normal, centre: centreOf(p) }
    for (let i = 0; i < 3; i++) {
      const [a, b] = [p[i]!, p[(i + 1) % 3]!]
      const key = edgeKey(a, b)
      const seen = edges.get(key)
      if (seen) seen.count++
      else edges.set(key, { a, b, tri, count: 1 })
    }
  }

  // The upright walls, as planes with their reach along and their lowest corner.
  interface Wall { n: P; d: number; u: P; u0: number; u1: number; low: number }
  const walls: Wall[] = []
  for (let t = range.first; t < range.first + range.walls; t++) {
    const p = poly(t)
    let n = normalOf(p)
    if (Math.abs(n[1]) > 0.1) continue
    const h = Math.hypot(n[0], n[2]) || 1
    n = [n[0] / h, 0, n[2] / h]
    const u: P = [-n[2], 0, n[0]]
    const along = p.map((c) => dot(c, u))
    walls.push({ n, d: dot(p[0], n), u, u0: Math.min(...along), u1: Math.max(...along), low: Math.min(p[0][1], p[1][1], p[2][1]) })
  }

  // The eaves and verges: outline edges with a wall hanging below them.
  const eaves: Edge[] = []
  for (const { a, b, tri, count } of edges.values()) {
    if (count !== 1) continue
    const length = Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2])
    if (length < 0.05) continue
    const top = Math.min(a[1], b[1])
    const wall = walls.find((w) => {
      if (Math.abs(dot(a, w.n) - w.d) > 0.02 || Math.abs(dot(b, w.n) - w.d) > 0.02) return false
      const [ua, ub] = [dot(a, w.u), dot(b, w.u)]
      if (Math.min(ua, ub) > w.u1 - 0.01 || Math.max(ua, ub) < w.u0 + 0.01) return false
      return w.low < top - WALL_BELOW
    })
    if (!wall) continue
    const mid = lerp(a, b, 0.5)
    let out: P = [wall.n[0], 0, wall.n[2]]
    if (dot(out, sub(mid, tri.centre)) < 0) out = [-out[0], 0, -out[2]]
    eaves.push({ a, b, out, normal: tri.normal, offset: Math.abs(a[1] - b[1]) < 0.02 ? EAVE : VERGE })
  }
  if (eaves.length === 0) return []

  // Where each corner of the outline goes: out by each edge's offset, mitred where two meet, and
  // lifted onto the roof's plane so the slope carries on.
  const touching = new Map<string, Edge[]>()
  for (const edge of eaves) for (const p of [edge.a, edge.b]) {
    const key = keyOf(p)
    const list = touching.get(key) ?? []
    list.push(edge)
    touching.set(key, list)
  }
  const lift = (p: P, dx: number, dz: number, edgesAt: readonly Edge[]): P => {
    let y = 0
    for (const { normal: n } of edgesAt) y += (p[1] - (n[0] * dx + n[2] * dz) / n[1]) / edgesAt.length
    return [p[0] + dx, y, p[2] + dz]
  }
  const moved = new Map<string, P>()
  for (const [key, list] of touching) {
    const p = keyOf(list[0]!.a) === key ? list[0]!.a : list[0]!.b
    const [e1, e2] = [list[0]!, list[1]]
    let [dx, dz] = [e1.out[0] * e1.offset, e1.out[2] * e1.offset]
    if (e2) {
      const [n1, n2] = [e1.out, e2.out]
      const det = n1[0] * n2[2] - n1[2] * n2[0]
      if (Math.abs(det) >= 0.05) {
        dx = (e1.offset * n2[2] - e2.offset * n1[2]) / det
        dz = (e2.offset * n1[0] - e1.offset * n2[0]) / det
        const most = 2.5 * Math.max(e1.offset, e2.offset)
        const l = Math.hypot(dx, dz)
        if (l > most) [dx, dz] = [(dx / l) * most, (dz / l) * most]
      }
    }
    moved.set(key, lift(p, dx, dz, list.slice(0, 2)))
  }

  const T: P = [0, FASCIA, 0]
  const strips: EaveStrip[] = []
  for (const edge of eaves) {
    const { a, b, out, normal, offset } = edge
    const [a1, b1] = [moved.get(keyOf(a))!, moved.get(keyOf(b))!]
    const pieces = Math.max(1, Math.ceil(Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]) / SEGMENT))
    const outerAt = (t: number): P => {
      if (t <= 0) return a1
      if (t >= 1) return b1
      const p = lerp(a, b, t)
      return lift(p, out[0] * offset, out[2] * offset, [edge])
    }
    for (let i = 0; i < pieces; i++) {
      const [t0, t1] = [i / pieces, (i + 1) / pieces]
      const [p0, p1] = [lerp(a, b, t0), lerp(a, b, t1)]
      const [q0, q1] = [outerAt(t0), outerAt(t1)]
      const quads: EaveQuad[] = [
        { corners: facing([p0, p1, q1, q0], normal), part: 'top' },
        { corners: facing([q0, q1, sub(q1, T), sub(q0, T)], out), part: 'fascia' },
        { corners: facing([sub(q0, T), sub(q1, T), sub(p1, T), sub(p0, T)], [-normal[0], -normal[1], -normal[2]]), part: 'soffit' },
      ]
      // An open end of the outline (the overhang stops against a higher roof part, say) is closed with a cap.
      const along = unit(sub(b, a))
      if (i === 0 && touching.get(keyOf(a))!.length === 1) quads.push({ corners: facing([p0, q0, sub(q0, T), sub(p0, T)], [-along[0], -along[1], -along[2]]), part: 'cap' })
      if (i === pieces - 1 && touching.get(keyOf(b))!.length === 1) quads.push({ corners: facing([p1, q1, sub(q1, T), sub(p1, T)], along), part: 'cap' })
      strips.push({ a: p0, b: p1, out, roofNormal: normal, mid: lerp(p0, p1, 0.5), quads })
    }
  }
  return strips.length > MOST_STRIPS ? [] : strips
}

const memo = new WeakMap<CollisionMesh, Map<number, readonly EaveStrip[]>>()

/** `eavesOf`, remembered for the look and the building (a hurt building is redrawn every frame it is hit). */
export function eavesFor(look: CollisionMesh, range: LookRange): readonly EaveStrip[] {
  let byBuilding = memo.get(look)
  if (!byBuilding) {
    byBuilding = new Map()
    memo.set(look, byBuilding)
  }
  let strips = byBuilding.get(range.first)
  if (!strips) {
    strips = eavesOf(look, range)
    byBuilding.set(range.first, strips)
  }
  return strips
}

/** The triangles `strips` take to draw. */
export function eaveTriangles(strips: readonly EaveStrip[]): number {
  let n = 0
  for (const strip of strips) n += strip.quads.length * 2
  return n
}

/** Calls `quad` for each quad of `strips`: the roof's own colour on top, trim on the fascia and caps, shaded trim under. */
export function eaveQuads(strips: readonly EaveStrip[], roof: RGB, quad: (corners: [P, P, P, P], rgb: RGB, part: EavePart) => void) {
  for (const strip of strips) for (const { corners, part } of strip.quads) quad(corners, part === 'top' ? roof : part === 'soffit' ? SOFFIT_RGB : TRIM_RGB, part)
}

/** Where strip `strip` hangs from, a hair inside the roof and inboard of its edge: the point whose cell decides whether it stands. */
export function eaveAnchor(strip: EaveStrip): P {
  return add(add(strip.mid, strip.out, -0.1), strip.roofNormal, -0.02)
}
