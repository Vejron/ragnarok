/**
 * A building cut into cells, the same way `crates/shared/src/carve.rs` cuts it: the server says
 * which cell numbers a blast broke, and this works out which cubes and triangles those were.
 * Everything is integer centimetres, so a cell means the same on both sides (`carve.spec.ts` pins it).
 */

/** Side of a cell, centimetres, on one lattice for the whole world. Mirrors `CELL`. */
export const CELL = 100
/** Health of a cell against bullets. Mirrors `CELL_HP`. */
export const CELL_HP = 40
/** Mirrors `MAX_CELLS`. */
export const MAX_CELLS = 4_000_000

/** A triangle: three corners `[x, y, z]`, centimetres, flattened. */
export type Tri = [number, number, number, number, number, number, number, number, number]

/**
 * Mirrors `subdivide`: bisects the longest edge, ties to the lowest, with integer midpoints rounded
 * down. Triangles are nine numbers each, flat, in `source` and appended to `out`.
 */
export function subdivideFlat(source: ArrayLike<number>, limit: number, out: number[]) {
  const limitSq = limit * limit
  let stack = new Int32Array(9 * 64)
  let top = 0
  const t = new Int32Array(9)
  for (let s = 0; s + 9 <= source.length; s += 9) {
    for (let k = 0; k < 9; k++) stack[k] = source[s + k]!
    top = 9
    while (top > 0) {
      top -= 9
      for (let k = 0; k < 9; k++) t[k] = stack[top + k]!
      const l0 = (t[0]! - t[3]!) ** 2 + (t[1]! - t[4]!) ** 2 + (t[2]! - t[5]!) ** 2
      const l1 = (t[3]! - t[6]!) ** 2 + (t[4]! - t[7]!) ** 2 + (t[5]! - t[8]!) ** 2
      const l2 = (t[6]! - t[0]!) ** 2 + (t[7]! - t[1]!) ** 2 + (t[8]! - t[2]!) ** 2
      // The longest edge, ties to the lowest: p to q, with r opposite.
      let longest = 0
      let length = l0
      if (l1 > length) {
        longest = 1
        length = l1
      }
      if (l2 > length) {
        longest = 2
        length = l2
      }
      const a = longest * 3
      const b = ((longest + 1) % 3) * 3
      const c = ((longest + 2) % 3) * 3
      const px = t[a]!
      const py = t[a + 1]!
      const pz = t[a + 2]!
      const qx = t[b]!
      const qy = t[b + 1]!
      const qz = t[b + 2]!
      const rx = t[c]!
      const ry = t[c + 1]!
      const rz = t[c + 2]!
      if (length <= limitSq) {
        const ux = qx - px
        const uy = qy - py
        const uz = qz - pz
        const vx = rx - px
        const vy = ry - py
        const vz = rz - pz
        if (uy * vz - uz * vy !== 0 || uz * vx - ux * vz !== 0 || ux * vy - uy * vx !== 0) out.push(px, py, pz, qx, qy, qz, rx, ry, rz)
        continue
      }
      const mx = (px + qx) >> 1
      const my = (py + qy) >> 1
      const mz = (pz + qz) >> 1
      if (top + 18 > stack.length) {
        const grown = new Int32Array(stack.length * 2)
        grown.set(stack)
        stack = grown
      }
      stack[top] = px
      stack[top + 1] = py
      stack[top + 2] = pz
      stack[top + 3] = mx
      stack[top + 4] = my
      stack[top + 5] = mz
      stack[top + 6] = rx
      stack[top + 7] = ry
      stack[top + 8] = rz
      stack[top + 9] = mx
      stack[top + 10] = my
      stack[top + 11] = mz
      stack[top + 12] = qx
      stack[top + 13] = qy
      stack[top + 14] = qz
      stack[top + 15] = rx
      stack[top + 16] = ry
      stack[top + 17] = rz
      top += 18
    }
  }
}

/** `subdivideFlat` for triangles one by one. */
export function subdivide(source: readonly Tri[], limit: number, out: Tri[]) {
  const flat: number[] = []
  subdivideFlat(source.flat(), limit, flat)
  for (let i = 0; i < flat.length; i += 9) out.push(flat.slice(i, i + 9) as Tri)
}

/** Mirrors `SAMPLE`. */
const SAMPLE = CELL / 2

/**
 * Mirrors `sample`: points spread over the triangle at `o` of `t` (nine numbers), from the corner
 * facing the longest edge, at most `SAMPLE` apart along the edges. Calls `visit` with each.
 */
function sample(t: ArrayLike<number>, o: number, visit: (x: number, y: number, z: number) => void) {
  const length = (k: number) => {
    const j = ((k + 1) % 3) * 3
    return (t[o + k * 3]! - t[o + j]!) ** 2 + (t[o + k * 3 + 1]! - t[o + j + 1]!) ** 2 + (t[o + k * 3 + 2]! - t[o + j + 2]!) ** 2
  }
  let edge = 0
  let longest = length(0)
  for (let k = 1; k < 3; k++) {
    const l = length(k)
    if (l > longest) {
      edge = k
      longest = l
    }
  }
  const a = o + ((edge + 2) % 3) * 3
  const b = o + edge * 3
  const c = o + ((edge + 1) % 3) * 3
  let n = 1
  while ((n * SAMPLE) ** 2 < longest) n++
  const [ax, ay, az] = [t[a]!, t[a + 1]!, t[a + 2]!]
  const [bx, by, bz] = [t[b]! - ax, t[b + 1]! - ay, t[b + 2]! - az]
  const [cx, cy, cz] = [t[c]! - ax, t[c + 1]! - ay, t[c + 2]! - az]
  for (let i = 0; i <= n; i++) {
    for (let j = 0; j <= n - i; j++) {
      visit(ax + Math.floor((i * bx + j * cx) / n), ay + Math.floor((i * by + j * cy) / n), az + Math.floor((i * bz + j * cz) / n))
    }
  }
}

/**
 * A building's walls and roof as a grid of cells, and the small triangles of them that are collided
 * with, each in a cell. Mirrors `Shell`. The cells are worked out at once; the triangles only when
 * they are first asked for (a building that is drawn but never hurt does not need them).
 */
export class Shell {
  /** Corner of the grid with the smallest coordinates, centimetres. */
  readonly origin: [number, number, number]
  readonly cell = CELL
  readonly dims: [number, number, number]
  /** The building's own wall and roof triangles, nine numbers each. */
  private readonly sourceWalls: ArrayLike<number>
  private readonly sourceRoof: ArrayLike<number>
  /** The small triangles, nine numbers each, walls then roof, and the cell of each, once cut. */
  private cut: { tris: Int32Array; cells: Uint32Array; walls: number } | null = null
  /** 1 where the cell is part of the building. */
  private readonly solid: Uint8Array
  /** 1 where the roof passes through the cell, for drawing it in the roof's colour. Client only. */
  private readonly roofed: Uint8Array

  private constructor(origin: [number, number, number], dims: [number, number, number], walls: ArrayLike<number>, roof: ArrayLike<number>) {
    this.origin = origin
    this.dims = dims
    this.sourceWalls = walls
    this.sourceRoof = roof
    this.solid = new Uint8Array(dims[0] * dims[1] * dims[2])
    this.roofed = new Uint8Array(this.solid.length)
    for (const [list, isRoof] of [[walls, false], [roof, true]] as const) {
      for (let o = 0; o + 9 <= list.length; o += 9) {
        sample(list, o, (x, y, z) => {
          const id = this.cellOfPoint(x, y, z)
          this.solid[id] = 1
          if (isRoof) this.roofed[id] = 1
        })
      }
    }
    this.closeGaps()
  }

  /** Cuts a building from its wall and roof triangles, or null when it has none or is too big. */
  static of(walls: readonly Tri[], roof: readonly Tri[]): Shell | null {
    return Shell.ofFlat(walls.flat(), roof.flat())
  }

  /** As `of`, from flat triangles, nine numbers each. */
  static ofFlat(walls: ArrayLike<number>, roof: ArrayLike<number>): Shell | null {
    if (walls.length === 0 && roof.length === 0) return null
    const lo: [number, number, number] = [Infinity, Infinity, Infinity]
    const hi: [number, number, number] = [-Infinity, -Infinity, -Infinity]
    for (const list of [walls, roof]) {
      for (let i = 0; i < list.length; i++) {
        const axis = i % 3
        const v = list[i]!
        if (v < lo[axis]!) lo[axis] = v
        if (v > hi[axis]!) hi[axis] = v
      }
    }
    const origin = lo.map((v) => Math.floor(v / CELL) * CELL) as [number, number, number]
    const dims = [0, 1, 2].map((a) => Math.max(0, Math.floor((hi[a]! - 1 - origin[a]!) / CELL)) + 1) as [number, number, number]
    if (dims[0] * dims[1] * dims[2] > MAX_CELLS) return null
    return new Shell(origin, dims, walls, roof)
  }

  /** Mirrors `close_gaps`. A filled cell is roof when either of the pair it joins is. */
  private closeGaps() {
    const was = this.solid.slice()
    const [nx, ny, nz] = this.dims
    const at = (x: number, y: number, z: number) => was[(y * nz + z) * nx + x] === 1
    const fill = (target: number, a: number, b: number) => {
      this.solid[target] = 1
      if (this.roofed[a] || this.roofed[b]) this.roofed[target] = 1
    }
    for (let y = 0; y < ny; y++) {
      for (let z = 0; z < nz; z++) {
        for (let x = 0; x < nx; x++) {
          if (!at(x, y, z)) continue
          const here = this.index(x, y, z)
          if (x + 1 < nx && z + 1 < nz && at(x + 1, y, z + 1) && !at(x + 1, y, z) && !at(x, y, z + 1)) fill(this.index(x + 1, y, z), here, this.index(x + 1, y, z + 1))
          if (x + 1 < nx && z > 0 && at(x + 1, y, z - 1) && !at(x + 1, y, z) && !at(x, y, z - 1)) fill(this.index(x + 1, y, z), here, this.index(x + 1, y, z - 1))
          if (x + 1 < nx && y + 1 < ny && at(x + 1, y + 1, z) && !at(x + 1, y, z) && !at(x, y + 1, z)) fill(this.index(x + 1, y, z), here, this.index(x + 1, y + 1, z))
          if (x + 1 < nx && y > 0 && at(x + 1, y - 1, z) && !at(x + 1, y, z) && !at(x, y - 1, z)) fill(this.index(x + 1, y, z), here, this.index(x + 1, y - 1, z))
          if (z + 1 < nz && y + 1 < ny && at(x, y + 1, z + 1) && !at(x, y, z + 1) && !at(x, y + 1, z)) fill(this.index(x, y, z + 1), here, this.index(x, y + 1, z + 1))
          if (z + 1 < nz && y > 0 && at(x, y - 1, z + 1) && !at(x, y, z + 1) && !at(x, y - 1, z)) fill(this.index(x, y, z + 1), here, this.index(x, y - 1, z + 1))
        }
      }
    }
  }

  /** Mirrors `cell_at`: the cell point `(x, y, z)` (centimetres) is in, a point on a boundary in the lower one. */
  private cellOfPoint(x: number, y: number, z: number): number {
    const at = (v: number, axis: number) => Math.min(this.dims[axis]! - 1, Math.max(0, Math.floor((v - 1 - this.origin[axis]!) / CELL)))
    return (at(y, 1) * this.dims[2] + at(z, 2)) * this.dims[0] + at(x, 0)
  }

  /** Mirrors `owner`: the cell of the small triangle at `o` of `t`. */
  private owner(t: ArrayLike<number>, o: number): number {
    const cx = Math.floor((t[o]! + t[o + 3]! + t[o + 6]!) / 3)
    const cy = Math.floor((t[o + 1]! + t[o + 4]! + t[o + 7]!) / 3)
    const cz = Math.floor((t[o + 2]! + t[o + 5]! + t[o + 8]!) / 3)
    const home = this.cellOfPoint(cx, cy, cz)
    if (this.solid[home]) return home
    const [nx, ny, nz] = this.dims
    const x = home % nx
    const z = Math.floor(home / nx) % nz
    const y = Math.floor(home / (nx * nz))
    let best = -1
    let nearest = Infinity
    for (let dy = -1; dy <= 1; dy++) {
      for (let dz = -1; dz <= 1; dz++) {
        for (let dx = -1; dx <= 1; dx++) {
          const [ax, ay, az] = [x + dx, y + dy, z + dz]
          if ((dx === 0 && dy === 0 && dz === 0) || ax < 0 || ay < 0 || az < 0 || ax >= nx || ay >= ny || az >= nz) continue
          const id = this.index(ax, ay, az)
          if (!this.solid[id]) continue
          const c = this.centre(id)
          const d = (c[0] - cx) ** 2 + (c[1] - cy) ** 2 + (c[2] - cz) ** 2
          if (d < nearest || (d === nearest && id < best)) [best, nearest] = [id, d]
        }
      }
    }
    return best < 0 ? home : best
  }

  /** The small triangles and their cells, cut the first time they are needed. */
  private triangles() {
    if (this.cut) return this.cut
    const walls: number[] = []
    const roof: number[] = []
    subdivideFlat(this.sourceWalls, CELL, walls)
    subdivideFlat(this.sourceRoof, CELL, roof)
    const tris = new Int32Array(walls.length + roof.length)
    tris.set(walls)
    tris.set(roof, walls.length)
    const cells = new Uint32Array(tris.length / 9)
    for (let i = 0; i < cells.length; i++) cells[i] = this.owner(tris, i * 9)
    this.cut = { tris, cells, walls: walls.length / 9 }
    return this.cut
  }

  /** How many small triangles there are, walls then roof. */
  get triangleCount(): number {
    return this.triangles().cells.length
  }

  /** How many of the small triangles are wall. */
  get walls(): number {
    return this.triangles().walls
  }

  /** The cell of small triangle `i`. */
  triangleCell(i: number): number {
    return this.triangles().cells[i]!
  }

  /** Whether cell `id` is part of the building. */
  isSolid(id: number): boolean {
    return this.solid[id] === 1
  }

  /** Whether the roof passes through cell `id`. */
  isRoof(id: number): boolean {
    return this.roofed[id] === 1
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
    let n = 0
    for (const s of this.solid) n += s
    return n
  }

  /** The cell holding the point `(x, y, z)`, metres, clamped to the grid. */
  cellAt(x: number, y: number, z: number): number {
    const at = (axis: number, v: number) => Math.min(this.dims[axis]! - 1, Math.max(0, Math.floor((v * 100 - this.origin[axis]!) / this.cell)))
    return this.index(at(0, x), at(1, y), at(2, z))
  }

  /**
   * The standing cell (not in `removed`) nearest the point, metres, if its middle is within a
   * cell's reach of it: where a round that landed there struck. Mirrors `chip`.
   */
  nearestCell(x: number, y: number, z: number, removed: ReadonlySet<number> | undefined): number | null {
    const at = [x * 100, y * 100, z * 100]
    const around = (axis: number) => {
      const i = Math.floor((at[axis]! - this.origin[axis]!) / this.cell)
      return [Math.max(0, i - 1), Math.min(this.dims[axis]! - 1, i + 1)] as const
    }
    const [x0, x1] = around(0)
    const [y0, y1] = around(1)
    const [z0, z1] = around(2)
    let best: number | null = null
    let nearest = Infinity
    for (let yi = y0; yi <= y1; yi++) {
      for (let zi = z0; zi <= z1; zi++) {
        for (let xi = x0; xi <= x1; xi++) {
          const id = this.index(xi, yi, zi)
          if (!this.solid[id] || removed?.has(id)) continue
          const c = this.centre(id)
          const d = (c[0] - at[0]!) ** 2 + (c[1] - at[1]!) ** 2 + (c[2] - at[2]!) ** 2
          if (d < nearest) [best, nearest] = [id, d]
        }
      }
    }
    return nearest <= this.cell * this.cell * 1.5 ? best : null
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
    const { tris, cells } = this.triangles()
    const out: Tri[] = []
    for (let i = 0; i < cells.length; i++) {
      if (removed.has(cells[i]!)) continue
      out.push(Array.from(tris.subarray(i * 9, i * 9 + 9)) as Tri)
    }
    return out
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
    const flat = (from: number, to: number) => {
      const out = new Int32Array((to - from) * 9)
      for (let t = from, o = 0; t < to; t++) {
        for (let k = 0; k < 3; k++) {
          const v = triangles[t * 3 + k]! * 3
          out[o++] = Math.round(vertices[v]! * 100)
          out[o++] = Math.round(vertices[v + 1]! * 100)
          out[o++] = Math.round(vertices[v + 2]! * 100)
        }
      }
      return out
    }
    const roofFrom = building.first + building.walls
    shell = Shell.ofFlat(flat(building.first, roofFrom), flat(roofFrom, roofFrom + building.roof))
  }
  byIndex.set(index, shell)
  return shell
}
