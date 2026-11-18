// What the client's per-kind if-chains said before each kind's presentation became its file in
// `kinds/`, kept here to hold the files to them (`kinds.spec.ts`).

import { KIND_HELI, KIND_PLANE, KIND_ROADSTER, KIND_SPORTS, KIND_STRV, KIND_TANK } from '@/net/codec'
import { JEEP_WHEELS, PLANE_WHEELS, SPORTS_WHEELS, STRV_TRACKS, TRACK_WHEELS } from '@/sim/slip'
import { isTracked } from '@/sim/vehicles'

/** The kinds there were then, which these hold to what they did; a kind added since has nothing to keep. */
export const BEFORE = [1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13]

/** `wheelLayout`. */
export function wheels(kind: number) {
  if (kind === KIND_HELI) return null
  if (kind === KIND_PLANE) return PLANE_WHEELS
  if (kind === KIND_STRV) return STRV_TRACKS
  if (isTracked(kind)) return TRACK_WHEELS
  if (kind === KIND_SPORTS || kind === KIND_ROADSTER) return SPORTS_WHEELS
  return JEEP_WHEELS
}
/** `VehicleFx`: which wheels are driven, what comes out of it, and the backfire. */
export const driven = (kind: number, wheel: number) => kind !== KIND_PLANE && (kind !== KIND_SPORTS || wheel >= 2)
export const exhaust = (kind: number) => (kind === KIND_HELI ? null : kind === KIND_PLANE ? 'castor' : kind === KIND_ROADSTER ? null : 'pipe')
export const backfire = (kind: number) => kind === KIND_SPORTS
/** `engineBay`. */
export function bay(kind: number): [number, number, number] {
  if (kind === KIND_TANK) return [0, 0.8, 2.4]
  if (kind === KIND_STRV) return [0, 0.75, 2.6]
  if (kind === KIND_HELI) return [0, 1.6, 0.4]
  if (kind === KIND_PLANE) return [0, 0.5, -1.8]
  return [0, 0.75, -1.35]
}
