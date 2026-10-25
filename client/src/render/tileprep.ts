import { rasterSurface, type TileSurface } from '@/sim/surface'
import type { TileGround } from '@/sim/terrain'
import type { Building, TileData } from '@/world'
import { buildingBuffers, buildingTransferables, type BuildingBuffers } from './buildingmesh'
import { drapedMeshes, drapedTransferables, type Draped } from './groundmesh'
import { grassMask, type GrassMask } from './grassmask'
import type { HeightSource } from './shade'
import { reliefPixels, terrainBuffers, TERRAIN_LOD_ERRORS, type Pixels, type TerrainBuffers } from './terrainmesh'

/*
 * Everything a streamed-in tile is turned into before it is attached, as plain arrays: its terrain,
 * what is draped on its ground, its buildings, and the rasters the main thread asks about the
 * ground. One function does it all, `prepareTile`: in the tile worker (`tile.worker.ts`), or on the
 * main thread when there is none. `world.ts` then only wraps the arrays in meshes. Nothing here
 * touches the scene, so it loads in a worker.
 */

/** What of a tile `prepareTile` reads: small enough to send to the worker (`tileParts`). */
export type TileParts = Pick<TileData, 'cover' | 'detail' | 'roads' | 'rails' | 'water' | 'streams' | 'bridges' | 'collision'> & {
  buildings: readonly Pick<Building, 'first' | 'walls' | 'roof' | 'roofKind' | 'cells' | 'footprint' | 'paint'>[]
}

/** The parts of `tile` `prepareTile` reads, leaving out what the worker has no use for (trees, the file's bytes). */
export function tileParts(tile: TileData): TileParts {
  const { cover, detail, roads, rails, water, streams, bridges, collision } = tile
  const buildings = tile.buildings.map(({ first, walls, roof, roofKind, cells, footprint, paint }) => ({ first, walls, roof, roofKind, cells, footprint, paint }))
  return { cover, ...(detail ? { detail } : {}), roads, rails, water, streams, bridges, collision, buildings }
}

/** What `prepareTile` makes a tile from. */
export interface TileInput {
  tile: TileParts
  /** Its ground, or null on a flat world. */
  ground: TileGround | null
  /** Keeps building colours stable between reloads and varied across tiles. */
  salt: number
  /** The level of detail to simplify the terrain to, indexing `TERRAIN_LOD_ERRORS`. */
  level: number
  /** The buildings to leave out: fallen, or drawn as shards. */
  undrawn: readonly number[]
  /** Where the tile lies, for the surface and grass rasters: its corner and side, metres. Without it there are none. */
  place?: { x0: number; z0: number; size: number }
}

/** A tile ready to be attached. */
export interface PreparedTile {
  level: number
  /** The buildings `buildings` leaves out. */
  undrawn: number[]
  /** The terrain mesh at `level` and its relief map; null on a flat world. */
  terrain: { buffers: TerrainBuffers; relief: Pixels } | null
  /** The painted cover. Absent where nothing could paint it (`paint`), for the main thread to bake. */
  cover?: ImageBitmap | null
  /** Roads, rails and water; null on a flat world. */
  draped: Draped | null
  /** The merged buildings and bridges; null when there are none. */
  buildings: BuildingBuffers | null
  /** What lies under each cell (`Surfaces`), and where grass grows (`Grass`); null without `place`. */
  surface: TileSurface | null
  grass: GrassMask | null
}

/** Paints a tile's cover, seen from above, into a bitmap, or null when there is no canvas. */
export type CoverPainter = (tile: Pick<TileData, 'cover'>, ground: TileGround) => ImageBitmap | null

/**
 * Turns a tile into arrays. `around` is the ground past its edge, so its relief and its roads near
 * an edge carry on into its neighbours. `paint` paints its cover where a canvas is to hand.
 */
export function prepareTile(input: TileInput, around?: HeightSource, paint?: CoverPainter): PreparedTile {
  const { tile, ground, place } = input
  const error = TERRAIN_LOD_ERRORS[Math.min(Math.max(input.level, 0), TERRAIN_LOD_ERRORS.length - 1)]!
  const surface = place ? rasterSurface(tile, place.x0, place.z0, place.size, ground) : null
  return {
    level: input.level,
    undrawn: [...input.undrawn],
    terrain: ground ? { buffers: terrainBuffers(ground, error), relief: reliefPixels(ground, around, tile.detail) } : null,
    ...(paint && ground ? { cover: paint(tile, ground) } : {}),
    draped: ground ? drapedMeshes(tile, ground, around) : null,
    buildings: buildingBuffers(tile, input.salt, new Set(input.undrawn)),
    surface,
    grass: place && surface ? grassMask(tile, place.x0, place.z0, place.size, ground, surface) : null,
  }
}

/** The arrays of a terrain mesh, so they move to the other thread instead of being copied. */
export function terrainTransferables(buffers: TerrainBuffers): ArrayBuffer[] {
  return Object.values(buffers).map((array: Float32Array | Uint32Array) => array.buffer as ArrayBuffer)
}

/** Everything in `prepared` that can move to the other thread instead of being copied. */
export function preparedTransferables(prepared: PreparedTile): Transferable[] {
  const out: Transferable[] = []
  if (prepared.terrain) out.push(...terrainTransferables(prepared.terrain.buffers), prepared.terrain.relief.data.buffer as ArrayBuffer)
  if (prepared.cover) out.push(prepared.cover)
  if (prepared.draped) out.push(...drapedTransferables(prepared.draped))
  if (prepared.buildings) out.push(...buildingTransferables(prepared.buildings))
  if (prepared.surface) out.push(prepared.surface.cells.buffer as ArrayBuffer)
  if (prepared.grass) out.push(prepared.grass.cells.buffer as ArrayBuffer)
  return out
}
