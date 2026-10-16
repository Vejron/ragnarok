import { BALLAST_EXTRA, COVER_FARM, COVER_INDUSTRIAL, COVER_SAND, COVER_WATER, NO_WATER, RAIL_TRAIN, ROAD_PATH, type Point2, type TileData, type TileRef } from '@/world'
import type { TileGround } from './terrain'

/**
 * What the ground is made of, for the dust, smoke and marks a tyre leaves. Nothing simulates it:
 * a jeep grips the same on grass as on tarmac.
 */
export const SURFACE_GRASS = 0
export const SURFACE_ASPHALT = 1
export const SURFACE_DIRT = 2
export const SURFACE_SAND = 3
export const SURFACE_WATER = 4

/** Side of a surface cell, metres. */
export const SURFACE_CELL = 2
/** Water deeper than this (metres) splashes instead of leaving a mark. */
const WADING_DEPTH = 0.15

function coverSurface(kind: number): number | null {
  switch (kind) {
    case COVER_WATER: return SURFACE_WATER
    case COVER_FARM: return SURFACE_DIRT
    case COVER_INDUSTRIAL: return SURFACE_DIRT
    case COVER_SAND: return SURFACE_SAND
    // Woods, parks and built-up land stay grass: the cover's own colour is what the terrain shows.
    default: return null
  }
}

/** One tile's surface grid: `side` cells a side, `SURFACE_CELL` metres each, the first at `(x0, z0)`. */
export interface TileSurface {
  x0: number
  z0: number
  side: number
  cells: Uint8Array
}

/**
 * Rasterises `tile`'s ground cover, tracks, roads and water into a grid of surfaces. Covers are
 * painted in their order, then the ballast of railway tracks (gravel), then roads over them, as the
 * terrain is drawn; water that is deep enough wins. A tile without `rails` has no tracks.
 */
export function rasterSurface(tile: Pick<TileData, 'cover' | 'roads' | 'water'> & Partial<Pick<TileData, 'rails'>>, x0: number, z0: number, size: number, ground: TileGround | null = null): TileSurface {
  const side = Math.max(1, Math.ceil(size / SURFACE_CELL))
  const cells = new Uint8Array(side * side)
  const centre = (index: number) => (index + 0.5) * SURFACE_CELL

  for (const cover of tile.cover) {
    const surface = coverSurface(cover.kind)
    if (surface === null) continue
    // Even-odd over every ring, so holes stay as they were.
    for (let row = 0; row < side; row++) {
      const z = z0 + centre(row)
      const crossings: number[] = []
      for (const ring of cover.rings) {
        for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
          const [ax, az] = ring[i]!
          const [bx, bz] = ring[j]!
          if (az > z !== bz > z) crossings.push(ax + ((z - az) / (bz - az)) * (bx - ax))
        }
      }
      crossings.sort((a, b) => a - b)
      for (let k = 0; k + 1 < crossings.length; k += 2) {
        const from = Math.max(0, Math.ceil((crossings[k]! - x0) / SURFACE_CELL - 0.5))
        const to = Math.min(side - 1, Math.floor((crossings[k + 1]! - x0) / SURFACE_CELL - 0.5))
        for (let column = from; column <= to; column++) cells[row * side + column] = surface
      }
    }
  }

  /** Paints every cell whose centre is within `half` of the line through `points`. */
  const stroke = (points: readonly Point2[], half: number, surface: number) => {
    for (let i = 1; i < points.length; i++) {
      const [ax, az] = points[i - 1]!
      const [bx, bz] = points[i]!
      const dx = bx - ax
      const dz = bz - az
      const length2 = dx * dx + dz * dz
      const column0 = Math.max(0, Math.floor((Math.min(ax, bx) - half - x0) / SURFACE_CELL))
      const column1 = Math.min(side - 1, Math.floor((Math.max(ax, bx) + half - x0) / SURFACE_CELL))
      const row0 = Math.max(0, Math.floor((Math.min(az, bz) - half - z0) / SURFACE_CELL))
      const row1 = Math.min(side - 1, Math.floor((Math.max(az, bz) + half - z0) / SURFACE_CELL))
      for (let row = row0; row <= row1; row++) {
        for (let column = column0; column <= column1; column++) {
          const px = x0 + centre(column) - ax
          const pz = z0 + centre(row) - az
          const t = length2 > 0 ? Math.min(1, Math.max(0, (px * dx + pz * dz) / length2)) : 0
          const ex = px - t * dx
          const ez = pz - t * dz
          if (ex * ex + ez * ez <= half * half) cells[row * side + column] = surface
        }
      }
    }
  }
  // A tram's rails are set in the street, which the roads paint.
  for (const rail of tile.rails ?? []) if (rail.kind === RAIL_TRAIN) stroke(rail.points, (rail.gauge + BALLAST_EXTRA) / 2, SURFACE_DIRT)
  for (const road of tile.roads) stroke(road.points, road.width / 2, road.kind === ROAD_PATH ? SURFACE_DIRT : SURFACE_ASPHALT)

  if (ground && tile.water.length === ground.heights.length) {
    for (let row = 0; row < side; row++) {
      for (let column = 0; column < side; column++) {
        const gx = Math.round((centre(column) + x0 - ground.x0) / ground.cell)
        const gz = Math.round((centre(row) + z0 - ground.z0) / ground.cell)
        if (gx < 0 || gz < 0 || gx >= ground.side || gz >= ground.side) continue
        const sample = gz * ground.side + gx
        const water = tile.water[sample]!
        if (water !== NO_WATER && water - ground.heights[sample]! > WADING_DEPTH) cells[row * side + column] = SURFACE_WATER
      }
    }
  }
  return { x0, z0, side, cells }
}

/** The streamed-in tiles' surfaces, looked up by position. */
export class Surfaces {
  private readonly tiles = new Map<number, TileSurface>()

  constructor(
    private readonly halfExtent: number,
    private readonly tileSize: number,
  ) {}

  private key(col: number, row: number) {
    return row * 65536 + col
  }

  /** Rasterises and keeps `tile`'s surface. */
  add(ref: TileRef, tile: Parameters<typeof rasterSurface>[0], ground: TileGround | null) {
    const x0 = -this.halfExtent + ref.col * this.tileSize
    const z0 = -this.halfExtent + ref.row * this.tileSize
    this.tiles.set(this.key(ref.col, ref.row), rasterSurface(tile, x0, z0, this.tileSize, ground))
  }

  delete(ref: TileRef) {
    this.tiles.delete(this.key(ref.col, ref.row))
  }

  /** The surface under `(x, z)`: grass where nothing else is, or where the tile is not loaded. */
  at(x: number, z: number): number {
    const grid = Math.max(1, Math.ceil((2 * this.halfExtent) / this.tileSize))
    const cell = (v: number) => Math.min(Math.max(Math.floor((v + this.halfExtent) / this.tileSize), 0), grid - 1)
    const surface = this.tiles.get(this.key(cell(x), cell(z)))
    if (!surface) return SURFACE_GRASS
    const column = Math.floor((x - surface.x0) / SURFACE_CELL)
    const row = Math.floor((z - surface.z0) / SURFACE_CELL)
    if (column < 0 || row < 0 || column >= surface.side || row >= surface.side) return SURFACE_GRASS
    return surface.cells[row * surface.side + column]!
  }
}
