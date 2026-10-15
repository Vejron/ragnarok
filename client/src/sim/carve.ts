/**
 * A building cut into cells, the same way `crates/shared/src/carve.rs` cuts it: the server says
 * which cell numbers a blast broke, and this works out which triangles those were. Everything is
 * integer centimetres, so a triangle lands in the same cell on both sides (`carve.spec.ts` pins it).
 */

/** Smallest cell, centimetres. Mirrors `CELL_MIN`. */
export const CELL_MIN = 100
/** Mirrors `MAX_CELLS` and `MAX_TRIANGLES`. */
export const MAX_CELLS = 60_000
export const MAX_TRIANGLES = 6_000
/** Mirrors `CELL_SIZES`. */
const CELL_SIZES = [100, 150, 200, 300, 400, 600, 900, 1400]

/** A triangle: three corners `[x, y, z]`, centimetres, flattened. */
export type Tri = [number, number, number, number, number, number, number, number, number]

const edge = (t: readonly number[], a: number, b: number) => (t[a * 3]! - t[b * 3]!) ** 2 + (t[a * 3 + 1]! - t[b * 3 + 1]!) ** 2 + (t[a * 3 + 2]! - t[b * 3 + 2]!) ** 2

/** Mirrors `subdivide`: bisects the longest edge, ties to the lowest, with integer midpoints rounded down. */
export function subdivide(source: readonly Tri[], limit: number, out: Tri[]) {
  const limitSq = limit * limit
  const stack: Tri[] = []
  for (const tri of source) {
    stack.push(tri)
    while (stack.length > 0) {
      const t = stack.pop()!
      let longest = 0
      let length = -1
      for (let i = 0; i < 3; i++) {
        const l = edge(t, i, (i + 1) % 3)
        if (l > length) [longest, length] = [i, l]
      }
      const a = longest * 3
      const b = ((longest + 1) % 3) * 3
      const c = ((longest + 2) % 3) * 3
      const p = [t[a]!, t[a + 1]!, t[a + 2]!]
      const q = [t[b]!, t[b + 1]!, t[b + 2]!]
      const r = [t[c]!, t[c + 1]!, t[c + 2]!]
      if (length <= limitSq) {
        const u = [q[0]! - p[0]!, q[1]! - p[1]!, q[2]! - p[2]!]
        const v = [r[0]! - p[0]!, r[1]! - p[1]!, r[2]! - p[2]!]
        const x = u[1]! * v[2]! - u[2]! * v[1]!
        const y = u[2]! * v[0]! - u[0]! * v[2]!
        const z = u[0]! * v[1]! - u[1]! * v[0]!
        if (x !== 0 || y !== 0 || z !== 0) out.push([p[0]!, p[1]!, p[2]!, q[0]!, q[1]!, q[2]!, r[0]!, r[1]!, r[2]!])
        continue
      }
      const m = [(p[0]! + q[0]!) >> 1, (p[1]! + q[1]!) >> 1, (p[2]! + q[2]!) >> 1]
      stack.push([p[0]!, p[1]!, p[2]!, m[0]!, m[1]!, m[2]!, r[0]!, r[1]!, r[2]!])
      stack.push([m[0]!, m[1]!, m[2]!, q[0]!, q[1]!, q[2]!, r[0]!, r[1]!, r[2]!])
    }
  }
}

/** A building's walls and roof as small triangles in a grid of cells. Mirrors `Shell`. */
export class Shell {
  /** Corner of the grid with the smallest coordinates, centimetres. */
  readonly origin: [number, number, number]
  readonly cell: number
  readonly dims: [number, number, number]
  /** Wall triangles then roof triangles, and the cell of each. */
  readonly triangles: { tri: Tri; cell: number }[] = []
  /** How many of `triangles` are wall. */
  readonly walls: number
  private readonly solid: boolean[]

  private constructor(origin: [number, number, number], cell: number, dims: [number, number, number], walls: Tri[], roof: Tri[]) {
    this.origin = origin
    this.cell = cell
    this.dims = dims
    this.walls = walls.length
    this.solid = Array.from({ length: dims[0] * dims[1] * dims[2] }, () => false)
    for (const tri of [...walls, ...roof]) {
      const id = this.cellOf(tri)
      this.solid[id] = true
      this.triangles.push({ tri, cell: id })
    }
  }

  /** Cuts a building from its wall and roof triangles, or null when it has none. */
  static of(walls: readonly Tri[], roof: readonly Tri[]): Shell | null {
    if (walls.length === 0 && roof.length === 0) return null
    const lo: [number, number, number] = [Infinity, Infinity, Infinity]
    const hi: [number, number, number] = [-Infinity, -Infinity, -Infinity]
    for (const tri of [...walls, ...roof]) {
      for (let k = 0; k < 3; k++) {
        for (let axis = 0; axis < 3; axis++) {
          lo[axis] = Math.min(lo[axis]!, tri[k * 3 + axis]!)
          hi[axis] = Math.max(hi[axis]!, tri[k * 3 + axis]!)
        }
      }
    }
    let best: { cell: number; dims: [number, number, number]; walls: Tri[]; roof: Tri[] } | null = null
    for (const cell of CELL_SIZES) {
      const dims = [0, 1, 2].map((a) => Math.max(1, Math.trunc((hi[a]! - lo[a]!) / cell) + 1)) as [number, number, number]
      const smallWalls: Tri[] = []
      const smallRoof: Tri[] = []
      subdivide(walls, cell, smallWalls)
      subdivide(roof, cell, smallRoof)
      best = { cell, dims, walls: smallWalls, roof: smallRoof }
      if (dims[0] * dims[1] * dims[2] <= MAX_CELLS && smallWalls.length + smallRoof.length <= MAX_TRIANGLES) break
    }
    return new Shell(lo, best!.cell, best!.dims, best!.walls, best!.roof)
  }

  /** Mirrors `cell_of`: the cell a triangle's centroid is in. */
  private cellOf(tri: Tri): number {
    const at = (axis: number) => {
      const centroid = Math.floor((tri[axis]! + tri[3 + axis]! + tri[6 + axis]!) / 3)
      const i = Math.floor((centroid - this.origin[axis]!) / this.cell)
      return Math.min(this.dims[axis]! - 1, Math.max(0, i))
    }
    return (at(1) * this.dims[2] + at(2)) * this.dims[0] + at(0)
  }

  /** Mirrors `Shell::index`. */
  index(x: number, y: number, z: number): number {
    return (y * this.dims[2] + z) * this.dims[0] + x
  }

  /** The middle of cell `id`, centimetres. */
  centre(id: number): [number, number, number] {
    const x = id % this.dims[0]
    const z = Math.floor(id / this.dims[0]) % this.dims[2]
    const y = Math.floor(id / (this.dims[0] * this.dims[2]))
    const half = this.cell / 2
    return [this.origin[0] + x * this.cell + half, this.origin[1] + y * this.cell + half, this.origin[2] + z * this.cell + half]
  }

  /** How many cells hold a piece of wall or roof. */
  get solidCount(): number {
    return this.solid.reduce((n, s) => n + (s ? 1 : 0), 0)
  }

  /** The cell holding the point `(x, y, z)`, metres, clamped to the grid. */
  cellAt(x: number, y: number, z: number): number {
    const at = (axis: number, v: number) => Math.min(this.dims[axis]! - 1, Math.max(0, Math.floor((v * 100 - this.origin[axis]!) / this.cell)))
    return this.index(at(0, x), at(1, y), at(2, z))
  }

  /**
   * Splits `cells` into pieces that hang together (cells touching by a face, edge or corner), none
   * bigger than `most` cells, in a fixed order.
   */
  clusters(cells: readonly number[], most: number): number[][] {
    const left = new Set(cells)
    const out: number[][] = []
    for (const start of [...cells].sort((a, b) => a - b)) {
      if (!left.has(start)) continue
      left.delete(start)
      const piece = [start]
      for (let at = 0; at < piece.length && piece.length < most; at++) {
        const id = piece[at]!
        const x = id % this.dims[0]
        const z = Math.floor(id / this.dims[0]) % this.dims[2]
        const y = Math.floor(id / (this.dims[0] * this.dims[2]))
        for (let dy = -1; dy <= 1 && piece.length < most; dy++) {
          for (let dz = -1; dz <= 1 && piece.length < most; dz++) {
            for (let dx = -1; dx <= 1 && piece.length < most; dx++) {
              const [nx, ny, nz] = [x + dx, y + dy, z + dz]
              if (nx < 0 || ny < 0 || nz < 0 || nx >= this.dims[0] || ny >= this.dims[1] || nz >= this.dims[2]) continue
              const next = this.index(nx, ny, nz)
              if (!left.has(next)) continue
              left.delete(next)
              piece.push(next)
            }
          }
        }
      }
      out.push(piece)
    }
    return out
  }

  /** The small triangles whose cell is not in `removed`, walls first. */
  standing(removed: ReadonlySet<number>): Tri[] {
    return this.triangles.filter((t) => !removed.has(t.cell)).map((t) => t.tri)
  }
}

/** Source triangles `[from, to)` of a tile's collision mesh as centimetre triangles. */
export function trianglesOf(vertices: Float32Array, triangles: Uint32Array, from: number, to: number): Tri[] {
  const out: Tri[] = []
  for (let t = from; t < to; t++) {
    const tri: number[] = []
    for (let k = 0; k < 3; k++) {
      const v = triangles[t * 3 + k]! * 3
      tri.push(Math.round(vertices[v]! * 100), Math.round(vertices[v + 1]! * 100), Math.round(vertices[v + 2]! * 100))
    }
    out.push(tri as Tri)
  }
  return out
}

const shells = new WeakMap<{ buildings: unknown }, Map<number, Shell | null>>()

/**
 * Building `index` of `tile`, cut into cells, once: its wall and roof triangles from the tile's
 * collision mesh. Null when it has none (a building the importer left no walls for).
 */
export function shellOf(tile: { buildings: readonly { first: number; walls: number; roof: number }[]; collision: { vertices: Float32Array; triangles: Uint32Array } }, index: number): Shell | null {
  let byIndex = shells.get(tile)
  if (!byIndex) shells.set(tile, (byIndex = new Map()))
  if (byIndex.has(index)) return byIndex.get(index)!
  const building = tile.buildings[index]
  let shell: Shell | null = null
  if (building) {
    const { vertices, triangles } = tile.collision
    const walls = trianglesOf(vertices, triangles, building.first, building.first + building.walls)
    const roof = trianglesOf(vertices, triangles, building.first + building.walls, building.first + building.walls + building.roof)
    shell = Shell.of(walls, roof)
  }
  byIndex.set(index, shell)
  return shell
}
