import { vehicleDef } from './vehicles'

/** Half extents of a vehicle kind's chassis box. Unknown kinds are jeeps. */
export function halfExtentsOf(kind: number | undefined): readonly [number, number, number] {
  return vehicleDef(kind).halfExtents
}
