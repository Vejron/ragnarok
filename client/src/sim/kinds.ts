import { KIND_HELI, KIND_SPORTS, KIND_TANK } from '@/net/codec'
import { HELI_HALF_EXTENTS, JEEP_HALF_EXTENTS, SPORTS_HALF_EXTENTS, TANK_HALF_EXTENTS } from './consts'

/** Half extents of a vehicle kind's chassis box. Unknown kinds are jeeps. */
export function halfExtentsOf(kind: number | undefined): readonly [number, number, number] {
  if (kind === KIND_TANK) return TANK_HALF_EXTENTS
  if (kind === KIND_HELI) return HELI_HALF_EXTENTS
  if (kind === KIND_SPORTS) return SPORTS_HALF_EXTENTS
  return JEEP_HALF_EXTENTS
}
