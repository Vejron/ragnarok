/**
 * What each vehicle kind is, in one table: built from `tables.generated.json`, which the Rust tests
 * write from `crates/shared/src/vehicles.rs` (`UPDATE_PARITY=1 cargo test -p shared parity`). Kind-specific
 * physics is the server's (`crates/sim`, run here through `motor.ts`) and models are in `render/`.
 */

import { KIND_MORTAR } from '@/net/codec'
import { MORTAR_GUN_PIVOT, MORTAR_SEAT_HEAD } from './consts'
import tables from './tables.generated.json'

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
  /** A hull on two spring legs, boarded from between its feet: a walker or a hen. */
  legged: boolean
  /** Tracked and heavy: turns on the spot, ruts soft ground, rumbles, and plows through trees. */
  heavy: boolean
}

type Triple = readonly [number, number, number]
const triple = ([x = 0, y = 0, z = 0]: number[]): Triple => [x, y, z]

/** In the server's order, the jeep first. */
export const VEHICLES: readonly VehicleDef[] = tables.vehicles.map((row) => ({
  ...row,
  halfExtents: triple(row.halfExtents),
  crew: row.crew as VehicleDef['crew'],
  seatHeads: row.seatHeads.map(triple),
}))

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
export const isLegged = (kind: number | undefined): boolean => kind !== undefined && vehicleDef(kind).legged

/** A tracked tank: the tank or the Strv S, which turn on the spot, rut soft ground and rumble alike. */
export const isTracked = (kind: number | undefined): boolean => kind !== undefined && vehicleDef(kind).heavy
