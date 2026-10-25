import type { TileGround } from '@/sim/terrain'
import type { TileData } from '@/world'
import { railRibbons } from './rails'
import { roadRibbons, type Ribbon } from './roads'
import { GroundShade, heightSampler, type HeightSource } from './shade'
import { streamBuffers, surfaceBuffers, type WaterBuffers } from './watermesh'

export type { Ribbon, WaterBuffers }

/*
 * Everything a tile drapes on its ground, as plain arrays: its roads and rails (ribbons) and its
 * water. They need the tile's ground and, near its edge, its neighbours', which the terrain worker
 * (`terrain.worker.ts`) keeps, so they are built there, and on the main thread only without one.
 */

/** What of a tile is draped on its ground. */
export type DrapedTile = Pick<TileData, 'roads' | 'rails' | 'water' | 'streams'>

/** A tile's draped meshes: ribbons drawn in the tile's group, water drawn apart (`buildWater`). */
export interface Draped {
  ribbons: Ribbon[]
  water: WaterBuffers[]
}

/** The roads, rails and water of `tile`, on `ground` and, past its edge, `around`. */
export function drapedMeshes(tile: DrapedTile, ground: TileGround, around?: HeightSource): Draped {
  const height = heightSampler(ground, around)
  return { ribbons: ribbonsOf(tile, ground, around), water: waterBuffers(tile, ground, around, height) }
}

/** The roads' and rails' ribbons of `tile`, sharing one `GroundShade`. */
export function ribbonsOf(tile: Pick<DrapedTile, 'roads' | 'rails'>, ground: TileGround, around?: HeightSource): Ribbon[] {
  if (tile.roads.length === 0 && tile.rails.length === 0) return []
  const size = (ground.side - 1) * ground.cell
  const border = [ground.x0, ground.z0, ground.x0 + size, ground.z0 + size] as const
  const shade = new GroundShade(heightSampler(ground, around), ground.cell, border)
  return [...roadRibbons(tile.roads, shade, border), ...railRibbons(tile.rails, shade, border)]
}

/** The lake and river surface and the streams of `tile`. */
export function waterBuffers(tile: Pick<DrapedTile, 'water' | 'streams'>, ground: TileGround, around?: HeightSource, height = heightSampler(ground, around)): WaterBuffers[] {
  const surface = tile.water.length > 0 ? surfaceBuffers(tile.water, ground) : null
  const streams = tile.streams.length > 0 ? streamBuffers(tile.streams, height) : null
  return [surface, streams].filter((buffers): buffers is WaterBuffers => buffers !== null)
}

/** The arrays of `draped`, so they move to the other thread instead of being copied. */
export function drapedTransferables(draped: Draped): ArrayBuffer[] {
  const ribbons = draped.ribbons.flatMap(({ buffers }) => [buffers.positions, buffers.normals, buffers.colors, buffers.indices])
  const water = draped.water.flatMap((buffers) => [buffers.positions, buffers.custom, buffers.indices])
  return [...ribbons, ...water].map((array) => array.buffer as ArrayBuffer)
}
