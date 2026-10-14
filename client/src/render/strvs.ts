import * as THREE from 'three'
import { TankModel, parseTankAsset, type TankLayout } from './tank'
import { STRV_TRACK_HALF } from '@/sim/consts'

export const STRV_MODEL_URL = `${import.meta.env.BASE_URL}models/strvs.glb`

/** Radius of the road wheels in the model (`WHEEL_RADIUS` in build_strvs.py). */
const WHEEL_RADIUS = 0.44
/** The driver sits in the hull at the front right, under a flat hatch in the roof, left of the gun. */
const LAYOUT: TankLayout = { driverHead: [-0.4, 0.9, -0.1], gunnerHead: null, wheelRadius: WHEEL_RADIUS, trackHalf: STRV_TRACK_HALF }

let asset: THREE.Object3D | null = null

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
 * The Strv S: a turretless tank with its gun fixed in the hull. It is a `TankModel` with
 * no turret node: the gun node nods, and the whole tank turns to aim sideways. The one head shown is
 * the driver's, who also fires.
 */
export class StrvModel extends TankModel {
  constructor(id: number) {
    super(id, asset, LAYOUT)
  }
}
