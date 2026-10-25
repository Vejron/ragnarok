import type { BuildingBuffers, BuildingTile } from './buildingmesh'

/*
 * The main thread's side of the buildings worker (`buildings.worker.ts`), which merges a tile's
 * building faces and fills its mesh's arrays off the main thread, so a tile arriving costs the frame
 * only the upload. The worker keeps nothing between requests: a building that is hurt later is
 * redrawn on the main thread (`rebuildBuildings`), from the cells the client keeps.
 */

/** What the worker is asked: tile `tile`'s buildings without those in `destroyed`. */
export type BuildingRequest = { kind: 'build'; id: number; salt: number; destroyed: number[]; tile: BuildingTile }

/** What the worker answers: the mesh's arrays, or null for nothing to draw. */
export type BuildingReply = { kind: 'built'; id: number; buffers: BuildingBuffers | null } | { kind: 'failed'; id: number; message: string }

interface Waiting {
  resolve: (buffers: BuildingBuffers | null) => void
  reject: (error: Error) => void
}

export class BuildingWorker {
  private readonly worker: Worker
  private readonly pending = new Map<number, Waiting>()
  private nextId = 1

  /** Null where there are no workers (tests), and the main thread builds instead. */
  static create(): BuildingWorker | null {
    if (typeof Worker === 'undefined') return null
    return new BuildingWorker()
  }

  private constructor() {
    this.worker = new Worker(new URL('./buildings.worker.ts', import.meta.url), { type: 'module', name: 'buildings' })
    this.worker.onmessage = (event: MessageEvent<BuildingReply>) => this.receive(event.data)
    this.worker.onerror = (event) => this.fail(new Error(`buildings worker: ${event.message}`))
  }

  /** The arrays of `tile`'s buildings mesh, leaving out those in `destroyed`. */
  build(tile: BuildingTile, salt: number, destroyed: ReadonlySet<number>): Promise<BuildingBuffers | null> {
    const id = this.nextId++
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
      const request: BuildingRequest = { kind: 'build', id, salt, destroyed: [...destroyed], tile }
      this.worker.postMessage(request)
    })
  }

  dispose() {
    this.worker.terminate()
    this.fail(new Error('buildings worker disposed'))
  }

  private receive(reply: BuildingReply) {
    const waiting = this.pending.get(reply.id)
    this.pending.delete(reply.id)
    if (reply.kind === 'failed') waiting?.reject(new Error(reply.message))
    else waiting?.resolve(reply.buffers)
  }

  private fail(error: Error) {
    for (const waiting of this.pending.values()) waiting.reject(error)
    this.pending.clear()
  }
}
