import type { TileData, TileRef, WorldData } from '@/world'
import { f32 } from './round'

/**
 * One tile's ground, positioned in the world. `heights` is row-major, rows along +Z, with `side`
 * samples a side `cell` metres apart, the first at `(x0, z0)`.
 */
export interface TileGround {
  x0: number
  z0: number
  cell: number
  side: number
  heights: Float32Array
}

/** The ground of `tile` at its place in the world, or null when the world is flat. */
export function groundOf(world: WorldData, ref: TileRef, tile: TileData): TileGround | null {
  if (world.groundCell <= 0 || tile.ground.length === 0) return null
  const side = Math.round(world.tileSize / world.groundCell) + 1
  if (tile.ground.length !== side * side) return null
  return {
    x0: -world.halfExtent + ref.col * world.tileSize,
    z0: -world.halfExtent + ref.row * world.tileSize,
    cell: world.groundCell,
    side,
    heights: tile.ground,
  }
}

/**
 * Height of `ground` under `(x, z)`, on the triangles the collider has: each cell is split from its
 * `(x0, z1)` corner to its `(x1, z0)` corner. Mirrors `Ground::height_at` in `crates/shared`.
 */
export function heightIn(ground: TileGround, x: number, z: number): number {
  const last = ground.side - 1
  const gx = Math.min(Math.max(f32((x - ground.x0) / ground.cell), 0), last)
  const gz = Math.min(Math.max(f32((z - ground.z0) / ground.cell), 0), last)
  const i = Math.min(Math.floor(gx), last - 1)
  const j = Math.min(Math.floor(gz), last - 1)
  const fx = f32(gx - i)
  const fz = f32(gz - j)
  const at = (row: number, column: number) => ground.heights[row * ground.side + column]!
  const h00 = at(j, i)
  const h10 = at(j + 1, i)
  const h01 = at(j, i + 1)
  const h11 = at(j + 1, i + 1)
  if (fx + fz <= 1) return f32(h00 + f32(fx * f32(h01 - h00)) + f32(fz * f32(h10 - h00)))
  return f32(h11 + f32(f32(1 - fx) * f32(h10 - h11)) + f32(f32(1 - fz) * f32(h01 - h11)))
}

/**
 * Lowers `ground`'s samples within `radius` cm of `(x, z)` (cm, in the world's frame) in a smooth bowl,
 * `depth` cm at the middle, never sample `at` (`j * side + i`) under `floor(at)` cm. Mirrors `Ground::dig` in `crates/shared` sample for
 * sample (the heights are whole centimetres), but on this tile alone: a crater over a seam is dug in
 * each tile it touches. Returns whether any sample changed.
 */
export function digCrater(ground: TileGround, x: number, z: number, radius: number, depth: number, floor: (at: number) => number): boolean {
  if (radius <= 0 || depth <= 0) return false
  const cell = Math.round(ground.cell * 100)
  const x0 = Math.round(ground.x0 * 100)
  const z0 = Math.round(ground.z0 * 100)
  const last = ground.side - 1
  const clamp = (v: number) => Math.min(Math.max(v, 0), last)
  const i0 = clamp(Math.ceil((x - radius - x0) / cell))
  const i1 = clamp(Math.floor((x + radius - x0) / cell))
  const j0 = clamp(Math.ceil((z - radius - z0) / cell))
  const j1 = clamp(Math.floor((z + radius - z0) / cell))
  const r2 = radius * radius
  let changed = false
  for (let j = j0; j <= j1; j++) {
    for (let i = i0; i <= i1; i++) {
      const dx = x0 + i * cell - x
      const dz = z0 + j * cell - z
      const d2 = dx * dx + dz * dz
      if (d2 >= r2) continue
      const t = (r2 - d2) / r2
      const delta = Math.round(depth * t * t)
      const at = j * ground.side + i
      const old = Math.round(ground.heights[at]! * 100)
      const lowered = Math.max(old - delta, Math.min(floor(at), old))
      if (lowered === old) continue
      ground.heights[at] = f32(lowered / 100)
      changed = true
    }
  }
  return changed
}

/** The streamed-in tiles' ground, to look up heights for the camera, spawning and effects. */
export class Terrain {
  private readonly tiles = new Map<number, TileGround>()

  constructor(
    private readonly halfExtent: number,
    private readonly tileSize: number,
  ) {}

  private key(col: number, row: number) {
    return row * 65536 + col
  }

  set(ref: TileRef, ground: TileGround | null) {
    if (ground) this.tiles.set(this.key(ref.col, ref.row), ground)
  }

  delete(ref: TileRef) {
    this.tiles.delete(this.key(ref.col, ref.row))
  }

  /** The ground of tile `(col, row)`, if it is loaded. */
  get(col: number, row: number): TileGround | undefined {
    return this.tiles.get(this.key(col, row))
  }

  /** Ground height under `(x, z)`, or null where its tile is not loaded. */
  sample(x: number, z: number): number | null {
    const grid = Math.max(1, Math.ceil((2 * this.halfExtent) / this.tileSize))
    const cell = (v: number) => Math.min(Math.max(Math.floor((v + this.halfExtent) / this.tileSize), 0), grid - 1)
    const ground = this.tiles.get(this.key(cell(x), cell(z)))
    return ground ? heightIn(ground, x, z) : null
  }

  /** Ground height under `(x, z)`; 0 where the tile is not loaded (the plane every world has underneath). */
  heightAt(x: number, z: number): number {
    return this.sample(x, z) ?? 0
  }
}
