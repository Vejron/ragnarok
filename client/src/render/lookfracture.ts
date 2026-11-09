import type { CollisionMesh } from '@/world'
import { CELL_HP, type Shell } from '@/sim/carve'
import { openingQuads, openingsFor, type Opening } from './windows'
import { SOFFIT_RGB, TRIM_RGB, eaveAnchor, eavesFor } from './eaves'
import { FLOOR_RGB, INTERIOR_SHADE, facadeFor, innerOf, type Facade } from './facade'
import { add, centreOf, cross, dot, edgeKey, hash, keyOf, normalOf, split, sub, unit, type P, type Poly, type RGB } from './geom'

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
/** The surface patterns that are wood (`Surface` Upright and Lap), that crack (Brick, Render and Ashlar), and roofs laid in loose pieces (Tiles and Shingles). */
const WOODEN = new Set([1, 2])
const CRACKING = new Set([3, 4, 8])
const SHINGLED = new Set([5, 9])
/** Loose shingles at a roof's hole: the share of its edges with one slid out of place, and with one missing; a shingle's size (metres). */
const SLID_CHANCE = 0.5
const MISSING_CHANCE = 0.4
const SHINGLE = [0.22, 0.33] as const
const SLID_SHADE = 0.85
const MISSING_SHADE = 0.4

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
interface Piece {
  poly: Poly
  /** Outward unit normal. */
  normal: P
  roof: boolean
}

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
 * Things with a size (edges, pieces of wall) by the cells of a building's grid their boxes reach
 * into, so what lies near a point is found without looking at all of them. A cell outside the grid
 * has a bucket too: what was cut off a building's edge lies just beyond it.
 */
class Buckets<T> {
  private readonly cells = new Map<number, T[]>()
  constructor(private readonly grid: Grid) {}

  /** Files `item` under every cell the box round `points`, grown by `margin` metres, reaches into. */
  add(item: T, points: readonly P[], margin: number) {
    const lo = [Infinity, Infinity, Infinity]
    const hi = [-Infinity, -Infinity, -Infinity]
    for (const p of points) for (let a = 0; a < 3; a++) {
      lo[a] = Math.min(lo[a]!, p[a]!)
      hi[a] = Math.max(hi[a]!, p[a]!)
    }
    const g = this.grid
    const [x0, y0, z0] = [0, 1, 2].map((a) => g.of(lo[a]! - margin, a))
    const [x1, y1, z1] = [0, 1, 2].map((a) => g.of(hi[a]! + margin, a))
    for (let y = y0!; y <= y1!; y++) for (let z = z0!; z <= z1!; z++) for (let x = x0!; x <= x1!; x++) {
      const key = Buckets.key(x, y, z)
      const list = this.cells.get(key)
      if (list) list.push(item)
      else this.cells.set(key, [item])
    }
  }

  /** What was filed under the cell holding `p`. */
  at(p: P): readonly T[] {
    const g = this.grid
    return this.cells.get(Buckets.key(g.of(p[0], 0), g.of(p[1], 1), g.of(p[2], 2))) ?? []
  }

  private static key(x: number, y: number, z: number): number {
    return ((x + 4096) * 8192 + (y + 4096)) * 8192 + (z + 4096)
  }
}

/**
 * The building's faces (`facadeFor`: its walls with the windows cut out, and its roof, wound
 * outwards): those further than `SLICE_REACH` cells from every cell of `near` whole, the rest sliced
 * along the grid so each piece lies in one cell, and split by whether `cut` holds for that cell.
 */
function slice(facade: Facade, grid: Grid, near: readonly (readonly number[])[], cut: (id: number) => boolean) {
  const { cell, origin, shell } = grid
  const whole: Piece[] = []
  const sliced: Piece[] = []
  const faces: [Poly, boolean][] = [...facade.walls.map((poly): [Poly, boolean] => [poly, false]), ...facade.roof.map((poly): [Poly, boolean] => [poly, true])]
  for (const [poly, roof] of faces) {
    const piece = { poly, normal: normalOf(poly), roof }
    const lo = [0, 1, 2].map((a) => grid.of(Math.min(...poly.map((p) => p[a]!)), a) - SLICE_REACH)
    const hi = [0, 1, 2].map((a) => grid.of(Math.max(...poly.map((p) => p[a]!)), a) + SLICE_REACH)
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

/**
 * Whether an edge runs along the side of one of `openings`: in its wall's plane and on the edge of
 * its rectangle. A wall piece that goes takes its window's hole with it, and that hole's edge is no
 * edge of the hole blown in the wall: nothing is drawn along it, or it would hang in the air.
 */
function windowEdge(openings: readonly Opening[]): (a: P, b: P) => boolean {
  const sides = openings.filter((o) => !o.door).map((o) => {
    const [a, b, , d] = o.corners
    const along = unit([b[0] - a[0], 0, b[2] - a[2]])
    return { n: o.normal, d: dot(a, o.normal), along, u: [dot(a, along), dot(b, along)] as const, v: [a[1], d[1]] as const }
  })
  const TOL = 0.015
  const onSide = (p: P, w: (typeof sides)[number]) => {
    if (Math.abs(dot(p, w.n) - w.d) > TOL) return false
    const [u, v] = [dot(p, w.along), p[1]]
    const inU = u > w.u[0] - TOL && u < w.u[1] + TOL
    const inV = v > w.v[0] - TOL && v < w.v[1] + TOL
    return (inU && (Math.abs(v - w.v[0]) < TOL || Math.abs(v - w.v[1]) < TOL)) || (inV && (Math.abs(u - w.u[0]) < TOL || Math.abs(u - w.u[1]) < TOL))
  }
  return (a, b) => {
    const m: P = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2]
    return sides.some((w) => onSide(a, w) && onSide(b, w) && onSide(m, w))
  }
}

/**
 * The floor left to draw with the cells `removed` gone: cut along the grid near them, and a piece
 * goes when no cell of the building stands in its own column or the ones round it, so no floor
 * hangs over the ground where a house stood. Floor far from any gone cell stays as it is.
 */
function floorStanding(floor: readonly Poly[], grid: Grid, removed: ReadonlySet<number>): Poly[] {
  if (removed.size === 0) return [...floor]
  const { cell, origin, shell } = grid
  const gone = [...removed].map((id) => shell.coords(id))
  const columns = new Map<string, boolean>()
  const columnStands = (x: number, z: number): boolean => {
    const key = `${x},${z}`
    let stands = columns.get(key)
    if (stands !== undefined) return stands
    stands = false
    for (let dx = -1; dx <= 1 && !stands; dx++) for (let dz = -1; dz <= 1 && !stands; dz++) for (let y = 0; y < shell.dims[1]; y++) {
      if (!grid.has(x + dx, y, z + dz)) continue
      const id = shell.index(x + dx, y, z + dz)
      if (shell.isSolid(id) && !removed.has(id)) {
        stands = true
        break
      }
    }
    columns.set(key, stands)
    return stands
  }
  const out: Poly[] = []
  for (const poly of floor) {
    const lo = [0, 2].map((a) => grid.of(Math.min(...poly.map((p) => p[a]!)), a) - SLICE_REACH)
    const hi = [0, 2].map((a) => grid.of(Math.max(...poly.map((p) => p[a]!)), a) + SLICE_REACH)
    if (!gone.some((c) => c[0] >= lo[0]! && c[0] <= hi[0]! && c[2] >= lo[1]! && c[2] <= hi[1]!)) {
      out.push(poly)
      continue
    }
    let polys: Poly[] = [poly]
    for (const axis of [0, 2]) {
      const [a, b] = [Math.min(...poly.map((p) => p[axis]!)), Math.max(...poly.map((p) => p[axis]!))]
      for (let k = Math.floor((a - origin[axis]!) / cell) + 1; origin[axis]! + k * cell < b; k++) {
        const at = origin[axis]! + k * cell
        const next: Poly[] = []
        for (const q of polys) for (const part of split(q, axis, at)) if (part.length >= 3) next.push(part)
        polys = next
      }
    }
    for (const piece of polys) {
      const c = centreOf(piece)
      if (columnStands(grid.of(c[0], 0), grid.of(c[2], 2))) out.push(piece)
    }
  }
  return out
}

interface Rim {
  a: P
  b: P
  normal: P
  roof: boolean
  /** In the plane of its wall, across the edge into the hole. */
  inward: P
}

/**
 * The outline of `taken`: the sides no other piece of it shares, each to be drawn with a wall's
 * depth; and where each corner of the outline goes, pushed about in its wall's plane (not a corner
 * two planes share: the building's own edges stay straight). The push depends on the corner alone,
 * so a piece that falls out of a wall has the same ragged edge as the hole it leaves. Edges `skip`
 * holds for (a window's) are no rim: they are the hole the wall had already.
 */
function outline(grid: Grid, taken: readonly Piece[], all: readonly Piece[], skip: (a: P, b: P) => boolean = () => false) {
  const edges = new Map<string, number>()
  for (const { poly } of taken) for (let i = 0; i < poly.length; i++) {
    const key = edgeKey(poly[i]!, poly[(i + 1) % poly.length]!)
    edges.set(key, (edges.get(key) ?? 0) + 1)
  }
  // The edges no other gone piece has as its own. A wall's top edge is cut where its window is and the
  // roof's edge over it is not, so an edge is shared, too, when another gone piece's edge runs along it
  // over its middle (`covered`): only what nothing else runs along is the hole's edge.
  const loose: { a: P; b: P; piece: number }[] = []
  taken.forEach(({ poly }, piece) => {
    for (let i = 0; i < poly.length; i++) {
      const [a, b] = [poly[i]!, poly[(i + 1) % poly.length]!]
      if (edges.get(edgeKey(a, b)) === 1 && !skip(a, b)) loose.push({ a, b, piece })
    }
  })
  // The loose edges by the grid cells their boxes reach into, so an edge is only held against those near it.
  const nearby = new Buckets<(typeof loose)[number]>(grid)
  for (const e of loose) nearby.add(e, [e.a, e.b], 0.02)
  const covered = (a: P, b: P, own: number): boolean => {
    const m: P = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2]
    const d = unit(sub(b, a))
    for (const e of nearby.at(m)) {
      if (e.piece === own) continue
      const ed = sub(e.b, e.a)
      const len = Math.hypot(...ed)
      if (len < 1e-6 || Math.abs(dot(ed, d)) < len * 0.9999) continue
      const u = ed.map((v) => v / len) as P
      const am = sub(m, e.a)
      const t = dot(am, u)
      if (t < -0.01 || t > len + 0.01) continue
      if (Math.hypot(am[0] - u[0] * t, am[1] - u[1] * t, am[2] - u[2] * t) < 0.01) return true
    }
    return false
  }
  const rims: Rim[] = []
  const corners = new Set<string>()
  // The grid cells round the hole's corners: a point in none of them is no corner, and needs no key made for it.
  const cornerCells = new Buckets<true>(grid)
  for (const { a, b, piece } of loose) {
    if (covered(a, b, piece)) continue
    const { poly, normal, roof } = taken[piece]!
    // Towards the middle of the piece that is gone, flattened into its plane.
    const mid = centreOf(poly)
    const d = [0, 1, 2].map((k) => mid[k]! - (a[k]! + b[k]!) / 2)
    const along = d[0]! * normal[0] + d[1]! * normal[1] + d[2]! * normal[2]
    const flat = [0, 1, 2].map((k) => d[k]! - normal[k]! * along)
    const fl = Math.hypot(...flat) || 1
    rims.push({ a, b, normal, roof, inward: flat.map((v) => v / fl) as P })
    corners.add(keyOf(a))
    corners.add(keyOf(b))
    cornerCells.add(true, [a, b], 0.01)
  }
  const planes = new Map<string, P[]>()
  for (const { poly, normal } of all) for (const p of poly) {
    if (cornerCells.at(p).length === 0) continue
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
  const place = (p: P): P => (moved.size > 0 && cornerCells.at(p).length > 0 ? (moved.get(keyOf(p)) ?? p) : p)
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
    const tones = tone ? at.map((p) => tone(p)) : null
    for (let i = 1; i + 1 < at.length; i++) {
      this.corner(at[0]!, base, tones ? tones[0]! : 1)
      this.corner(at[i]!, base, tones ? tones[i]! : 1)
      this.corner(at[i + 1]!, base, tones ? tones[i + 1]! : 1)
    }
  }

  /** One corner of a triangle at `p`, in `base` times `tone`. */
  private corner(p: P, base: RGB, tone: number) {
    const [ox, oy, oz] = this.offset
    this.positions.push(p[0] - ox, p[1] - oy, p[2] - oz)
    this.colors.push(base[0] * tone, base[1] * tone, base[2] * tone)
    this.custom.push(this.surface, 0, 0, 0)
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
  const facade = facadeFor(look, range)
  const openings = openingsFor(look, range)
  const { whole, kept, taken } = slice(facade, grid, near, (id) => removed.has(id))
  const { rims, place } = outline(grid, taken, [...kept, ...taken], windowEdge(openings))

  // Which cells are gone, and which have a gone cell within two of them: only a corner in one of those can be sooted.
  const [nx, ny, nz] = shell.dims
  const gone = new Uint8Array(nx * ny * nz)
  const sooty = new Uint8Array(nx * ny * nz)
  for (const id of removed) {
    gone[id] = 1
    const [x, y, z] = shell.coords(id)
    for (let dy = -2; dy <= 2; dy++) for (let dz = -2; dz <= 2; dz++) for (let dx = -2; dx <= 2; dx++) {
      if (grid.has(x + dx, y + dy, z + dz)) sooty[shell.index(x + dx, y + dy, z + dz)] = 1
    }
  }
  // Soot and wear at a corner: by how near the nearest cell that is gone is, and how worn its own cell.
  const shade = (p: P): number => {
    const [gx, gy, gz] = [grid.of(p[0], 0), grid.of(p[1], 1), grid.of(p[2], 2)]
    if (grid.has(gx, gy, gz) && sooty[shell.index(gx, gy, gz)] === 0) return 1
    // The nearest gone cell's middle, from `p`, by its square first.
    let [best, ex, ey, ez] = [Infinity, 0, 0, 0]
    for (let y = Math.max(0, gy - 2); y <= Math.min(ny - 1, gy + 2); y++) {
      for (let z = Math.max(0, gz - 2); z <= Math.min(nz - 1, gz + 2); z++) {
        for (let x = Math.max(0, gx - 2); x <= Math.min(nx - 1, gx + 2); x++) {
          if (gone[(y * nz + z) * nx + x] === 0) continue
          const [dx, dy, dz] = [origin[0] + (x + 0.5) * cell - p[0], origin[1] + (y + 0.5) * cell - p[1], origin[2] + (z + 0.5) * cell - p[2]]
          const d = dx * dx + dy * dy + dz * dz
          if (d < best) [best, ex, ey, ez] = [d, dx, dy, dz]
        }
      }
    }
    if (best === Infinity) return 1
    const nearest = Math.hypot(ex, ey, ez)
    let burn = Math.max(0, Math.min(1, (BURN_FAR - nearest) / (BURN_FAR - BURN_NEAR)))
    burn = Math.min(1, burn * (0.6 + 0.8 * hash(p[0], p[1], p[2], 3)))
    return 1 - burn * (1 - SOOT)
  }

  const out = new Writer(place)
  // A piece drawn whole lies two cells or more from every hole, so none of its corners is pushed about (`outline`).
  for (const piece of whole) {
    out.surface = piece.roof ? surfaces.roof : surfaces.wall
    out.poly(piece.poly, piece.roof ? roof : wall, undefined, true)
  }
  for (const piece of kept) {
    out.surface = piece.roof ? surfaces.roof : surfaces.wall
    out.poly(piece.poly, piece.roof ? roof : wall, shade)
  }
  out.surface = 0
  // The room inside: what stands, turned in a wall's thickness, dim, with the same ragged edge; and the floor where the house still stands over it.
  const dim = (c: RGB): RGB => [c[0] * INTERIOR_SHADE, c[1] * INTERIOR_SHADE, c[2] * INTERIOR_SHADE]
  for (const piece of whole) out.poly(innerOf(piece.poly, piece.normal), dim(piece.roof ? roof : wall), shade, true)
  for (const piece of kept) out.poly(innerOf(piece.poly.map(place), piece.normal), dim(piece.roof ? roof : wall), shade, true)
  for (const poly of floorStanding(facade.floor, grid, removed)) out.poly(poly, FLOOR_RGB, undefined, true)
  out.rims(rims, wall, roof, shade)
  // The roof's overhang, each strip while the roof it hangs from still stands.
  for (const strip of eavesFor(look, range)) {
    const at = eaveAnchor(strip)
    const [x, y, z] = [0, 1, 2].map((a) => grid.of(at[a]!, a)) as P
    if (grid.has(x, y, z) && removed.has(shell.index(x, y, z))) continue
    for (const { corners, part } of strip.quads) {
      out.surface = part === 'top' ? surfaces.roof : 0
      out.poly(corners, part === 'top' ? roof : part === 'soffit' ? SOFFIT_RGB : TRIM_RGB, shade, true)
    }
  }
  out.surface = 0
  // Whether `p` lies on what stands of a wall facing `normal`: within one of its pieces, in their plane.
  // The standing pieces by the cells they reach into, filed the first time a crack or shingle asks.
  let standing: Buckets<Piece> | null = null
  const onWall = (p: P, normal: P) => {
    if (!standing) {
      standing = new Buckets<Piece>(grid)
      for (const piece of [...whole, ...kept]) standing.add(piece, piece.poly, 0.035)
    }
    return standing.at(p).some((piece) => {
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
  }
  edgeDetail(out, rims, surfaces, wall, roof, onWall)
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
  // Windows and doors, but only those with wall still standing round them: a frame does not hang in the air.
  const framed = removed.size === 0 ? openings : openings.filter((o) => openingStands(shell, removed, o))
  openingQuads(framed, (corners, rgb) => out.poly(corners, rgb, shade, true))
  return out.soup()
}

/**
 * Whether opening `o` still stands with the cells in `removed` gone: the wall just outside each of
 * its four sides stands (its cell is not removed, nor left with nothing to hang from, `orphan`), and
 * no cell of its own is gone. Otherwise its frame would hang in the air, and its glass is gone with the wall.
 */
export function openingStands(shell: Shell, removed: ReadonlySet<number>, o: Opening): boolean {
  const grid = new Grid(shell)
  const cut = (id: number) => removed.has(id)
  const [a, b, c, d] = o.corners
  const n = o.normal
  const along = unit([b[0] - a[0], 0, b[2] - a[2]])
  const mid = (p: P, q: P): P => [(p[0] + q[0]) / 2, (p[1] + q[1]) / 2, (p[2] + q[2]) / 2]
  const beside: P[] = [add(mid(a, b), [0, -1, 0], 0.05), add(mid(b, c), along, 0.05), add(mid(c, d), [0, 1, 0], 0.05), add(mid(d, a), along, -0.05)]
  // And the opening itself: a cell blasted out of it takes it along, frame and all.
  const centre = mid(mid(a, b), mid(c, d))
  const within: P[] = [centre, ...[a, b, c, d].map((p) => mid(mid(p, centre), p))]
  for (const p of [...beside, ...within]) {
    // A hair inside the wall, as the wall's own pieces are judged.
    const q = add(p, n, -0.01)
    const [x, y, z] = [0, 1, 2].map((k) => grid.of(q[k]!, k)) as P
    if (grid.has(x, y, z) && cut(shell.index(x, y, z))) return false
    if (orphan(grid, x, y, z, cut)) return false
  }
  return true
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
function edgeDetail(out: Writer, rims: readonly Rim[], surfaces: Surfaces, wall: RGB, roofRgb: RGB, onWall: (p: P, normal: P) => boolean) {
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
      if (SHINGLED.has(surfaces.roof)) shingles(out, rim, a, b, h, normal, inward, onWall, roofRgb)
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
 * Loose shingles at a roof hole's edge from `a` to `b`: one slid out of its course and tilted up on
 * the roof just outside the hole, and a dark patch where one is missing, both only on what stands of the roof.
 */
function shingles(out: Writer, rim: Rim, a: P, b: P, h: (salt: number) => number, normal: P, inward: P, onWall: (p: P, normal: P) => boolean, roof: RGB) {
  const along = unit([b[0] - a[0], b[1] - a[1], b[2] - a[2]])
  const at = (t: number) => add(a, [b[0] - a[0], b[1] - a[1], b[2] - a[2]], t)
  const shade = (k: number): RGB => [roof[0] * k, roof[1] * k, roof[2] * k]
  // A shingle on the roof about `centre`, its length along `dir`, lifted `rise` at its far end and `up` off the roof.
  const plate = (centre: P, dir: P, rise: number, up: number, rgb: RGB) => {
    const side = unit(cross(normal, dir))
    const [w, l] = [SHINGLE[0] / 2, SHINGLE[1] / 2]
    const near = add(centre, dir, -l)
    const far = add(add(centre, dir, l), normal, rise)
    out.poly([add(add(near, side, -w), normal, up), add(add(near, side, w), normal, up), add(add(far, side, w), normal, up), add(add(far, side, -w), normal, up)], rgb, undefined, true)
  }
  const outward: P = [-inward[0], -inward[1], -inward[2]]
  if (h(15) < SLID_CHANCE) {
    const centre = add(at(0.2 + 0.6 * h(17)), outward, 0.12 + 0.25 * h(18))
    if (onWall(centre, normal)) {
      const dir = unit(add(outward, along, (h(19) - 0.5) * 0.8))
      plate(centre, dir, 0.08 + 0.1 * h(21), 0.03, shade(SLID_SHADE))
    }
  }
  if (h(16) < MISSING_CHANCE) {
    const centre = add(at(0.15 + 0.7 * h(22)), outward, 0.3 + 0.3 * h(23))
    if (onWall(centre, normal)) plate(centre, unit(add(outward, along, (h(24) - 0.5) * 0.3)), 0, 0.006, shade(MISSING_SHADE))
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
  const { kept, taken } = slice(facadeFor(look, range), grid, [...cells].map((id) => shell.coords(id)), (id) => cells.has(id))
  const { rims, place } = outline(grid, taken, [...kept, ...taken], windowEdge(openingsFor(look, range)))
  const out = new Writer(place, [pivot[0], pivot[1], pivot[2]])
  for (const piece of taken) out.poly(piece.poly, piece.roof ? roof : wall)
  // Its inner face goes with it, so a section lying on the ground shows its dim inside.
  for (const piece of taken) out.poly(innerOf(piece.poly.map(place), piece.normal), [wall[0] * INTERIOR_SHADE, wall[1] * INTERIOR_SHADE, wall[2] * INTERIOR_SHADE], undefined, true)
  out.rims(rims, wall, roof)
  // The overhang goes with the roof it hangs from.
  for (const strip of eavesFor(look, range)) {
    const at = eaveAnchor(strip)
    const [x, y, z] = [0, 1, 2].map((a) => grid.of(at[a]!, a)) as P
    if (!grid.has(x, y, z) || !cells.has(shell.index(x, y, z))) continue
    for (const { corners, part } of strip.quads) out.poly(corners, part === 'top' ? roof : part === 'soffit' ? SOFFIT_RGB : TRIM_RGB, undefined, true)
  }
  return out.soup()
}
