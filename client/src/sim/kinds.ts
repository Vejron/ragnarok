import { KIND_HELI, KIND_TANK } from '@/net/codec'
import { CAR_HALF_EXTENTS, HELI_HALF_EXTENTS, TANK_HALF_EXTENTS } from './consts'

/** Half extents of a vehicle kind's chassis box. Unknown kinds are cars. */
export function halfExtentsOf(kind: number | undefined): readonly [number, number, number] {
  if (kind === KIND_TANK) return TANK_HALF_EXTENTS
  if (kind === KIND_HELI) return HELI_HALF_EXTENTS
  return CAR_HALF_EXTENTS
}
