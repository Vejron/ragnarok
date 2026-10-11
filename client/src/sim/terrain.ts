import type { TileData, TileRef, WorldData } from '@/world'
import { f32 } from './movement'

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
