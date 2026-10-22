/**
 * What each vehicle kind is, in one table. Mirrors `crates/shared/src/vehicles.rs`; `parity.spec.ts`
 * checks it against `fixtures/parity/gameplay.json`. Kind-specific physics is the server's
 * (`crates/sim`, run here through `motor.ts`) and models are in `render/`.
 */

import { KIND_DRONE, KIND_HELI, KIND_HEN, KIND_JEEP, KIND_LOITER, KIND_MORTAR, KIND_PLANE, KIND_ROADSTER, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_WALKER } from '@/net/codec'
import {
  CAR_SEATS, DRONE_HALF_EXTENTS, HELI_SEAT_HEADS, JEEP_SEAT_HEADS, MORTAR_GUN_PIVOT, MORTAR_SEAT_HEAD, PLANE_SEAT_HEADS, ROADSTER_SEAT_HEADS, SPORTS_SEAT_HEADS, DRONE_SEATS, LOITER_HALF_EXTENTS, LOITER_SEATS, HELI_HALF_EXTENTS, HELI_SEATS, HEN_HALF_EXTENTS, HEN_SEATS, JEEP_AXLE_Z, JEEP_HALF_EXTENTS, JEEP_TRACK_HALF, MORTAR_HALF_EXTENTS, MORTAR_SEATS, PLANE_HALF_EXTENTS, PLANE_SEATS,
  ROADSTER_AXLE_Z, ROADSTER_HALF_EXTENTS, ROADSTER_SEATS, ROADSTER_TRACK_HALF,
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
  /** Has a plane's pilot weapons (forward guns that heat and jam, bombs). A hen's eggs are its bombs. */
  arms: boolean
  /** A car's four wheels, which can be bent and lost; `trackHalf` and `axleZ` place them. */
  carWheels: boolean
  trackHalf: number
  axleZ: number
  /** Whether its crew can be shot: `sealed` inside armour, `open` above the hull, `glazed` behind glass the rounds go through. */
  crew: 'sealed' | 'open' | 'glazed'
  /** Each seat's head point in the hull frame, where its occupant is drawn sitting (not the mortar's: `seatHead`). */
  seatHeads: readonly (readonly [number, number, number])[]
  /** Its passengers (every seat but the driver's) fire the guns in their own hotbar. */
  crewGuns: boolean
}

const none = { gun: false, arms: false, carWheels: false, trackHalf: 0, axleZ: 0, crew: 'sealed', seatHeads: [], crewGuns: false } as const

export const VEHICLES: readonly VehicleDef[] = [
  { kind: KIND_JEEP, name: 'Jeep', hp: 100, seats: CAR_SEATS, halfExtents: JEEP_HALF_EXTENTS, ...none, carWheels: true, trackHalf: JEEP_TRACK_HALF, axleZ: JEEP_AXLE_Z, crew: 'open', seatHeads: JEEP_SEAT_HEADS, crewGuns: true },
  { kind: KIND_TANK, name: 'Tank', hp: 250, seats: TANK_SEATS, halfExtents: TANK_HALF_EXTENTS, ...none, gun: true },
  { kind: KIND_HELI, name: 'Helicopter', hp: 100, seats: HELI_SEATS, halfExtents: HELI_HALF_EXTENTS, ...none, gun: true, crew: 'glazed', seatHeads: HELI_SEAT_HEADS },
  { kind: KIND_SPORTS, name: 'Sports car', hp: 100, seats: SPORTS_SEATS, halfExtents: SPORTS_HALF_EXTENTS, ...none, carWheels: true, trackHalf: SPORTS_TRACK_HALF, axleZ: SPORTS_AXLE_Z, crew: 'open', seatHeads: SPORTS_SEAT_HEADS, crewGuns: true },
  { kind: KIND_PLANE, name: 'Triplane', hp: 70, seats: PLANE_SEATS, halfExtents: PLANE_HALF_EXTENTS, ...none, gun: true, arms: true, crew: 'open', seatHeads: PLANE_SEAT_HEADS },
  { kind: KIND_WALKER, name: 'Walker', hp: 200, seats: WALKER_SEATS, halfExtents: WALKER_HALF_EXTENTS, ...none, gun: true },
  { kind: KIND_STRV, name: 'Strv S', hp: 220, seats: STRV_SEATS, halfExtents: STRV_HALF_EXTENTS, ...none, gun: true },
  { kind: KIND_MORTAR, name: 'Field mortar', hp: 150, seats: MORTAR_SEATS, halfExtents: MORTAR_HALF_EXTENTS, ...none, gun: true, crew: 'open' },
  { kind: KIND_ROADSTER, name: 'Roadster', hp: 100, seats: ROADSTER_SEATS, halfExtents: ROADSTER_HALF_EXTENTS, ...none, carWheels: true, trackHalf: ROADSTER_TRACK_HALF, axleZ: ROADSTER_AXLE_Z, crew: 'open', seatHeads: ROADSTER_SEAT_HEADS, crewGuns: true },
  { kind: KIND_DRONE, name: 'FPV drone', hp: 1, seats: DRONE_SEATS, halfExtents: DRONE_HALF_EXTENTS, ...none },
  { kind: KIND_HEN, name: 'Hen', hp: 180, seats: HEN_SEATS, halfExtents: HEN_HALF_EXTENTS, ...none, arms: true },
  { kind: KIND_LOITER, name: 'Loitering drone', hp: 40, seats: LOITER_SEATS, halfExtents: LOITER_HALF_EXTENTS, ...none },
]

/** The definition of a vehicle `kind`. Unknown kinds are jeeps. */
export const vehicleDef = (kind: number | undefined): VehicleDef => VEHICLES.find((def) => def.kind === kind) ?? VEHICLES[0]!

/** The seat whose occupant aims and fires `kind`'s gun: seat 1, or the driver's seat 0 where there is only one. Mirrors `gun_seat`. */
export const gunSeat = (kind: number | undefined): number => (vehicleDef(kind).seats === 1 ? 0 : 1)

/**
 * Where seat `index` of a `kind` puts its occupant's head, in the hull frame, with the mortar's mount
 * turned to `gunYaw`; null for a crew that does not show. Mirrors `seat_head`.
 */
export const seatHead = (kind: number | undefined, index: number, gunYaw = 0): readonly [number, number, number] | null => {
  const def = vehicleDef(kind)
  if (def.crew === 'sealed') return null
  if (def.kind === KIND_MORTAR) {
    if (index !== 0) return null
    const [x, y, z] = MORTAR_SEAT_HEAD
    const [px, py, pz] = MORTAR_GUN_PIVOT
    const sin = Math.sin(gunYaw)
    const cos = Math.cos(gunYaw)
    return [px + x * cos + z * sin, py + y, pz - x * sin + z * cos]
  }
  return def.seatHeads[index] ?? null
}

/** Whether seat `index` of a `kind` fires its occupant's own guns: a car's passengers. */
export const crewGun = (kind: number | undefined, index: number): boolean => vehicleDef(kind).crewGuns && index !== 0

/** A walker or a hen: a hull on two spring legs, boarded from between its feet. */
export const isLegged = (kind: number | undefined): boolean => kind === KIND_WALKER || kind === KIND_HEN

/** A tracked tank: the tank or the Strv S, which turn on the spot, rut soft ground and rumble alike. */
export const isTracked = (kind: number | undefined): boolean => kind === KIND_TANK || kind === KIND_STRV
