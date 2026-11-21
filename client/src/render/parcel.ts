import { Group, Mesh, boxGeometry, type Geometry, type Material } from '@ragnarok/engine'
import { LyingBackpacks } from './backpack'
import { toonMaterial } from './toon'

/** A delivery's parcel (DELIVERY.md): the courier's insulated cube of a bag, teal with a white band and a lid strap. */

const BAG = 0x14a38f
export const PARCEL_TRIM = 0xf4f1e8
const STRAP = 0x1b2429

let parts: { body: Geometry; band: Geometry; strap: Geometry; bag: Material; trim: Material; dark: Material } | null = null

/**
 * The bag, 0.42 m wide, 0.44 m tall and 0.34 m deep, its straps' side towards -Z (worn, against the
 * back of a soldier facing -Z), with its bottom at the origin.
 */
export function buildParcel(): Group {
  parts ??= {
    body: boxGeometry(0.42, 0.44, 0.34),
    band: boxGeometry(0.43, 0.07, 0.35),
    strap: boxGeometry(0.06, 0.02, 0.36),
    bag: toonMaterial({ color: BAG }),
    trim: toonMaterial({ color: PARCEL_TRIM }),
    dark: toonMaterial({ color: STRAP }),
  }
  const group = new Group()
  const body = new Mesh(parts.body, parts.bag)
  body.position.y = 0.22
  group.add(body)
  const band = new Mesh(parts.band, parts.trim)
  band.position.y = 0.3
  group.add(band)
  // The lid's strap, over the top from back to front.
  const strap = new Mesh(parts.strap, parts.dark)
  strap.position.y = 0.445
  group.add(strap)
  return group
}

/** Parcels lying on the ground, dropped by couriers who died: bobbing over a pale ring, as the backpack does. */
export const lyingParcels = () => new LyingBackpacks(buildParcel, PARCEL_TRIM, 1.4)
