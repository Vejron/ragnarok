/// <reference lib="webworker" />
import init from '@/sim/wasm/sim'
import { buildingBuffers, buildingTransferables } from './buildingmesh'
import type { BuildingReply, BuildingRequest } from './buildingworker'

/*
 * Builds tiles' building meshes off the main thread (`BuildingWorker`): the faces merged in its own
 * copy of the WebAssembly module (`carve_baked_faces`), then the arrays filled.
 */

declare const self: DedicatedWorkerGlobalScope

const ready = init()

self.onmessage = async (event: MessageEvent<BuildingRequest>) => {
  const { id, salt, destroyed, tile } = event.data
  try {
    await ready
    const buffers = buildingBuffers(tile, salt, new Set(destroyed))
    const reply: BuildingReply = { kind: 'built', id, buffers }
    self.postMessage(reply, buffers ? buildingTransferables(buffers) : [])
  } catch (error) {
    const reply: BuildingReply = { kind: 'failed', id, message: error instanceof Error ? error.message : String(error) }
    self.postMessage(reply)
  }
}
