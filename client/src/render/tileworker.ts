import type { TerrainBuffers, Pixels } from './terrainmesh'
import type { PreparedTile, TileInput } from './tileprep'

/*
 * The main thread's side of the tile worker (`tile.worker.ts`), which prepares each streamed-in tile
 * off the main thread (`prepareTile`), so a tile arriving costs the frame only wrapping its arrays
 * in meshes and the upload. The worker keeps every prepared tile's ground, to shade and drape across
 * seams and to mesh the terrain again at another level of detail.
 */

/** What the worker is asked. */
export type TileRequest =
  | { kind: 'init'; halfExtent: number; tileSize: number }
  | { kind: 'prepare'; id: number; col: number; row: number; input: TileInput }
  | { kind: 'remesh'; id: number; col: number; row: number; level: number }
  | { kind: 'drop'; col: number; row: number }

/**
 * What the worker answers. A `prepare` gets a `prepared` (or a `failed`), then a `shaded` for each
 * neighbour whose relief it changed; a `remesh` gets a `meshed` (or a `failed`).
 */
export type TileReply =
  | { kind: 'prepared'; id: number; prepared: PreparedTile }
  | { kind: 'meshed'; id: number; buffers: TerrainBuffers }
  | { kind: 'failed'; id: number; message: string }
  | { kind: 'shaded'; col: number; row: number; relief: Pixels }

type Answer = Extract<TileReply, { kind: 'prepared' | 'meshed' }>

interface Waiting {
  resolve: (reply: Answer) => void
  reject: (error: Error) => void
}

export class TileWorker {
  private readonly worker: Worker
  private readonly pending = new Map<number, Waiting>()
  private nextId = 1

  /** Called when a neighbour's arrival changed the relief along a tile's seam. */
  onShaded: (col: number, row: number, relief: Pixels) => void = () => {}

  /** Null where there are no workers or no `OffscreenCanvas` (tests), and the main thread prepares tiles instead. */
  static create(halfExtent: number, tileSize: number): TileWorker | null {
    if (typeof Worker === 'undefined' || typeof OffscreenCanvas === 'undefined') return null
    return new TileWorker(halfExtent, tileSize)
  }

  private constructor(halfExtent: number, tileSize: number) {
    this.worker = new Worker(new URL('./tile.worker.ts', import.meta.url), { type: 'module', name: 'tiles' })
    this.worker.onmessage = (event: MessageEvent<TileReply>) => this.receive(event.data)
    this.worker.onerror = (event) => this.fail(new Error(`tile worker: ${event.message}`))
    this.post({ kind: 'init', halfExtent, tileSize })
  }

  /** Prepares tile `(col, row)`. From now on the worker shades and drapes its neighbours with its ground, until `drop`. */
  async prepare(col: number, row: number, input: TileInput): Promise<PreparedTile> {
    const reply = await this.ask((id) => ({ kind: 'prepare', id, col, row, input }))
    if (reply.kind !== 'prepared') throw new Error(`tile worker answered ${reply.kind}`)
    return reply.prepared
  }

  /** Tile `(col, row)`'s terrain mesh simplified to `level` instead. */
  async remesh(col: number, row: number, level: number): Promise<TerrainBuffers> {
    const reply = await this.ask((id) => ({ kind: 'remesh', id, col, row, level }))
    if (reply.kind !== 'meshed') throw new Error(`tile worker answered ${reply.kind}`)
    return reply.buffers
  }

  /** Tile `(col, row)` is gone: the worker forgets its ground. */
  drop(col: number, row: number) {
    this.post({ kind: 'drop', col, row })
  }

  dispose() {
    this.worker.terminate()
    this.fail(new Error('tile worker disposed'))
  }

  private ask(request: (id: number) => TileRequest): Promise<Answer> {
    const id = this.nextId++
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
      this.post(request(id))
    })
  }

  private post(request: TileRequest) {
    this.worker.postMessage(request)
  }

  private receive(reply: TileReply) {
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
