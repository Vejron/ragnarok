import type { Node } from '@ragnarok/engine'
import { TankModel, parseTankAsset, type TankLayout } from './tank'
import { CB90_MG_BARREL_X } from '@/sim/consts'
import { assetUrl } from '@/assets'

export const CB90_MODEL_URL = assetUrl('models/cb90.glb')

/**
 * The Swedish navy's coastal splinter camo, base coat first: grey-green with a darker green and a
 * blue-grey, then a darker scheme with the same, and a faded one.
 */
const PAINTS = [
  [0x6f7a68, 0x3e4a3a, 0x4f5a62],
  [0x55604f, 0x7b8676, 0x3a434a],
  [0x858c80, 0x4b5648, 0x5c666c],
] as const
/**
 * Its crew are soldiers, drawn where they are (the driver behind the cockpit's glass, the gunner standing
 * in the ring, the riders on the after deck), so no heads are drawn in hatches. The 25 mm barrel kicks
 * back a short way.
 */
const LAYOUT: TankLayout = { driverHead: null, gunnerHead: null, wheelRadius: 1, trackHalf: 0, paints: PAINTS, recoilStroke: 0.12 }

let asset: Node | null = null

/** Parses `cb90.glb` (built by `tools/vehicles/build_cb90.py`) for every `Cb90Model` to share. */
export async function parseCb90Model(data: ArrayBuffer) {
  asset = await parseTankAsset(data)
}

export async function loadCb90Model(url = CB90_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parseCb90Model(await response.arrayBuffer())
}

/**
 * The Stridsbåt 90: a `TankModel` with no wheels, its cannon on its pedestal in the ring as the turret,
 * and the twin machine guns fixed on the cockpit as the "roof gun", which only elevate (their node is
 * the hull's, not the turret's).
 */
export class Cb90Model extends TankModel {
  constructor(id: number) {
    super(id, asset, LAYOUT)
  }

  /** One of the two machine guns in world space as of the last `update`: `side` -1 the left, 1 the right. */
  machineGun(side: number) {
    const barrel = this.pintleBarrel()
    if (!barrel) return null
    // The barrels lie either side of the cradle, square to their line and level with it.
    const right = { x: -barrel.direction.z, z: barrel.direction.x }
    const across = Math.hypot(right.x, right.z) || 1
    barrel.origin.x += (side * CB90_MG_BARREL_X * right.x) / across
    barrel.origin.z += (side * CB90_MG_BARREL_X * right.z) / across
    return barrel
  }
}
