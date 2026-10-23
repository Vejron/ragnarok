/**
 * A building cut into cells, as `crates/shared/src/carve.rs` cuts it, through the WebAssembly build
 * (`carve_*` in `crates/sim-wasm`): the server says which cell numbers a blast broke, and the client
 * draws what is left. Cutting, unpacking what the importer baked, merging the faces and finding the
 * cell a round struck are Rust's; what is here is what only the client asks of the cells.
 * The module is initialised by `SimMotor.create` before any tile is drawn.
 */

import { carve_cut, carve_faces, carve_nearest, carve_unpack } from './wasm/sim'

/** Side of a cell, centimetres, on one lattice for the whole world. Mirrors `CELL`. */
export const CELL = 50
/** Health of a cell against bullets. Mirrors `CELL_HP`. */
export const CELL_HP = 40
/** Mirrors `MAX_CELLS`. */
export const MAX_CELLS = 16_000_000

/** A triangle: three corners `[x, y, z]`, centimetres, flattened. */
export type Tri = [number, number, number, number, number, number, number, number, number]

/**
 * `count` flags, one a cell, from the bytes the importer packed them into, a layer being `layer`
 * cells (`unpack` in `carve.rs`).
 */
export const unpack = (packed: Uint8Array, count: number, layer: number): Uint8Array => carve_unpack(packed, count, layer)

/**
 * What a grid of keyed cells (`keys`: one a cell, -1 for none) is drawn as: the outer faces of its
 * cubes, those of one key merged into rectangles, as triangles of ten numbers each, the corners in
 * centimetres and then the key (`faces_of` in `carve.rs`).
 */
export const keyedFaces = (origin: readonly number[], dims: readonly number[], keys: Int32Array): Int32Array =>
  carve_faces(Int32Array.from(origin), Uint32Array.from(dims), keys)

/** A building's walls and roof as a grid of cells. */
export class Shell {
  /** Corner of the grid with the smallest coordinates, centimetres. */
  readonly origin: [number, number, number]
  readonly cell = CELL
  readonly dims: [number, number, number]
  /** 1 where the cell is part of the building. */
  private readonly solid: Uint8Array
  /** 1 where the roof passes through the cell, for drawing it in the roof's colour. */
  private readonly roofed: Uint8Array

  private constructor(origin: [number, number, number], dims: [number, number, number], solid: Uint8Array, roof: Uint8Array) {
    this.origin = origin
    this.dims = dims
    this.solid = solid
    this.roofed = roof
  }

  /** The building as the importer baked it (`Shell::from_cells`), or null when it has no cells. */
  static fromCells(cells: { origin: [number, number, number]; dims: [number, number, number]; solid: Uint8Array; roof: Uint8Array }): Shell | null {
    const [nx, ny, nz] = cells.dims
    const count = nx * ny * nz
    if (count === 0 || count > MAX_CELLS) return null
    const layer = nx * nz
    return new Shell([...cells.origin], [nx, ny, nz], unpack(cells.solid, count, layer), unpack(cells.roof, count, layer))
  }

  /** Cuts a building from its wall and roof triangles, or null when it has none or is too big. */
  static of(walls: readonly Tri[], roof: readonly Tri[]): Shell | null {
    return Shell.ofFlat(walls.flat(), roof.flat())
  }

  /** As `of`, from flat triangles, nine numbers each (`Shell::new`). */
  static ofFlat(walls: ArrayLike<number>, roof: ArrayLike<number>): Shell | null {
    const cut = carve_cut(Int32Array.from(walls), Int32Array.from(roof))
    if (!cut) return null
    try {
      const [ox, oy, oz] = cut.origin
      const [nx, ny, nz] = cut.dims
      return new Shell([ox!, oy!, oz!], [nx!, ny!, nz!], cut.solid, cut.roof)
    } finally {
      cut.free()
    }
  }

  /** Whether cell `id` is part of the building. */
  isSolid(id: number): boolean {
    return this.solid[id] === 1
  }

  /** Whether the roof passes through cell `id`. */
  isRoof(id: number): boolean {
    return this.roofed[id] === 1
  }

  /** The number of the cell at `(x, y, z)`, as `Shell::index`. */
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
    return this.cells.length
  }

  private solidIds: Int32Array | null = null

  /** The cells that hold a piece of wall or roof, in order. */
  get cells(): Int32Array {
    if (this.solidIds) return this.solidIds
    // Found natively, as most of the grid is empty.
    const found: number[] = []
    for (let id = this.solid.indexOf(1); id >= 0; id = this.solid.indexOf(1, id + 1)) found.push(id)
    const ids = new Int32Array(found)
    return (this.solidIds = ids)
  }

  /** The cell holding the point `(x, y, z)`, metres, clamped to the grid. */
  cellAt(x: number, y: number, z: number): number {
    const at = (axis: number, v: number) => Math.min(this.dims[axis]! - 1, Math.max(0, Math.floor((v * 100 - this.origin[axis]!) / this.cell)))
    return this.index(at(0, x), at(1, y), at(2, z))
  }

  /**
   * The standing cell (not in `removed`) nearest the point, metres, if its middle is within a
   * cell's reach: where a round that landed there struck, as the server's `chip` finds it.
   */
  nearestCell(x: number, y: number, z: number, removed: ReadonlySet<number> | undefined): number | null {
    const gone = removed ? Uint32Array.from(removed) : new Uint32Array()
    const id = carve_nearest(Int32Array.from(this.origin), Uint32Array.from(this.dims), this.solid, gone, Math.round(x * 100), Math.round(y * 100), Math.round(z * 100))
    return id < 0 ? null : id
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

  /** The faces of the whole building, worked out once: the drawing shares them. */
  private whole: { tris: Int32Array; roof: Uint8Array } | null = null

  /**
   * What the building is drawn as (`Shell::faces`): the outer faces of its standing cubes (not in
   * `removed`), wall and roof merged apart, as triangles of nine centimetre numbers each.
   */
  faces(removed: ReadonlySet<number>): Int32Array {
    return this.facesWithRoof(removed).tris
  }

  /** `faces`, and for each triangle 1 when it is a roof cube's. */
  facesWithRoof(removed: ReadonlySet<number>): { tris: Int32Array; roof: Uint8Array } {
    if (removed.size === 0 && this.whole) return this.whole
    const keys = new Int32Array(this.solid.length).fill(-1)
    for (const id of this.cells) if (!removed.has(id)) keys[id] = this.roofed[id]!
    const faces = keyedFaces(this.origin, this.dims, keys)
    const count = faces.length / 10
    const tris = new Int32Array(count * 9)
    const roof = new Uint8Array(count)
    for (let t = 0; t < count; t++) {
      tris.set(faces.subarray(t * 10, t * 10 + 9), t * 9)
      roof[t] = faces[t * 10 + 9]!
    }
    const result = { tris, roof }
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
