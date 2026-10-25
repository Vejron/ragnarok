import type { TileGround } from '@/sim/terrain'
import type { TileData } from '@/world'
import type { Draped, DrapedTile } from './groundmesh'
import type { Pixels, TerrainBuffers } from './terrainmesh'

/*
 * The main thread's side of the terrain worker (`terrain.worker.ts`), which builds a tile's terrain
 * mesh, relief map and cover texture, and the roads, rails and water draped on it (`groundmesh.ts`),
 * off the main thread, so a tile arriving costs the frame only the upload. The worker keeps its own copy of every tile's ground it has built, to shade across seams
 * and to mesh the tile again at another level of detail.
 */

/** What the worker is asked. `level` indexes `TERRAIN_LOD_ERRORS`. */
export type TerrainRequest =
  | { kind: 'init'; halfExtent: number; tileSize: number }
  | { kind: 'build'; id: number; col: number; row: number; level: number; ground: TileGround; cover: TileData['cover']; detail?: Int8Array; draped: DrapedTile }
  | { kind: 'remesh'; id: number; col: number; row: number; level: number }
  | { kind: 'drop'; col: number; row: number }

/**
 * What the worker answers. A `build` gets a `built` (or a `failed`), then a `shaded` for each
 * neighbour whose relief it changed; a `remesh` gets a `meshed` (or a `failed`).
 */
export type TerrainReply =
  | { kind: 'built'; id: number; buffers: TerrainBuffers; cover: ImageBitmap | null; relief: Pixels; draped: Draped }
  | { kind: 'meshed'; id: number; buffers: TerrainBuffers }
  | { kind: 'failed'; id: number; message: string }
  | { kind: 'shaded'; col: number; row: number; relief: Pixels }

/** A tile's terrain, ready to be wrapped in a mesh. */
export interface PreparedTerrain {
  buffers: TerrainBuffers
  cover: ImageBitmap | null
  relief: Pixels
  /** The level of detail `buffers` were simplified to. */
  level: number
  /** Its roads, rails and water. */
  draped: Draped
}

/** The transferable buffers of `buffers`, so they move to the other thread instead of being copied. */
export function transferables(buffers: TerrainBuffers): ArrayBuffer[] {
  return Object.values(buffers).map((array: Float32Array | Uint32Array) => array.buffer as ArrayBuffer)
}

interface Waiting {
  resolve: (reply: Extract<TerrainReply, { kind: 'built' | 'meshed' }>) => void
  reject: (error: Error) => void
}

export class TerrainWorker {
  private readonly worker: Worker
  private readonly pending = new Map<number, Waiting>()
  private nextId = 1

  /** Called when a neighbour's arrival changed the relief along a tile's seam. */
  onShaded: (col: number, row: number, relief: Pixels) => void = () => {}

  /** Null where there are no workers or no `OffscreenCanvas` (tests), and the main thread builds instead. */
  static create(halfExtent: number, tileSize: number): TerrainWorker | null {
    if (typeof Worker === 'undefined' || typeof OffscreenCanvas === 'undefined') return null
    return new TerrainWorker(halfExtent, tileSize)
  }

  private constructor(halfExtent: number, tileSize: number) {
    this.worker = new Worker(new URL('./terrain.worker.ts', import.meta.url), { type: 'module', name: 'terrain' })
    this.worker.onmessage = (event: MessageEvent<TerrainReply>) => this.receive(event.data)
    this.worker.onerror = (event) => this.fail(new Error(`terrain worker: ${event.message}`))
    this.post({ kind: 'init', halfExtent, tileSize })
  }

  /** Builds tile `(col, row)`'s terrain at `level`. From now on the worker shades its neighbours with it, until `drop`. */
  async build(col: number, row: number, level: number, ground: TileGround, tile: Pick<TileData, 'cover' | 'detail'> & DrapedTile): Promise<PreparedTerrain> {
    const draped: DrapedTile = { roads: tile.roads, rails: tile.rails, water: tile.water, streams: tile.streams }
    const reply = await this.ask((id) => ({ kind: 'build', id, col, row, level, ground, cover: tile.cover, ...(tile.detail ? { detail: tile.detail } : {}), draped }))
    if (reply.kind !== 'built') throw new Error(`terrain worker answered ${reply.kind}`)
    return { buffers: reply.buffers, cover: reply.cover, relief: reply.relief, level, draped: reply.draped }
  }

  /** Tile `(col, row)`'s terrain mesh simplified to `level` instead. */
  async remesh(col: number, row: number, level: number): Promise<TerrainBuffers> {
    const reply = await this.ask((id) => ({ kind: 'remesh', id, col, row, level }))
    return reply.buffers
  }

  /** Tile `(col, row)` is gone: the worker forgets its ground. */
  drop(col: number, row: number) {
    this.post({ kind: 'drop', col, row })
  }

  dispose() {
    this.worker.terminate()
    this.fail(new Error('terrain worker disposed'))
  }

  private ask(request: (id: number) => TerrainRequest) {
    const id = this.nextId++
    return new Promise<Parameters<Waiting['resolve']>[0]>((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
      this.post(request(id))
    })
  }

  private post(request: TerrainRequest) {
    this.worker.postMessage(request)
  }

  private receive(reply: TerrainReply) {
    if (reply.kind === 'shaded') {
      this.onShaded(reply.col, reply.row, reply.relief)
      return
    }
    const waiting = this.pending.get(reply.id)
    this.pending.delete(reply.id)
    if (reply.kind === 'failed') waiting?.reject(new Error(reply.message))
    else waiting?.resolve(reply)
  }

  private fail(error: Error) {
    for (const waiting of this.pending.values()) waiting.reject(error)
    this.pending.clear()
  }
}
