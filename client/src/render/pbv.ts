import type { Node } from '@ragnarok/engine'
import { TankModel, parseTankAsset, type TankLayout } from './tank'
import { PBV_TRACK_HALF } from '@/sim/consts'
import { assetUrl } from '@/assets'

export const PBV_MODEL_URL = assetUrl('models/pbv.glb')

/** Radius of the road wheels in the model (`WHEEL_RADIUS` in build_pbv.py). */
const WHEEL_RADIUS = 0.33
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
 * The driver's head shows in the hatch in the middle of the front; the gunner's in the hatch on the
 * turret's roof, in the turret's frame. The 20 mm barrel kicks back a short way.
 */
const LAYOUT: TankLayout = { driverHead: [0, 0.85, -1.4], gunnerHead: [0, 0.37, 0.12], wheelRadius: WHEEL_RADIUS, trackHalf: PBV_TRACK_HALF, paints: PAINTS, recoilStroke: 0.1 }

let asset: Node | null = null

/** Parses `pbv.glb` (built by `tools/vehicles/build_pbv.py`) for every `PbvModel` to share. */
export async function parsePbvModel(data: ArrayBuffer) {
  asset = await parseTankAsset(data)
}

export async function loadPbvModel(url = PBV_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parsePbvModel(await response.arrayBuffer())
}

/**
 * The Pbv 302: an amphibious tracked troop carrier, a `TankModel` with a small turret at the front left
 * and a thin 20 mm gun. The troops in the back are sealed in and never shown.
 */
export class PbvModel extends TankModel {
  constructor(id: number) {
    super(id, asset, LAYOUT)
  }
}
