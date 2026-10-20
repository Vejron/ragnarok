/**
 * A building cut into cells, the same way `crates/shared/src/carve.rs` cuts it: the server says
 * which cell numbers a blast broke, and this works out which cubes those were. The building is
 * drawn and collided with as its cubes.
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
 * Mirrors `unpack`: `count` flags, one a cell, from the bytes the importer packed them into (each
 * XORed with the flag a layer of `layer` cells below, eight to a byte, runs of zero bytes as a 0 and
 * the run's length less one in LEB128).
 */
export function unpack(packed: Uint8Array, count: number, layer: number): Uint8Array {
  const bytes = new Uint8Array(Math.ceil(count / 8))
  let at = 0
  let to = 0
  while (at < packed.length && to < bytes.length) {
    const byte = packed[at++]!
    if (byte !== 0) {
      bytes[to++] = byte
      continue
    }
    let run = 0
    let scale = 1
    while (at < packed.length) {
      const part = packed[at++]!
      run += (part & 0x7f) * scale
      scale *= 128
      if ((part & 0x80) === 0) break
    }
    to += run + 1
  }
  const flags = new Uint8Array(count)
  for (let i = 0; i < count; i++) {
    const bit = (bytes[i >> 3]! >> (i & 7)) & 1
    flags[i] = bit ^ (i >= layer ? flags[i - layer]! : 0)
  }
  return flags
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
 * One merged face of `greedyFaces`: `w × h` cells from `(a, b)` of layer `i` along axis `d`, facing
 * `side` (1 towards +d, -1 towards -d). `u` and `v` are the next axes round from `d`. Mirrors `Face`.
 */
export interface Face {
  d: number
  u: number
  v: number
  side: number
  i: number
  a: number
  b: number
  w: number
  h: number
  key: number
}

/**
 * Mirrors `greedy`: the faces of the cells of a `dims` grid with a key (`keys`, -1 for an empty
 * cell) that look onto an empty cell or out of the grid, merged into rectangles of one key, in the
 * same order as the server's.
 */
export function greedyFaces(dims: readonly [number, number, number], keys: Int32Array, emit: (face: Face) => void) {
  // Step in the cell number for one cell along x, y and z (`Shell::index`).
  const stride = [1, dims[0] * dims[2], dims[0]]
  for (let d = 0; d < 3; d++) {
    const u = (d + 1) % 3
    const v = (d + 2) % 3
    const [nu, nv, nd] = [dims[u]!, dims[v]!, dims[d]!]
    const [su, sv, sd] = [stride[u]!, stride[v]!, stride[d]!]
    const mask = new Int32Array(nu * nv)
    for (const side of [1, -1]) {
      for (let i = 0; i < nd; i++) {
        const j = i + side
        const beyond = j >= 0 && j < nd
        let any = false
        for (let b = 0; b < nv; b++) {
          for (let a = 0; a < nu; a++) {
            const id = i * sd + a * su + b * sv
            const key = keys[id]!
            mask[b * nu + a] = key >= 0 && !(beyond && keys[id + side * sd]! >= 0) ? key : -1
            if (mask[b * nu + a]! >= 0) any = true
          }
        }
        if (!any) continue
        for (let b = 0; b < nv; b++) {
          for (let a = 0; a < nu; ) {
            const key = mask[b * nu + a]!
            if (key < 0) {
              a++
              continue
            }
            let w = 1
            while (a + w < nu && mask[b * nu + a + w] === key) w++
            let h = 1
            grow: while (b + h < nv) {
              for (let k = 0; k < w; k++) if (mask[(b + h) * nu + a + k] !== key) break grow
              h++
            }
            for (let r = 0; r < h; r++) mask.fill(-1, (b + r) * nu + a, (b + r) * nu + a + w)
            emit({ d, u, v, side, i, a, b, w, h, key })
            a += w
          }
        }
      }
    }
  }
}

/** A building's walls and roof as a grid of cells. Mirrors `Shell`. */
export class Shell {
  /** Corner of the grid with the smallest coordinates, centimetres. */
  readonly origin: [number, number, number]
  readonly cell = CELL
  readonly dims: [number, number, number]
  /** 1 where the cell is part of the building. */
  private readonly solid: Uint8Array
  /** 1 where the roof passes through the cell, for drawing it in the roof's colour. Client only. */
  private readonly roofed: Uint8Array

  private constructor(origin: [number, number, number], dims: [number, number, number], walls: ArrayLike<number>, roof: ArrayLike<number>, baked?: { solid: Uint8Array; roof: Uint8Array }) {
    this.origin = origin
    this.dims = dims
    if (baked) {
      this.solid = baked.solid
      this.roofed = baked.roof
      return
    }
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

  /** Mirrors `Shell::from_cells`: the building as the importer baked it, or null when it has no cells. */
  static fromCells(cells: { origin: [number, number, number]; dims: [number, number, number]; solid: Uint8Array; roof: Uint8Array }): Shell | null {
    const [nx, ny, nz] = cells.dims
    const count = nx * ny * nz
    if (count === 0 || count > MAX_CELLS) return null
    const layer = nx * nz
    return new Shell([...cells.origin], [nx, ny, nz], [], [], { solid: unpack(cells.solid, count, layer), roof: unpack(cells.roof, count, layer) })
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

  /** The boxes of the whole building, worked out once: every tile collider shares them. */
  private wholeBoxes: Int32Array | null = null

  /**
   * Mirrors `boxes`: what the building is collided with, its standing cubes (not in `removed`)
   * merged into boxes of six centimetre numbers each, the low corner then the high one.
   */
  boxes(removed: ReadonlySet<number>): Int32Array {
    if (removed.size === 0 && this.wholeBoxes) return this.wholeBoxes
    const [nx, ny, nz] = this.dims
    const free = new Uint8Array(this.solid.length)
    for (let id = 0; id < free.length; id++) free[id] = this.solid[id] && !removed.has(id) ? 1 : 0
    const row = (x: number, y: number, z: number, w: number) => {
      const start = (y * nz + z) * nx + x
      for (let k = 0; k < w; k++) if (!free[start + k]) return false
      return true
    }
    const out: number[] = []
    const [ox, oy, oz] = this.origin
    for (let y = 0; y < ny; y++) {
      for (let z = 0; z < nz; z++) {
        for (let x = 0; x < nx; x++) {
          if (!free[(y * nz + z) * nx + x]) continue
          let w = 1
          while (x + w < nx && free[(y * nz + z) * nx + x + w]) w++
          let d = 1
          while (z + d < nz && row(x, y, z + d, w)) d++
          let h = 1
          for (; y + h < ny; h++) {
            let whole = true
            for (let k = 0; k < d && whole; k++) whole = row(x, y + h, z + k, w)
            if (!whole) break
          }
          for (let j = 0; j < h; j++) for (let k = 0; k < d; k++) free.fill(0, ((y + j) * nz + z + k) * nx + x, ((y + j) * nz + z + k) * nx + x + w)
          out.push(ox + x * CELL, oy + y * CELL, oz + z * CELL, ox + (x + w) * CELL, oy + (y + h) * CELL, oz + (z + d) * CELL)
        }
      }
    }
    const result = Int32Array.from(out)
    if (removed.size === 0) this.wholeBoxes = result
    return result
  }

  /** The faces of the whole building, worked out once: the drawing shares them. */
  private whole: { tris: Int32Array; roof: Uint8Array } | null = null

  /**
   * Mirrors `faces`: what the building is drawn as, the outer faces of its
   * standing cubes (not in `removed`), wall and roof merged apart, as triangles of nine centimetre
   * numbers each.
   */
  faces(removed: ReadonlySet<number>): Int32Array {
    return this.facesWithRoof(removed).tris
  }

  /** `faces`, and for each triangle 1 when it is a roof cube's. */
  facesWithRoof(removed: ReadonlySet<number>): { tris: Int32Array; roof: Uint8Array } {
    if (removed.size === 0 && this.whole) return this.whole
    const keys = new Int32Array(this.solid.length)
    for (let id = 0; id < keys.length; id++) keys[id] = this.solid[id] && !removed.has(id) ? this.roofed[id]! : -1
    const out: number[] = []
    const roof: number[] = []
    const p = [0, 0, 0]
    const corner = (f: Face, du: number, dv: number) => {
      p[f.d] = this.origin[f.d]! + (f.side > 0 ? f.i + 1 : f.i) * CELL
      p[f.u] = this.origin[f.u]! + (f.a + du * f.w) * CELL
      p[f.v] = this.origin[f.v]! + (f.b + dv * f.h) * CELL
      out.push(p[0]!, p[1]!, p[2]!)
    }
    greedyFaces(this.dims, keys, (f) => {
      const order = f.side > 0 ? [0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1] : [0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1]
      for (let k = 0; k < 12; k += 2) corner(f, order[k]!, order[k + 1]!)
      roof.push(f.key, f.key)
    })
    const result = { tris: Int32Array.from(out), roof: Uint8Array.from(roof) }
    if (removed.size === 0) this.whole = result
    return result
  }
}

const shells = new WeakMap<{ buildings: unknown }, Map<number, Shell | null>>()

/**
 * Building `index` of `tile`, cut into cells, once: the cells the importer baked, or for a building
 * with none (a world built by hand) its wall and roof triangles from the tile's collision mesh, cut
 * as the importer cuts them. Null when it has no cubes.
 */
export function shellOf(
  tile: { buildings: readonly { first: number; walls: number; roof: number; cells?: Parameters<typeof Shell.fromCells>[0] | null }[]; collision: { vertices: Float32Array; triangles: Uint32Array } },
  index: number,
): Shell | null {
  let byIndex = shells.get(tile)
  if (!byIndex) shells.set(tile, (byIndex = new Map()))
  if (byIndex.has(index)) return byIndex.get(index)!
  const building = tile.buildings[index]
  let shell: Shell | null = null
  if (building?.cells) {
    shell = Shell.fromCells(building.cells)
  } else if (building) {
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
