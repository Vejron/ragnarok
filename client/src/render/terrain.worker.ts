/// <reference lib="webworker" />
import { Terrain } from '@/sim/terrain'
import { coverPixels, paintCover, reliefPixels, terrainBuffers, TERRAIN_LOD_ERRORS, type Pixels } from './terrainmesh'
import { transferables, type TerrainReply, type TerrainRequest } from './terrainworker'

/*
 * Builds tiles' terrain off the main thread (`TerrainWorker`): the simplified mesh, the relief map
 * and the painted cover. Every tile built here stays in `terrain` until dropped, so a tile is shaded
 * with the neighbours already built, those are shaded again along the new seam, and the tile can be
 * meshed again at another level of detail.
 */

declare const self: DedicatedWorkerGlobalScope

let terrain: Terrain | null = null
/** Each built tile's relief map and slope map, by `col * 65536 + row`, to shade its seams again. */
const reliefs = new Map<number, { relief: Pixels; detail: Int8Array | undefined }>()

const NEIGHBOURS = [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]] as const

const key = (col: number, row: number) => col * 65536 + row
const errorOf = (level: number) => TERRAIN_LOD_ERRORS[Math.min(Math.max(level, 0), TERRAIN_LOD_ERRORS.length - 1)]!

function reply(message: TerrainReply, transfer: Transferable[] = []) {
  self.postMessage(message, transfer)
}

/** A copy of `relief` to send, so the worker keeps its own. */
function copy(relief: Pixels): Pixels {
  return { ...relief, data: relief.data.slice() }
}

self.onmessage = (event: MessageEvent<TerrainRequest>) => {
  const request = event.data
  if (request.kind === 'init') {
    terrain = new Terrain(request.halfExtent, request.tileSize)
  } else if (request.kind === 'drop') {
    terrain?.delete({ col: request.col, row: request.row, hash: 0 })
    reliefs.delete(key(request.col, request.row))
  } else if (request.kind === 'remesh') {
    const ground = terrain?.get(request.col, request.row)
    if (!ground) {
      reply({ kind: 'failed', id: request.id, message: 'tile not built' })
      return
    }
    const buffers = terrainBuffers(ground, errorOf(request.level))
    reply({ kind: 'meshed', id: request.id, buffers }, transferables(buffers))
  } else if (terrain) {
    const { id, col, row, ground, level, detail } = request
    try {
      terrain.set({ col, row, hash: 0 }, ground)
      const buffers = terrainBuffers(ground, errorOf(level))
      const relief = reliefPixels(ground, terrain, detail)
      reliefs.set(key(col, row), { relief, detail })
      const size = ground.cell * (ground.side - 1)
      const pixels = coverPixels(size)
      const canvas = new OffscreenCanvas(pixels, pixels)
      const ctx = canvas.getContext('2d')
      let cover: ImageBitmap | null = null
      if (ctx) {
        paintCover(ctx, request, ground.x0, ground.z0, size, pixels)
        cover = canvas.transferToImageBitmap()
      }
      const sent = copy(relief)
      const transfer: Transferable[] = [...transferables(buffers), sent.data.buffer]
      if (cover) transfer.push(cover)
      reply({ kind: 'built', id, buffers, cover, relief: sent }, transfer)
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
      reply({ kind: 'failed', id, message: error instanceof Error ? error.message : String(error) })
    }
  }
}
