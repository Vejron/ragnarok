import type { CollisionMesh } from '@/world'
import { CELL_HP, type Shell } from '@/sim/carve'
import { openingQuads, openingsOf } from './windows'

/*
 * A hurt building drawn from its own walls and roof (`Tile.look`), not from its cubes. The cells are
 * still what is blasted and collided with; here they only say where the holes are. Each triangle near
 * a hole is sliced along the cell grid, the pieces lying in a cell that is gone are left out, the edge
 * of the hole is given a depth (the wall's cross-section), and the corners round the hole are pushed
 * about in the wall's plane so it is ragged, not square. Corners near a hole are darkened by soot, so
 * the burn fades smoothly across the wall rather than cube by cube.
 */

/** A building's triangles in the look: `walls` from `first`, then `roof`. */
export interface LookRange {
  first: number
  walls: number
  roof: number
}

export interface LookSoup {
  positions: Float32Array
  colors: Float32Array
  /** Four numbers a vertex, the first the surface pattern on it (`Surface` in buildingmesh.ts). */
  custom: Float32Array
}

/** The surface patterns of a building's walls and roof (`buildingSurfaces`). */
export interface Surfaces {
  wall: number
  roof: number
}

const PLAIN: Surfaces = { wall: 0, roof: 0 }

/** Broken rafters in a roof's hole: the share of its edges with one, their size and how far they reach (metres), and their colour. */
const RAFTER_CHANCE = 0.7
const RAFTER = [0.07, 0.11] as const
const RAFTER_REACH = [0.25, 0.75] as const
const RAFTER_RGB = [0.13, 0.075, 0.04] as const
/** Splinters at a wooden wall's hole: a few to an edge, thin, short, raw wood. */
const SPLINTERS = 2
const SPLINTER = [0.035, 0.02] as const
const SPLINTER_REACH = [0.08, 0.32] as const
const SPLINTER_RGB = [0.5, 0.36, 0.2] as const
/** Cracks out from a brick or rendered wall's hole: the share of its corners with one, segments, their length and the crack's width (metres), and how dark. */
const CRACK_CHANCE = 0.55
const CRACK_SEGMENTS = 6
const CRACK_STEP = [0.16, 0.38] as const
const CRACK_WIDTH = 0.028
const CRACK_SHADE = 0.22
/** The surface patterns that are wood (`Surface` Upright and Lap), and that crack (Brick, Render and Ashlar). */
const WOODEN = new Set([1, 2])
const CRACKING = new Set([3, 4, 8])

/** How deep the edge of a hole is drawn, metres: a wall, and a roof. */
const WALL_DEPTH = 0.3
const ROOF_DEPTH = 0.16
/** Most a corner round a hole is pushed in the plane of its wall, metres. */
const JITTER = 0.22
/** Soot: fully black within this of a hole's cell middle, gone past this, metres. */
const BURN_NEAR = 0.45
const BURN_FAR = 1.7
/** The share of its colour a fully burnt wall keeps. */
const SOOT = 0.12
/** Most chips drawn in a cell that has taken all but its last hit, and how dark they are. */
const CHIPS = 6
const CHIP_SHADE = 0.38
/** The cross-section of a wall: its colour pulled towards this grey, and darkened. */
const SECTION = [0.55, 0.5, 0.44] as const
const SECTION_MIX = 0.55
const SECTION_SHADE = 0.72
/** Cells round a triangle's box within which a hole slices it (so the soot has corners to fade over). */
const SLICE_REACH = 2
const EPS = 1e-4

type P = [number, number, number]
type Poly = P[]

/** A number in [0, 1) from a point, the same on every client. */
function hash(x: number, y: number, z: number, salt: number): number {
  const a = Math.sin(x * 127.1 + y * 311.7 + z * 74.7 + salt * 19.19) * 43758.5453
  return a - Math.floor(a)
}

/** The key of a point, to the millimetre. */
function keyOf(p: P): string {
  return `${Math.round(p[0] * 1000)},${Math.round(p[1] * 1000)},${Math.round(p[2] * 1000)}`
}

/** `poly` cut by the plane `axis = at` into the part below and the part above; either may be empty. */
function split(poly: Poly, axis: number, at: number): [Poly, Poly] {
  const below: Poly = []
  const above: Poly = []
  const side = (p: P) => (p[axis]! - at > EPS ? 1 : p[axis]! - at < -EPS ? -1 : 0)
  let allOn = true
  for (let i = 0; i < poly.length; i++) {
    const a = poly[i]!
    const b = poly[(i + 1) % poly.length]!
    const [sa, sb] = [side(a), side(b)]
    if (sa !== 0) allOn = false
    if (sa <= 0) below.push(a)
    if (sa >= 0) above.push(a)
    if (sa * sb < 0) {
      const t = (at - a[axis]!) / (b[axis]! - a[axis]!)
      const cut: P = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
      cut[axis] = at
      below.push(cut)
      above.push([...cut] as P)
    }
  }
  if (allOn) return [[], poly]
  return [below.length >= 3 ? below : [], above.length >= 3 ? above : []]
}

/** The unit normal `poly` winds towards, `(c - b) × (a - b)` of its first corners, as the soup draws it. */
function normalOf(poly: Poly): P {
  let [nx, ny, nz] = [0, 0, 0]
  // Newell's method: sound for any planar polygon, slivers included.
  for (let i = 0; i < poly.length; i++) {
    const a = poly[i]!
    const b = poly[(i + 1) % poly.length]!
    nx += (a[1] - b[1]) * (a[2] + b[2])
    ny += (a[2] - b[2]) * (a[0] + b[0])
    nz += (a[0] - b[0]) * (a[1] + b[1])
  }
  const l = Math.hypot(nx, ny, nz) || 1
  return [nx / l, ny / l, nz / l]
}

interface Piece {
  poly: Poly
  /** Outward unit normal. */
  normal: P
  roof: boolean
}

type RGB = readonly [number, number, number]

/** A building's grid, in metres. */
class Grid {
  readonly cell: number
  readonly origin: P
  constructor(readonly shell: Shell) {
    this.cell = shell.cell / 100
    this.origin = shell.origin.map((v) => v / 100) as P
  }
  of(v: number, axis: number): number {
    return Math.floor((v - this.origin[axis]!) / this.cell)
  }
  has(x: number, y: number, z: number): boolean {
    const d = this.shell.dims
    return x >= 0 && y >= 0 && z >= 0 && x < d[0] && y < d[1] && z < d[2]
  }
}

/**
 * The building's triangles, wound outwards (the look is wound as the collision mesh, inwards): those
 * further than `SLICE_REACH` cells from every cell of `near` whole, the rest sliced along the grid so
 * each piece lies in one cell, and split by whether `cut` holds for that cell.
 */
function slice(look: CollisionMesh, range: LookRange, grid: Grid, near: readonly (readonly number[])[], cut: (id: number) => boolean) {
  const { cell, origin, shell } = grid
  const whole: Piece[] = []
  const sliced: Piece[] = []
  const { vertices, triangles } = look
  const corner = (v: number): P => [vertices[v * 3]!, vertices[v * 3 + 1]!, vertices[v * 3 + 2]!]
  for (let t = range.first; t < range.first + range.walls + range.roof; t++) {
    const poly: Poly = [corner(triangles[t * 3]!), corner(triangles[t * 3 + 2]!), corner(triangles[t * 3 + 1]!)]
    const piece = { poly, normal: normalOf(poly), roof: t >= range.first + range.walls }
    const lo = [0, 1, 2].map((a) => grid.of(Math.min(poly[0]![a]!, poly[1]![a]!, poly[2]![a]!), a) - SLICE_REACH)
    const hi = [0, 1, 2].map((a) => grid.of(Math.max(poly[0]![a]!, poly[1]![a]!, poly[2]![a]!), a) + SLICE_REACH)
    const close = near.some((c) => c[0]! >= lo[0]! && c[0]! <= hi[0]! && c[1]! >= lo[1]! && c[1]! <= hi[1]! && c[2]! >= lo[2]! && c[2]! <= hi[2]!)
    ;(close ? sliced : whole).push(piece)
  }
  const kept: Piece[] = []
  const taken: Piece[] = []
  for (const piece of sliced) {
    let polys: Poly[] = [piece.poly]
    for (let axis = 0; axis < 3; axis++) {
      let lo = Infinity
      let hi = -Infinity
      for (const p of piece.poly) {
        lo = Math.min(lo, p[axis]!)
        hi = Math.max(hi, p[axis]!)
      }
      for (let k = Math.floor((lo - origin[axis]!) / cell) + 1; origin[axis]! + k * cell < hi; k++) {
        const at = origin[axis]! + k * cell
        const next: Poly[] = []
        for (const poly of polys) for (const part of split(poly, axis, at)) if (part.length >= 3) next.push(part)
        polys = next
      }
    }
    for (const poly of polys) {
      const c: P = [0, 0, 0]
      for (const p of poly) for (let a = 0; a < 3; a++) c[a]! += p[a]! / poly.length
      // A hair inside the wall, so a wall on a cell boundary is judged by the cell it is cut into.
      const [x, y, z] = [0, 1, 2].map((a) => grid.of(c[a]! - piece.normal[a]! * 0.01, a)) as P
      const inCut = grid.has(x, y, z) && cut(shell.index(x, y, z))
      ;(inCut || orphan(grid, x, y, z, cut) ? taken : kept).push({ poly, normal: piece.normal, roof: piece.roof })
    }
  }
  return { whole, kept, taken }
}

/**
 * Whether the wall in cell `(x, y, z)` is left with nothing to hang from: the cell is not the
 * building's own (another house was given it, or the cut missed it), and every cell of the
 * building round it is gone (`cut`). It goes with them rather than stay in the air.
 */
function orphan(grid: Grid, x: number, y: number, z: number, cut: (id: number) => boolean): boolean {
  const shell = grid.shell
  if (grid.has(x, y, z) && shell.isSolid(shell.index(x, y, z))) return false
  let lost = false
  for (let dy = -1; dy <= 1; dy++) for (let dz = -1; dz <= 1; dz++) for (let dx = -1; dx <= 1; dx++) {
    const [ax, ay, az] = [x + dx, y + dy, z + dz]
    if (!grid.has(ax, ay, az)) continue
    const id = shell.index(ax, ay, az)
    if (!shell.isSolid(id)) continue
    if (!cut(id)) return false
    lost = true
  }
  return lost
}

interface Rim {
  a: P
  b: P
  normal: P
  roof: boolean
  /** In the plane of its wall, across the edge into the hole. */
  inward: P
}

function centreOf(poly: Poly): P {
  const c: P = [0, 0, 0]
  for (const p of poly) for (let a = 0; a < 3; a++) c[a]! += p[a]! / poly.length
  return c
}

/**
 * The outline of `taken`: the sides no other piece of it shares, each to be drawn with a wall's
 * depth; and where each corner of the outline goes, pushed about in its wall's plane (not a corner
 * two planes share: the building's own edges stay straight). The push depends on the corner alone,
 * so a piece that falls out of a wall has the same ragged edge as the hole it leaves.
 */
function outline(taken: readonly Piece[], all: readonly Piece[]) {
  const edgeKey = (a: P, b: P) => {
    const [ka, kb] = [keyOf(a), keyOf(b)]
    return ka < kb ? `${ka}|${kb}` : `${kb}|${ka}`
  }
  const edges = new Map<string, number>()
  for (const { poly } of taken) for (let i = 0; i < poly.length; i++) {
    const key = edgeKey(poly[i]!, poly[(i + 1) % poly.length]!)
    edges.set(key, (edges.get(key) ?? 0) + 1)
  }
  const rims: Rim[] = []
  const corners = new Set<string>()
  for (const { poly, normal, roof } of taken) for (let i = 0; i < poly.length; i++) {
    const [a, b] = [poly[i]!, poly[(i + 1) % poly.length]!]
    if (edges.get(edgeKey(a, b)) !== 1) continue
    // Towards the middle of the piece that is gone, flattened into its plane.
    const mid = centreOf(poly)
    const d = [0, 1, 2].map((k) => mid[k]! - (a[k]! + b[k]!) / 2)
    const along = d[0]! * normal[0] + d[1]! * normal[1] + d[2]! * normal[2]
    const flat = [0, 1, 2].map((k) => d[k]! - normal[k]! * along)
    const fl = Math.hypot(...flat) || 1
    rims.push({ a, b, normal, roof, inward: flat.map((v) => v / fl) as P })
    corners.add(keyOf(a))
    corners.add(keyOf(b))
  }
  const planes = new Map<string, P[]>()
  for (const { poly, normal } of all) for (const p of poly) {
    const key = keyOf(p)
    if (!corners.has(key)) continue
    const list = planes.get(key) ?? []
    if (!list.some((n) => n[0] * normal[0] + n[1] * normal[1] + n[2] * normal[2] > 0.999)) list.push(normal)
    planes.set(key, list)
  }
  const moved = new Map<string, P>()
  for (const [key, normals] of planes) {
    if (normals.length !== 1) continue
    const n = normals[0]!
    const at = key.split(',').map(Number) as P
    // Two directions in the plane.
    const up: P = Math.abs(n[1]) > 0.9 ? [1, 0, 0] : [0, 1, 0]
    const t: P = [n[1] * up[2] - n[2] * up[1], n[2] * up[0] - n[0] * up[2], n[0] * up[1] - n[1] * up[0]]
    const tl = Math.hypot(...t) || 1
    const b: P = [n[1] * t[2] - n[2] * t[1], n[2] * t[0] - n[0] * t[2], n[0] * t[1] - n[1] * t[0]]
    const [u, v] = [(hash(at[0], at[1], at[2], 1) - 0.5) * 2 * JITTER, (hash(at[0], at[1], at[2], 2) - 0.5) * 2 * JITTER]
    moved.set(key, [0, 1, 2].map((a) => at[a]! / 1000 + (t[a]! / tl) * u + (b[a]! / tl) * v) as P)
  }
  const place = (p: P): P => moved.get(keyOf(p)) ?? p
  return { rims, place }
}

/** Soup being written: corners and a colour a corner. */
class Writer {
  readonly positions: number[] = []
  readonly colors: number[] = []
  readonly custom: number[] = []
  /** The surface pattern the next polygons are drawn with: only walls and roofs whole or cut keep theirs, not edges, chips or windows. */
  surface = 0
  constructor(readonly place: (p: P) => P, private readonly offset: P = [0, 0, 0]) {}

  /** `poly` as a fan, each corner in `base` times what `tone` gives it; `placed` when its corners are where they go already. */
  poly(poly: Poly, base: RGB, tone?: (p: P) => number, placed = false) {
    const at = placed ? poly : poly.map(this.place)
    const tones = at.map((p) => (tone ? tone(p) : 1))
    const [ox, oy, oz] = this.offset
    for (let i = 1; i + 1 < at.length; i++) {
      for (const k of [0, i, i + 1]) {
        const p = at[k]!
        this.positions.push(p[0] - ox, p[1] - oy, p[2] - oz)
        this.colors.push(base[0] * tones[k]!, base[1] * tones[k]!, base[2] * tones[k]!)
        this.custom.push(this.surface, 0, 0, 0)
      }
    }
  }

  /** The edges in `rims` given their depth, going in from the outer face, in the cross-section colour. */
  rims(rims: readonly Rim[], wall: RGB, roof: RGB, tone?: (p: P) => number) {
    const section = (c: RGB) => [0, 1, 2].map((a) => (c[a]! * (1 - SECTION_MIX) + SECTION[a]! * SECTION_MIX) * SECTION_SHADE) as unknown as RGB
    const [wallSection, roofSection] = [section(wall), section(roof)]
    for (const { a, b, normal, roof: isRoof } of rims) {
      const depth = isRoof ? ROOF_DEPTH : WALL_DEPTH
      const [pa, pb] = [this.place(a), this.place(b)]
      const [qa, qb] = [pa, pb].map((p) => [p[0] - normal[0] * depth, p[1] - normal[1] * depth, p[2] - normal[2] * depth] as P)
      // The quad faces into the hole; the material is double-sided, so its winding does not matter.
      this.poly([pa, pb, qb!, qa!], isRoof ? roofSection : wallSection, tone, true)
    }
  }

  soup(): LookSoup {
    return { positions: Float32Array.from(this.positions), colors: Float32Array.from(this.colors), custom: Float32Array.from(this.custom) }
  }
}

/**
 * Building `range` of `look` as it stands with the cells in `removed` gone and those in `dents`
 * worn, in `wall` and `roof` (linear `[r, g, b]`). Triangles far from any hole are drawn whole.
 */
export function lookFracture(look: CollisionMesh, range: LookRange, shell: Shell, removed: ReadonlySet<number>, dents: ReadonlyMap<number, number>, wall: RGB, roof: RGB, surfaces: Surfaces = PLAIN): LookSoup {
  const grid = new Grid(shell)
  const { cell, origin } = grid
  const near = [...removed, ...dents.keys()].map((id) => shell.coords(id))
  const { whole, kept, taken } = slice(look, range, grid, near, (id) => removed.has(id))
  const { rims, place } = outline(taken, [...kept, ...taken])

  // Soot and wear at a corner: by how near the nearest cell that is gone is, and how worn its own cell.
  const shade = (p: P): number => {
    const g = [0, 1, 2].map((a) => grid.of(p[a]!, a))
    let nearest = Infinity
    for (let dy = -2; dy <= 2; dy++) for (let dz = -2; dz <= 2; dz++) for (let dx = -2; dx <= 2; dx++) {
      const [x, y, z] = [g[0]! + dx, g[1]! + dy, g[2]! + dz]
      if (!grid.has(x, y, z) || !removed.has(shell.index(x, y, z))) continue
      const d = Math.hypot(origin[0] + (x + 0.5) * cell - p[0], origin[1] + (y + 0.5) * cell - p[1], origin[2] + (z + 0.5) * cell - p[2])
      nearest = Math.min(nearest, d)
    }
    let burn = Math.max(0, Math.min(1, (BURN_FAR - nearest) / (BURN_FAR - BURN_NEAR)))
    burn = Math.min(1, burn * (0.6 + 0.8 * hash(p[0], p[1], p[2], 3)))
    return 1 - burn * (1 - SOOT)
  }

  const out = new Writer(place)
  for (const piece of whole) {
    out.surface = piece.roof ? surfaces.roof : surfaces.wall
    out.poly(piece.poly, piece.roof ? roof : wall)
  }
  for (const piece of kept) {
    out.surface = piece.roof ? surfaces.roof : surfaces.wall
    out.poly(piece.poly, piece.roof ? roof : wall, shade)
  }
  out.surface = 0
  out.rims(rims, wall, roof, shade)
  // Whether `p` lies on what stands of a wall facing `normal`: within one of its pieces, in their plane.
  const standing = [...whole, ...kept]
  const onWall = (p: P, normal: P) =>
    standing.some((piece) => {
      if (dot(piece.normal, normal) < 0.999) return false
      const poly = piece.poly
      if (Math.abs(dot([p[0] - poly[0]![0], p[1] - poly[0]![1], p[2] - poly[0]![2]], normal)) > 0.03) return false
      let sign = 0
      for (let i = 0; i < poly.length; i++) {
        const [u, v] = [poly[i]!, poly[(i + 1) % poly.length]!]
        const c = dot(cross([v[0] - u[0], v[1] - u[1], v[2] - u[2]], [p[0] - u[0], p[1] - u[1], p[2] - u[2]]), normal)
        if (Math.abs(c) < 1e-9) continue
        if (sign === 0) sign = Math.sign(c)
        else if (Math.sign(c) !== sign) return false
      }
      return true
    })
  edgeDetail(out, rims, surfaces, wall, onWall)
  // Chips where rounds struck: a few ragged marks on the wall in each worn cell, more as it wears.
  if (dents.size > 0) {
    const inCell = new Map<number, Piece>()
    for (const piece of kept) {
      const c: P = [0, 0, 0]
      for (const p of piece.poly) for (let a = 0; a < 3; a++) c[a]! += p[a]! / piece.poly.length
      const [x, y, z] = [0, 1, 2].map((a) => grid.of(c[a]! - piece.normal[a]! * 0.01, a)) as P
      if (!grid.has(x, y, z)) continue
      const id = shell.index(x, y, z)
      if (dents.has(id) && !inCell.has(id)) inCell.set(id, piece)
    }
    for (const [id, damage] of dents) {
      const piece = inCell.get(id)
      if (!piece) continue
      chips(out, piece, id, Math.min(CHIPS, Math.ceil((damage / CELL_HP) * CHIPS)), piece.roof ? roof : wall)
    }
  }
  // Windows and doors, but not those within a cell of a hole: their glass is blown out.
  const nearHole = (corners: readonly P[]) => {
    const lo = [0, 1, 2].map((a) => grid.of(Math.min(...corners.map((p) => p[a]!)), a) - 1)
    const hi = [0, 1, 2].map((a) => grid.of(Math.max(...corners.map((p) => p[a]!)), a) + 1)
    for (let y = lo[1]!; y <= hi[1]!; y++) for (let z = lo[2]!; z <= hi[2]!; z++) for (let x = lo[0]!; x <= hi[0]!; x++) if (grid.has(x, y, z) && removed.has(shell.index(x, y, z))) return true
    return false
  }
  const openings = removed.size === 0 ? openingsOf(look, range) : openingsOf(look, range).filter((o) => !nearHole(o.corners))
  openingQuads(openings, (corners, rgb) => out.poly(corners, rgb, shade, true))
  return out.soup()
}

const add = (a: readonly number[], b: readonly number[], k = 1): P => [a[0]! + b[0]! * k, a[1]! + b[1]! * k, a[2]! + b[2]! * k]
const dot = (a: readonly number[], b: readonly number[]) => a[0]! * b[0]! + a[1]! * b[1]! + a[2]! * b[2]!
const cross = (a: readonly number[], b: readonly number[]): P => [a[1]! * b[2]! - a[2]! * b[1]!, a[2]! * b[0]! - a[0]! * b[2]!, a[0]! * b[1]! - a[1]! * b[0]!]
const unit = (a: readonly number[]): P => {
  const l = Math.hypot(a[0]!, a[1]!, a[2]!) || 1
  return [a[0]! / l, a[1]! / l, a[2]! / l]
}

/**
 * A stick from `from` to `to`, `w` by `h` across (`h` along `normal`), its far end `taper` as thick:
 * a rafter or a splinter, its four sides and its broken end.
 */
function stick(out: Writer, from: P, to: P, normal: P, w: number, h: number, taper: number, rgb: RGB) {
  const e = unit([to[0] - from[0], to[1] - from[1], to[2] - from[2]])
  const s1 = unit(cross(e, normal))
  const s2 = cross(s1, e)
  const ring = (at: P, k: number) => [[-1, -1], [1, -1], [1, 1], [-1, 1]].map(([i, j]) => add(add(at, s1, (i! * w * k) / 2), s2, (j! * h * k) / 2))
  const [a, b] = [ring(from, 1), ring(to, taper)]
  for (let i = 0; i < 4; i++) out.poly([a[i]!, a[(i + 1) % 4]!, b[(i + 1) % 4]!, b[i]!], rgb, undefined, true)
  out.poly(b, rgb, undefined, true)
}

/**
 * What shows at a hole's edge: broken rafters reaching into a roof's hole along the slope, and, by
 * the walls' pattern, splinters at a wooden wall's or cracks running out across a brick or rendered
 * one. Cracks keep to the wall: each step must land on what is left of it (`onWall`).
 */
function edgeDetail(out: Writer, rims: readonly Rim[], surfaces: Surfaces, wall: RGB, onWall: (p: P, normal: P) => boolean) {
  const crackRgb: RGB = [wall[0] * CRACK_SHADE, wall[1] * CRACK_SHADE, wall[2] * CRACK_SHADE]
  for (const rim of rims) {
    const { normal, inward } = rim
    const [a, b] = [out.place(rim.a), out.place(rim.b)]
    const edge = Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2])
    if (edge < 0.05) continue
    const seed = centreOf([rim.a, rim.b])
    const h = (salt: number) => hash(seed[0], seed[1], seed[2], salt)
    const at = (t: number) => add(a, [b[0] - a[0], b[1] - a[1], b[2] - a[2]], t)
    if (rim.roof) {
      if (h(11) > RAFTER_CHANCE) continue
      // Rafters run down the slope, and only an edge across them shows their ends; a flat roof's joists, whichever way the edge goes.
      const slope = Math.abs(normal[1]) > 0.97 ? inward : unit(cross(cross([0, 1, 0], normal), normal))
      const across = dot(slope, inward)
      if (Math.abs(across) < 0.3) continue
      const dir: P = across > 0 ? slope : [-slope[0], -slope[1], -slope[2]]
      const from = add(at(0.2 + 0.6 * h(12)), normal, -ROOF_DEPTH * 0.5 - RAFTER[1] / 2)
      const reach = RAFTER_REACH[0] + (RAFTER_REACH[1] - RAFTER_REACH[0]) * h(13)
      // Snapped and sagging a little at its end.
      const to = add(add(from, dir, reach), [0, -1, 0], reach * 0.25 * h(14))
      stick(out, from, to, normal, RAFTER[0], RAFTER[1], 0.8, RAFTER_RGB)
      continue
    }
    if (WOODEN.has(surfaces.wall)) {
      for (let k = 0; k < SPLINTERS; k++) {
        if (h(20 + k) > 0.6) continue
        const from = add(at(0.15 + 0.7 * h(22 + k)), normal, -WALL_DEPTH * (0.15 + 0.6 * h(24 + k)))
        const reach = SPLINTER_REACH[0] + (SPLINTER_REACH[1] - SPLINTER_REACH[0]) * h(26 + k)
        // Into the hole, splayed a little out of the wall's plane and along the edge.
        const dir = unit(add(add(inward, normal, (h(28 + k) - 0.3) * 0.8), [b[0] - a[0], b[1] - a[1], b[2] - a[2]], (h(30 + k) - 0.5) / edge))
        stick(out, from, add(from, dir, reach), normal, SPLINTER[0], SPLINTER[1], 0.15, SPLINTER_RGB)
      }
      continue
    }
    if (!CRACKING.has(surfaces.wall) || h(40) > CRACK_CHANCE) continue
    // From the edge's start, out across the wall, bending as it goes and narrowing to nothing.
    const side = cross(normal, inward)
    let angle = (h(41) - 0.5) * 1.2
    let p = add(a, normal, 0.006)
    for (let k = 0; k < CRACK_SEGMENTS; k++) {
      angle += (h(42 + k) - 0.5) * 1.1
      const step = CRACK_STEP[0] + (CRACK_STEP[1] - CRACK_STEP[0]) * h(50 + k)
      const dir = unit(add([-inward[0] * Math.cos(angle), -inward[1] * Math.cos(angle), -inward[2] * Math.cos(angle)], side, Math.sin(angle)))
      const q = add(p, dir, step)
      if (!onWall(q, normal)) break
      const [w0, w1] = [CRACK_WIDTH * (1 - k / CRACK_SEGMENTS), CRACK_WIDTH * (1 - (k + 1) / CRACK_SEGMENTS) + 0.002]
      const n = cross(normal, dir)
      out.poly([add(p, n, -w0 / 2), add(q, n, -w1 / 2), add(q, n, w1 / 2), add(p, n, w0 / 2)], crackRgb, undefined, true)
      p = q
    }
  }
}

/**
 * `count` chips on `piece`, the wall in cell `id`: small dark ragged marks a hair proud of it,
 * scattered about the piece's middle, the same for the same cell.
 */
function chips(out: Writer, piece: Piece, id: number, count: number, base: RGB) {
  const n = piece.normal
  const c: P = [0, 0, 0]
  for (const p of piece.poly) for (let a = 0; a < 3; a++) c[a]! += p[a]! / piece.poly.length
  const up: P = Math.abs(n[1]) > 0.9 ? [1, 0, 0] : [0, 1, 0]
  const t: P = [n[1] * up[2] - n[2] * up[1], n[2] * up[0] - n[0] * up[2], n[0] * up[1] - n[1] * up[0]]
  const tl = Math.hypot(...t) || 1
  const u: P = [t[0] / tl, t[1] / tl, t[2] / tl]
  const v: P = [n[1] * u[2] - n[2] * u[1], n[2] * u[0] - n[0] * u[2], n[0] * u[1] - n[1] * u[0]]
  // How far a chip may stray from the middle: inside the piece, as near as its corners allow.
  const reach = Math.min(0.24, ...piece.poly.map((p) => Math.hypot(p[0] - c[0], p[1] - c[1], p[2] - c[2]) * 0.6))
  const tone: RGB = [base[0] * CHIP_SHADE, base[1] * CHIP_SHADE, base[2] * CHIP_SHADE]
  for (let k = 0; k < count; k++) {
    const [du, dv] = [(hash(id, k, 1, 4) - 0.5) * 2 * reach, (hash(id, k, 2, 4) - 0.5) * 2 * reach]
    const size = 0.035 + hash(id, k, 3, 4) * 0.05
    const at: P = [0, 1, 2].map((a) => c[a]! + u[a]! * du + v[a]! * dv + n[a]! * 0.012) as P
    const ring: P[] = []
    for (let i = 0; i < 7; i++) {
      const angle = (i / 7) * Math.PI * 2
      const r = size * (0.55 + hash(id, k, i, 5) * 0.7)
      ring.push([0, 1, 2].map((a) => at[a]! + (u[a]! * Math.cos(angle) + v[a]! * Math.sin(angle)) * r) as P)
    }
    out.poly(ring, tone, undefined, true)
  }
}

/**
 * The part of building `range`'s walls and roof that lies in `cells`, cut out as one piece with a
 * wall's depth round its edge, around `pivot` (metres): a section of wall or roof that has lost its
 * hold, to fall whole. Its edge matches the hole it leaves (`lookFracture`). Empty when no wall or
 * roof passes through those cells.
 */
export function lookChunk(look: CollisionMesh, range: LookRange, shell: Shell, cells: ReadonlySet<number>, pivot: readonly [number, number, number], wall: RGB, roof: RGB): LookSoup {
  const grid = new Grid(shell)
  const { kept, taken } = slice(look, range, grid, [...cells].map((id) => shell.coords(id)), (id) => cells.has(id))
  const { rims, place } = outline(taken, [...kept, ...taken])
  const out = new Writer(place, [pivot[0], pivot[1], pivot[2]])
  for (const piece of taken) out.poly(piece.poly, piece.roof ? roof : wall)
  out.rims(rims, wall, roof)
  return out.soup()
}
