import type { Node } from '@ragnarok/engine'
import { TankModel, parseTankAsset, type TankLayout } from './tank'
import { STRF_TRACK_HALF } from '@/sim/consts'

export const STRF_MODEL_URL = `${import.meta.env.BASE_URL}models/strf.glb`

/** Radius of the road wheels in the model (`WHEEL_RADIUS` in build_strf.py). */
const WHEEL_RADIUS = 0.36
/**
 * Swedish splinter camo, base coat first: light green with dark green and black, then a darker green
 * with the same two, and a winter-faded one.
 */
const PAINTS = [
  [0x6b7a4a, 0x2f3b24, 0x1c1d1a],
  [0x4e5a35, 0x7c8a58, 0x1c1d1a],
  [0x8a8f6a, 0x3d4a2c, 0x23241f],
] as const
/**
 * The driver's head shows in the hatch at the front left, beside the engine deck; the gunner's in the
 * hatch on the turret's left, in the turret's frame. The 40 mm barrel kicks back a short way.
 */
const LAYOUT: TankLayout = { driverHead: [-0.75, 0.78, -2.25], gunnerHead: [-0.4, 0.42, 0.25], wheelRadius: WHEEL_RADIUS, trackHalf: STRF_TRACK_HALF, paints: PAINTS, recoilStroke: 0.18 }

let asset: Node | null = null

/** Parses `strf.glb` (built by `tools/vehicles/build_strf.py`) for every `StrfModel` to share. */
export async function parseStrfModel(data: ArrayBuffer) {
  asset = await parseTankAsset(data)
}

export async function loadStrfModel(url = STRF_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parseStrfModel(await response.arrayBuffer())
}

/**
 * The Strf 90: a tracked infantry fighting vehicle, a `TankModel` with a smaller turret set forward
 * and a long thin autocannon. The troops in the back are sealed in and never shown.
 */
export class StrfModel extends TankModel {
  constructor(id: number) {
    super(id, asset, LAYOUT)
  }
}
