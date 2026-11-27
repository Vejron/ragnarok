import { rasterSurface, type TileSurface } from '@/sim/surface'
import type { TileGround } from '@/sim/terrain'
import type { Building, TileData } from '@/world'
import { buildingBuffers, buildingTransferables, type BuildingBuffers } from './buildingmesh'
import { drapedMeshes, drapedTransferables, type Draped } from './groundmesh'
import { forestOf, woodsOf } from './forest'
import { gardenPlan, type GardenPlan } from './gardenplan'
import { grassMask, type GrassMask } from './grassmask'
import { wetnessPixels } from './marshnoise'
import type { HeightSource } from './shade'
import { reliefPixels, terrainBuffers, TERRAIN_LOD_ERRORS, type CoverSource, type Pixels, type TerrainBuffers } from './terrainmesh'
import { pack, packAlongside, packableTransferables } from './packing'
import { gatesOfTile, type GateSpot } from './gatespots'
import { stonesPixels } from './groundmaps'
import { shoreOf, type Shore } from './shore'
import { lineBuffers, lineTransferables, type LineBuffers } from './lines'

/*
 * Everything a streamed-in tile is turned into before it is attached, as plain arrays: its terrain,
 * what is draped on its ground, its buildings, and the rasters the main thread asks about the
 * ground. One function does it all, `prepareTile`: in the tile worker (`tile.worker.ts`), or on the
 * main thread when there is none. `world.ts` then only wraps the arrays in meshes. Nothing here
 * touches the scene, so it loads in a worker.
 */

/** What of a tile `prepareTile` reads: small enough to send to the worker (`tileParts`). */
export type TileParts = Pick<TileData, 'cover' | 'crops' | 'detail' | 'stones' | 'lines' | 'roads' | 'rails' | 'ski' | 'water' | 'reach' | 'flow' | 'foam' | 'stir' | 'lane' | 'streams' | 'bridges' | 'collision' | 'look'> & {
  buildings: readonly Pick<Building, 'first' | 'walls' | 'roof' | 'roofKind' | 'kind' | 'cells' | 'footprint' | 'holes' | 'paint' | 'clad' | 'look' | 'front'>[]
  /** The tile's trees as the forest's floor reads them (`woodsOf`). */
  woods: Float32Array
  /** Where its graves stand, x and z each: no garden is grown in a churchyard. */
  graves?: Float32Array
}

/** Where `graves` stand, x and z each. */
export function gravesOf(graves: TileData['graves']): Float32Array {
  return Float32Array.from((graves ?? []).flatMap(({ x, z }) => [x, z]))
}

/** The parts of `tile` `prepareTile` reads, leaving out what the worker has no use for (all of a tree but where it stands and what it is, the file's bytes). */
export function tileParts(tile: TileData): TileParts {
  const { cover, crops, detail, stones, lines, roads, rails, ski, water, reach, flow, foam, stir, lane, streams, bridges, collision, look } = tile
  const buildings = tile.buildings.map(({ first, walls, roof, roofKind, kind, cells, footprint, holes, paint, clad, look, front }) => ({ first, walls, roof, roofKind, kind, cells, footprint, holes, paint, clad, look: look ?? null, front }))
  return { cover, ...(crops ? { crops } : {}), ...(detail ? { detail } : {}), ...(stones ? { stones } : {}), ...(lines ? { lines } : {}), roads, rails, ski, water, ...(reach ? { reach } : {}), ...(flow ? { flow } : {}), ...(foam ? { foam } : {}), ...(stir ? { stir } : {}), ...(lane ? { lane } : {}), streams, bridges, collision, ...(look ? { look } : {}), buildings, woods: woodsOf(tile.trees), graves: gravesOf(tile.graves) }
}

/** What `prepareTile` makes a tile from. */
export interface TileInput {
  /** The tile's parts, or on the main thread all of it: its `trees` then stand for the `woods` made of them. */
  tile: Omit<TileParts, 'woods' | 'graves'> & { woods?: Float32Array; trees?: TileData['trees']; graves?: Float32Array | TileData['graves'] }
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
  /**
   * The terrain mesh at `level`, its relief map, how wet each of its texels is (`wetnessPixels`), and its
   * stony ground, shore and rock (`stonesPixels`, null where it has none); null on a flat world.
   */
  terrain: { buffers: TerrainBuffers; relief: Pixels; wet: Uint8Array | null; stones?: Pixels | null } | null
  /** Where its ground meets the water (`shoreOf`), null where it does not; absent when not worked out. */
  shore?: Shore | null
  /** The painted cover. Absent where nothing could paint it (`paint`), for the main thread to bake. */
  cover?: ImageBitmap | null
  /** Roads, rails and water; null on a flat world. */
  draped: Draped | null
  /** The merged buildings and bridges; null when there are none. */
  buildings: BuildingBuffers | null
  /** What lies under each cell (`Surfaces`), and where grass grows (`Grass`); null without `place`. */
  surface: TileSurface | null
  grass: GrassMask | null
  /** What grows in its houses' gardens (`gardenPlan`); null without `place`. */
  gardens: GardenPlan | null
  /** Where its gates are (`gatesOfTile`), for `GateView` to hang them without looking at every house. */
  gates: GateSpot[]
  /** Its fences, walls and power lines, whole (`lineBuffers`); null where it has none. Absent when not made. */
  lines?: LineBuffers | null
}

/** Paints a tile's cover, seen from above, into a bitmap, or null when there is no canvas. */
export type CoverPainter = (tile: CoverSource, ground: TileGround) => ImageBitmap | null

/**
 * Turns a tile into arrays. `around` is the ground past its edge, so its relief and its roads near
 * an edge carry on into its neighbours. `paint` paints its cover where a canvas is to hand.
 */
export function prepareTile(input: TileInput, around?: HeightSource, paint?: CoverPainter): PreparedTile {
  const { tile, ground, place } = input
  const error = TERRAIN_LOD_ERRORS[Math.min(Math.max(input.level, 0), TERRAIN_LOD_ERRORS.length - 1)]!
  const surface = place ? rasterSurface(tile, place.x0, place.z0, place.size, ground) : null
  const woods = tile.woods ?? woodsOf(tile.trees ?? [])
  const forest = place ? forestOf(woods, place.x0, place.z0, place.size) : null
  const graves = tile.graves instanceof Float32Array ? tile.graves : gravesOf(tile.graves)
  const gardens = surface ? gardenPlan(tile, surface, woods, graves) : null
  const shore = ground ? shoreOf(ground, tile.water) : null
  return {
    level: input.level,
    undrawn: [...input.undrawn],
    terrain: ground ? { ...terrainOf(tile, ground, error, around), stones: stonesPixels(tile, shore, ground) } : null,
    shore,
    ...(paint && ground ? { cover: paint({ ...tile, woods, ...(gardens ? { beds: gardens.beds } : {}) }, ground) } : {}),
    draped: ground ? drapedMeshes(tile, ground, around) : null,
    buildings: buildingBuffers(tile, input.salt, new Set(input.undrawn)),
    surface,
    grass: place && surface ? grassMask(tile, place.x0, place.z0, place.size, ground, surface, forest, gardens?.beds) : null,
    gardens,
    gates: gatesOfTile(tile),
    lines: lineBuffers(tile.lines, ground),
  }
}

function terrainOf(tile: TileInput['tile'], ground: TileGround, error: number, around?: HeightSource): NonNullable<PreparedTile['terrain']> {
  const relief = reliefPixels(ground, around, tile.detail)
  const wet = wetnessPixels(tile, ground.x0, ground.z0, ground.cell * (ground.side - 1), relief.width)
  return { buffers: terrainBuffers(ground, error), relief, wet }
}

/** The arrays of a terrain mesh, so they move to the other thread instead of being copied. */
export function terrainTransferables(buffers: TerrainBuffers): ArrayBuffer[] {
  return packableTransferables(buffers)
}

/**
 * Packs `prepared`'s big meshes into the renderer's layout (`pack`), as the tile worker does before it
 * hands a tile over: its terrain, its roads and rails, and its buildings. Its water keeps its arrays
 * as well, which the water's refinement reads. Returns it.
 */
export function packPrepared(prepared: PreparedTile): PreparedTile {
  if (prepared.terrain) pack(prepared.terrain.buffers)
  for (const ribbon of prepared.draped?.ribbons ?? []) pack(ribbon.buffers)
  for (const water of prepared.draped?.water ?? []) packAlongside(water)
  if (prepared.buildings && prepared.buildings.positions.length > 0) pack(prepared.buildings)
  return prepared
}

/** Everything in `prepared` that can move to the other thread instead of being copied. */
export function preparedTransferables(prepared: PreparedTile): Transferable[] {
  const out: Transferable[] = []
  if (prepared.terrain) out.push(...terrainTransferables(prepared.terrain.buffers), prepared.terrain.relief.data.buffer as ArrayBuffer)
  if (prepared.terrain?.wet) out.push(prepared.terrain.wet.buffer as ArrayBuffer)
  if (prepared.terrain?.stones) out.push(prepared.terrain.stones.data.buffer as ArrayBuffer)
  if (prepared.shore) out.push(prepared.shore.levels.buffer as ArrayBuffer)
  if (prepared.lines) out.push(...lineTransferables(prepared.lines))
  if (prepared.cover) out.push(prepared.cover)
  if (prepared.draped) out.push(...drapedTransferables(prepared.draped))
  if (prepared.buildings) out.push(...buildingTransferables(prepared.buildings))
  if (prepared.surface) out.push(prepared.surface.cells.buffer as ArrayBuffer)
  if (prepared.grass) out.push(prepared.grass.cells.buffer as ArrayBuffer)
  if (prepared.gardens) out.push(prepared.gardens.items.buffer as ArrayBuffer, prepared.gardens.beds.buffer as ArrayBuffer)
  return out
}
