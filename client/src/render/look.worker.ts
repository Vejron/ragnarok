/// <reference lib="webworker" />
import { Geometry } from '@ragnarok/engine'
import { Shell } from '@/sim/carve'
import { lookFracture } from './lookfracture'
import type { LookReply, LookRequest, LookSource } from './lookworker'

/*
 * Redraws hurt buildings with a look off the main thread (`LookWorker`). Each building is told once
 * and kept: its own walls and roof and its cells. A request is what is gone from it now; the answer
 * is the soup `lookFracture` makes and its normals, handed back without a copy.
 */

declare const self: DedicatedWorkerGlobalScope

const buildings = new Map<string, { source: LookSource; shell: Shell }>()

self.onmessage = (event: MessageEvent<LookRequest>) => {
  const request = event.data
  try {
    if (request.source) buildings.set(request.building, { source: request.source, shell: Shell.ofFlags(request.source.shell) })
    const building = buildings.get(request.building)
    if (!building) throw new Error(`look worker: building ${request.building} was never told`)
    const { ask } = request
    const dents = new Map<number, number>()
    for (let i = 0; i < ask.dents.length; i += 2) dents.set(ask.dents[i]!, ask.dents[i + 1]!)
    const soup = lookFracture(building.source.look, building.source.range, building.shell, new Set(ask.removed), dents, ask.wall, ask.roof, ask.surfaces)
    // Flat shading, as `shardGeometry` gives it.
    const geometry = new Geometry({ positions: soup.positions })
    geometry.computeNormals()
    const normals = geometry.normals!
    const reply: LookReply = { kind: 'drawn', id: request.id, positions: soup.positions, colors: soup.colors, custom: soup.custom, normals }
    self.postMessage(reply, [soup.positions.buffer, soup.colors.buffer, soup.custom.buffer, normals.buffer])
  } catch (error) {
    const reply: LookReply = { kind: 'failed', id: request.id, message: error instanceof Error ? error.message : String(error) }
    self.postMessage(reply)
  }
}
