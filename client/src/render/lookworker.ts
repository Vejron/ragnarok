import type { CollisionMesh } from '@/world'
import type { ShellFlags } from '@/sim/carve'
import type { LookRange, Surfaces } from './lookfracture'
import type { RGB } from './geom'
import { insideFor } from '@/sim/interior'

/*
 * The main thread's side of the look worker (`look.worker.ts`), which redraws hurt buildings with a
 * look (`lookFracture`) off the main thread: on a slow machine a big house took a tenth of a second
 * to redraw, and it is redrawn every time a round or a blast takes something out of it. The worker
 * is told each building once (its own walls and roof, and its cells), then only what is gone from it.
 */

/** One building, as the worker keeps it: its own triangles of the look (`range` counts from 0), and its cells. */
export interface LookSource {
  look: CollisionMesh
  range: LookRange
  shell: ShellFlags
}

/** What a redraw is asked for: what is gone from the building, and its colours and surfaces. */
export interface LookAsk {
  removed: Uint32Array
  /** Cell and damage taken, in pairs. */
  dents: Float64Array
  wall: RGB
  roof: RGB
  surfaces: Surfaces
}

/** A redrawn building, ready to be a `Geometry`: three numbers a corner, four of `custom`. */
export interface LookBuffers {
  positions: Float32Array
  colors: Float32Array
  custom: Float32Array
  normals: Float32Array
}

export type LookRequest = { kind: 'draw'; id: number; building: string; source: LookSource | null; ask: LookAsk }

export type LookReply = ({ kind: 'drawn'; id: number } & LookBuffers) | { kind: 'failed'; id: number; message: string }

interface Waiting {
  resolve: (buffers: LookBuffers) => void
  reject: (error: Error) => void
}

/**
 * Building `range`'s own triangles of `look`, numbered from 0: all the worker needs of the tile's look
 * to draw it (`facadeFor`, `openingsFor` and `eavesFor` read only a building's own range).
 */
export function lookSlice(look: CollisionMesh, range: LookRange): { look: CollisionMesh; range: LookRange } {
  // What stands in its rooms goes along worked out: the worker has no WebAssembly to work it out with.
  insideFor(look, range)
  const count = range.walls + range.roof
  const triangles = new Uint32Array(count * 3)
  const renumbered = new Map<number, number>()
  const vertices: number[] = []
  for (let k = 0; k < count * 3; k++) {
    const v = look.triangles[range.first * 3 + k]!
    let at = renumbered.get(v)
    if (at === undefined) {
      at = renumbered.size
      renumbered.set(v, at)
      vertices.push(look.vertices[v * 3]!, look.vertices[v * 3 + 1]!, look.vertices[v * 3 + 2]!)
    }
    triangles[k] = at
  }
  return { look: { vertices: Float32Array.from(vertices), triangles }, range: { first: 0, walls: range.walls, roof: range.roof, inside: range.inside ?? new Int32Array(0) } }
}

export class LookWorker {
  private readonly worker: Worker
  private readonly pending = new Map<number, Waiting>()
  /** The buildings the worker has been told about. */
  private readonly known = new Set<string>()
  private nextId = 1

  /** Null where there are no workers (tests), and hurt buildings are redrawn on the main thread instead. */
  static create(): LookWorker | null {
    if (typeof Worker === 'undefined') return null
    return new LookWorker()
  }

  private constructor() {
    this.worker = new Worker(new URL('./look.worker.ts', import.meta.url), { type: 'module', name: 'looks' })
    this.worker.onmessage = (event: MessageEvent<LookReply>) => this.receive(event.data)
    this.worker.onerror = (event) => this.fail(new Error(`look worker: ${event.message}`))
  }

  /**
   * Building `building` (any name that stays its own) redrawn as `ask` has it. `source` is asked for
   * only the first time the worker hears of the building.
   */
  draw(building: string, source: () => LookSource, ask: LookAsk): Promise<LookBuffers> {
    const id = this.nextId++
    const told = this.known.has(building)
    this.known.add(building)
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
      const request: LookRequest = { kind: 'draw', id, building, source: told ? null : source(), ask }
      this.worker.postMessage(request, [ask.removed.buffer, ask.dents.buffer])
    })
  }

  dispose() {
    this.worker.terminate()
    this.fail(new Error('look worker disposed'))
  }

  private receive(reply: LookReply) {
    const waiting = this.pending.get(reply.id)
    this.pending.delete(reply.id)
    if (reply.kind === 'failed') waiting?.reject(new Error(reply.message))
    else waiting?.resolve(reply)
  }

  private fail(error: Error) {
    for (const waiting of this.pending.values()) waiting.reject(error)
    this.pending.clear()
    // Whatever it knew is gone with it.
    this.known.clear()
  }
}
