import type { Node } from '@ragnarok/engine'
import { SEAPLANE_BOMB_PIVOT } from '@/sim/consts'
import { parsePlaneAsset, PlaneModel } from './plane'

export const SEAPLANE_MODEL_URL = `${import.meta.env.BASE_URL}models/seaplane.glb`

/** Paint handed out by vehicle id: naval greys and blues, a rescue orange and a mail plane's cream. */
const PAINTS = [0x5f7786, 0xc8672a, 0x2f4d7a, 0xd8cfae] as const
let asset: Node | null = null

/** Parses `seaplane.glb` (built by `tools/vehicles/build_seaplane.py`) for every `SeaplaneModel` to share. */
export async function parseSeaplaneModel(data: ArrayBuffer) {
  asset = parsePlaneAsset(data)
}

export async function loadSeaplaneModel(url = SEAPLANE_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parseSeaplaneModel(await response.arrayBuffer())
}

/**
 * The seaplane: a two-seat biplane on twin pontoons with small wheels beside them. Its moving parts carry
 * the triplane's names (the propeller, the two cowling guns, the ring and its gun, the control surfaces
 * and the bombs), so it is drawn, armed and flown from the cockpit as a `PlaneModel` is.
 */
export class SeaplaneModel extends PlaneModel {
  constructor(id: number) {
    super(id, { asset, paints: PAINTS, bombPivot: SEAPLANE_BOMB_PIVOT })
  }
}
