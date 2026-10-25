/// <reference lib="webworker" />
import init from '@/sim/wasm/sim'
import { Terrain } from '@/sim/terrain'
import { coverPixels, paintCover, reliefPixels, terrainBuffers, TERRAIN_LOD_ERRORS, type Pixels } from './terrainmesh'
import { ribbonsOf } from './groundmesh'
import { prepareTile, preparedTransferables, terrainTransferables, type CoverPainter, type TileParts } from './tileprep'
import type { TileReply, TileRequest } from './tileworker'

/*
 * Prepares streamed-in tiles off the main thread (`TileWorker`): everything `prepareTile` makes,
 * the buildings' faces merged in its own copy of the WebAssembly module. Every tile prepared here
 * keeps its ground in `terrain` until dropped, so a tile is shaded and draped with the neighbours
 * already here, those are shaded again along the new seam, and the tile can be meshed again at
 * another level of detail.
 */

declare const self: DedicatedWorkerGlobalScope

/** The WebAssembly module, for `carve_baked_faces`: awaited before the first tile with buildings. */
const ready = init()

let terrain: Terrain | null = null
/** Each prepared tile's relief map and slope map, by `col * 65536 + row`, to shade its seams again. */
const reliefs = new Map<number, { relief: Pixels; detail: Int8Array | undefined }>()

/** What each prepared tile was made from, by `col * 65536 + row`: its roads and rails, to drape on dented ground. */
const parts = new Map<number, TileParts>()

const NEIGHBOURS = [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]] as const

const key = (col: number, row: number) => col * 65536 + row
const errorOf = (level: number) => TERRAIN_LOD_ERRORS[Math.min(Math.max(level, 0), TERRAIN_LOD_ERRORS.length - 1)]!

function reply(message: TileReply, transfer: Transferable[] = []) {
  self.postMessage(message, transfer)
}

/** A copy of `relief` to send, so the worker keeps its own. */
function copy(relief: Pixels): Pixels {
  return { ...relief, data: relief.data.slice() }
}

/** Paints a tile's cover on an `OffscreenCanvas`, to send as a bitmap. */
const paint: CoverPainter = (tile, ground) => {
  const size = ground.cell * (ground.side - 1)
  const pixels = coverPixels(size)
  const ctx = new OffscreenCanvas(pixels, pixels).getContext('2d')
  if (!ctx) return null
  paintCover(ctx, tile, ground.x0, ground.z0, size, pixels)
  return ctx.canvas.transferToImageBitmap()
}

self.onmessage = async (event: MessageEvent<TileRequest>) => {
  const request = event.data
  if (request.kind === 'init') {
    terrain = new Terrain(request.halfExtent, request.tileSize)
  } else if (request.kind === 'drop') {
    terrain?.delete({ col: request.col, row: request.row, hash: 0 })
    reliefs.delete(key(request.col, request.row))
    parts.delete(key(request.col, request.row))
  } else if (request.kind === 'reground') {
    const { id, col, row, heights, level } = request
    const ground = terrain?.get(col, row)
    const kept = reliefs.get(key(col, row))
    const tile = parts.get(key(col, row))
    if (!terrain || !ground || !kept || !tile) {
      reply({ kind: 'failed', id, message: 'tile not prepared' })
      return
    }
    ground.heights = heights
    const buffers = terrainBuffers(ground, errorOf(level))
    kept.relief = reliefPixels(ground, terrain, kept.detail)
    const relief = copy(kept.relief)
    const ribbons = ribbonsOf(tile, ground, terrain)
    const transfer = [...terrainTransferables(buffers), relief.data.buffer as ArrayBuffer, ...ribbons.flatMap(({ buffers }) => [buffers.positions, buffers.normals, buffers.colors, buffers.indices].map((array) => array.buffer as ArrayBuffer))]
    reply({ kind: 'regrounded', id, buffers, relief, ribbons }, transfer)
    // The tiles beside it see its new heights along the seam.
    for (const [dc, dr] of NEIGHBOURS) {
      const next = terrain.get(col + dc, row + dr)
      const beside = reliefs.get(key(col + dc, row + dr))
      if (!next || !beside) continue
      beside.relief = reliefPixels(next, terrain, beside.detail, beside.relief)
      const shaded = copy(beside.relief)
      reply({ kind: 'shaded', col: col + dc, row: row + dr, relief: shaded }, [shaded.data.buffer])
    }
  } else if (request.kind === 'remesh') {
    const ground = terrain?.get(request.col, request.row)
    if (!ground) {
      reply({ kind: 'failed', id: request.id, message: 'tile not prepared' })
      return
    }
    const buffers = terrainBuffers(ground, errorOf(request.level))
    reply({ kind: 'meshed', id: request.id, buffers }, terrainTransferables(buffers))
  } else if (terrain) {
    const { id, col, row, input } = request
    try {
      if (input.tile.buildings.length > 0) await ready
      if (input.ground) terrain.set({ col, row, hash: 0 }, input.ground)
      parts.set(key(col, row), input.tile)
      const prepared = prepareTile(input, terrain, paint)
      if (prepared.terrain) {
        reliefs.set(key(col, row), { relief: prepared.terrain.relief, detail: input.tile.detail })
        prepared.terrain.relief = copy(prepared.terrain.relief)
      }
      reply({ kind: 'prepared', id, prepared }, preparedTransferables(prepared))
      if (!input.ground) return
      // The tiles already here were shaded without this one beside them: close the seams.
      for (const [dc, dr] of NEIGHBOURS) {
        const next = terrain.get(col + dc, row + dr)
        const kept = reliefs.get(key(col + dc, row + dr))
        if (!next || !kept) continue
        kept.relief = reliefPixels(next, terrain, kept.detail, kept.relief)
        const shaded = copy(kept.relief)
        reply({ kind: 'shaded', col: col + dc, row: row + dr, relief: shaded }, [shaded.data.buffer])
      }
    } catch (error) {
      terrain.delete({ col, row, hash: 0 })
      reliefs.delete(key(col, row))
      parts.delete(key(col, row))
      reply({ kind: 'failed', id, message: error instanceof Error ? error.message : String(error) })
    }
  }
}
