/// <reference lib="webworker" />
import { Shell } from '@/sim/carve'
import type { RubbleMemo } from './shards'
import { drawLook, heapRubble } from './lookjobs'
import { transferablesOf, type LookReply, type LookRequest, type LookSource } from './lookworker'
import { pack } from './packing'

/*
 * Draws hurt buildings with a look off the main thread (`LookWorker`). Each building is told once
 * and kept: its own walls and roof and its cells, and the mounds of its rubble heap. A request is
 * what is gone from it now; the answer is its redraw and the sections that fell with it
 * (`drawLook`), or its heap (`heapRubble`), handed back without a copy.
 */

declare const self: DedicatedWorkerGlobalScope

const buildings = new Map<string, { source: LookSource; shell: Shell; mounds: RubbleMemo }>()

self.onmessage = (event: MessageEvent<LookRequest>) => {
  const request = event.data
  try {
    if (request.source) buildings.set(request.building, { source: request.source, shell: Shell.ofFlags(request.source.shell), mounds: new Map() })
    const building = buildings.get(request.building)
    if (!building) throw new Error(`look worker: building ${request.building} was never told`)
    if (request.kind === 'heap') {
      // Packed here, so the main thread only copies it to the GPU.
      const heap = pack(heapRubble(building.shell, request.ask, building.mounds))
      const reply: LookReply = { kind: 'heaped', id: request.id, ...heap }
      self.postMessage(reply, transferablesOf(heap))
      return
    }
    const drawn = drawLook(building.source, building.shell, request.ask)
    pack(drawn)
    for (const section of drawn.sections) pack(section)
    const reply: LookReply = { kind: 'drawn', id: request.id, ...drawn }
    self.postMessage(reply, transferablesOf(drawn))
  } catch (error) {
    const reply: LookReply = { kind: 'failed', id: request.id, message: error instanceof Error ? error.message : String(error) }
    self.postMessage(reply)
  }
}
