import type { Node } from '@ragnarok/engine'
import { TankModel, parseTankAsset, type TankLayout } from './tank'
import { STRV_TRACK_HALF } from '@/sim/consts'

export const STRV_MODEL_URL = `${import.meta.env.BASE_URL}models/strvs.glb`

/** Radius of the road wheels in the model (`WHEEL_RADIUS` in build_strvs.py). */
const WHEEL_RADIUS = 0.4
/** The driver sits in the hull on the left, under a round hatch in the roof; the cupola on the right is the absent commander's. */
const LAYOUT: TankLayout = { driverHead: [-0.6, 0.85, 0.45], gunnerHead: null, wheelRadius: WHEEL_RADIUS, trackHalf: STRV_TRACK_HALF }

let asset: Node | null = null

/** Parses `strvs.glb` (built by `tools/vehicles/build_strvs.py`) for every `StrvModel` to share. */
export async function parseStrvModel(data: ArrayBuffer) {
  asset = await parseTankAsset(data)
}

export async function loadStrvModel(url = STRV_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parseStrvModel(await response.arrayBuffer())
}

/**
 * The Strv S: a turretless tank shaped as the Stridsvagn 103, with its gun fixed in the hull. It is a `TankModel` with
 * no turret node: the gun node nods, and the whole tank turns to aim sideways. The one head shown is
 * the driver's, who also fires.
 */
export class StrvModel extends TankModel {
  constructor(id: number) {
    super(id, asset, LAYOUT)
  }
}
