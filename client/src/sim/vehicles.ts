/**
 * What each vehicle kind is, in one table. Mirrors `crates/shared/src/vehicles.rs`; `parity.spec.ts`
 * checks it against `fixtures/parity/gameplay.json`. Kind-specific physics lives in `vehicle.ts`
 * and models in `render/`.
 */

import { KIND_HELI, KIND_JEEP, KIND_MORTAR, KIND_PLANE, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_WALKER } from '@/net/codec'
import {
  CAR_SEATS, HELI_HALF_EXTENTS, HELI_SEATS, JEEP_AXLE_Z, JEEP_HALF_EXTENTS, JEEP_TRACK_HALF, MORTAR_HALF_EXTENTS, MORTAR_SEATS, PLANE_HALF_EXTENTS, PLANE_SEATS,
  SPORTS_AXLE_Z, SPORTS_HALF_EXTENTS, SPORTS_SEATS, SPORTS_TRACK_HALF, STRV_HALF_EXTENTS, STRV_SEATS, TANK_HALF_EXTENTS, TANK_SEATS, WALKER_HALF_EXTENTS, WALKER_SEATS,
} from './consts'

export interface VehicleDef {
  kind: number
  name: string
  /** Health of a fresh one. */
  hp: number
  /** Seat 0 drives (or pilots); on a tank, helicopter or plane seat 1 is the gunner. */
  seats: number
  /** Half extents of the chassis box. */
  halfExtents: readonly [number, number, number]
  /** Has a turret or mounted gun the gunner aims. */
  gun: boolean
  /** Has a plane's pilot weapons (forward guns that heat and jam, bombs). */
  arms: boolean
  /** A car's four wheels, which can be bent and lost; `trackHalf` and `axleZ` place them. */
  carWheels: boolean
  trackHalf: number
  axleZ: number
}

const none = { gun: false, arms: false, carWheels: false, trackHalf: 0, axleZ: 0 }

export const VEHICLES: readonly VehicleDef[] = [
  { kind: KIND_JEEP, name: 'Jeep', hp: 100, seats: CAR_SEATS, halfExtents: JEEP_HALF_EXTENTS, ...none, carWheels: true, trackHalf: JEEP_TRACK_HALF, axleZ: JEEP_AXLE_Z },
  { kind: KIND_TANK, name: 'Tank', hp: 250, seats: TANK_SEATS, halfExtents: TANK_HALF_EXTENTS, ...none, gun: true },
  { kind: KIND_HELI, name: 'Helicopter', hp: 100, seats: HELI_SEATS, halfExtents: HELI_HALF_EXTENTS, ...none, gun: true },
  { kind: KIND_SPORTS, name: 'Sports car', hp: 100, seats: SPORTS_SEATS, halfExtents: SPORTS_HALF_EXTENTS, ...none, carWheels: true, trackHalf: SPORTS_TRACK_HALF, axleZ: SPORTS_AXLE_Z },
  { kind: KIND_PLANE, name: 'Triplane', hp: 70, seats: PLANE_SEATS, halfExtents: PLANE_HALF_EXTENTS, ...none, gun: true, arms: true },
  { kind: KIND_WALKER, name: 'Walker', hp: 200, seats: WALKER_SEATS, halfExtents: WALKER_HALF_EXTENTS, ...none, gun: true },
  { kind: KIND_STRV, name: 'Strv S', hp: 220, seats: STRV_SEATS, halfExtents: STRV_HALF_EXTENTS, ...none, gun: true },
  { kind: KIND_MORTAR, name: 'Field mortar', hp: 150, seats: MORTAR_SEATS, halfExtents: MORTAR_HALF_EXTENTS, ...none, gun: true },
]

/** The definition of a vehicle `kind`. Unknown kinds are jeeps. */
export const vehicleDef = (kind: number | undefined): VehicleDef => VEHICLES.find((def) => def.kind === kind) ?? VEHICLES[0]!

/** The seat whose occupant aims and fires `kind`'s gun: seat 1, or the driver's seat 0 where there is only one. Mirrors `gun_seat`. */
export const gunSeat = (kind: number | undefined): number => (vehicleDef(kind).seats === 1 ? 0 : 1)

/** A tracked tank: the tank or the Strv S, which turn on the spot, rut soft ground and rumble alike. */
export const isTracked = (kind: number | undefined): boolean => kind === KIND_TANK || kind === KIND_STRV
