/**
 * What each vehicle kind is, in one table: built from `tables.generated.json`, which the Rust tests
 * write from `crates/shared/src/vehicles.rs` (`UPDATE_PARITY=1 cargo test -p shared parity`). Kind-specific
 * physics is the server's (`crates/sim`, run here through `motor.ts`), and how this client presents each
 * kind (model, seats, sound, effects) is its file in `kinds/`.
 */

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
  /** Each seat's head point in the hull frame, where its occupant is drawn sitting (not for a `mountSeat`: `seatHead`). */
  seatHeads: readonly (readonly [number, number, number])[]
  /** A seat on its turning mount, shown whatever `crew` says (the mortar's one, a tank's machine gunner): the head is `head` from the mount's `pivot`, turned with the gun. */
  mountSeat: { seat: number; pivot: Triple; head: Triple } | null
  /** Its passengers (every seat but the driver's) fire the guns in their own hotbar. */
  crewGuns: boolean
  /** A hull on two spring legs, boarded from between its feet: a walker or a hen. */
  legged: boolean
  /** Tracked and heavy: turns on the spot, ruts soft ground, rumbles, and plows through trees. */
  heavy: boolean
  /** Nobody drives it: it is towed, and has no engine. */
  towed: boolean
  /** Its turret can be shot off. */
  shedsTurret: boolean
  /** Flown from afar by whoever launched it. */
  drone: boolean
  /** Its vision slits, which a round goes straight through (`Armor::slits`). */
  slits: readonly Slit[]
}

/** A vision slit: a box in the hull frame and the way it looks out; on a turret, placed at yaw 0 and turned about `pivot`. */
export interface Slit {
  centre: Triple
  half: Triple
  out: Triple
  turret: boolean
  pivot: Triple
  /** The seat that looks out of it, if any. */
  seat: number | null
}

type Triple = readonly [number, number, number]
const triple = ([x = 0, y = 0, z = 0]: number[]): Triple => [x, y, z]

/** In the server's order, the jeep first. */
export const VEHICLES: readonly VehicleDef[] = tables.vehicles.map((row) => ({
  ...row,
  halfExtents: triple(row.halfExtents),
  crew: row.crew as VehicleDef['crew'],
  seatHeads: row.seatHeads.map(triple),
  mountSeat: row.mountSeat && { seat: row.mountSeat.seat, pivot: triple(row.mountSeat.pivot), head: triple(row.mountSeat.head) },
  slits: row.slits.map((slit) => ({ ...slit, centre: triple(slit.centre), half: triple(slit.half), out: triple(slit.out), pivot: triple(slit.pivot) })),
}))

/** The definition of a vehicle `kind`. Unknown kinds are jeeps. */
export const vehicleDef = (kind: number | undefined): VehicleDef => VEHICLES.find((def) => def.kind === kind) ?? VEHICLES[0]!

/** The seat whose occupant aims and fires `kind`'s gun: seat 1, or the driver's seat 0 where there is only one. Mirrors `gun_seat`. */
export const gunSeat = (kind: number | undefined): number => (vehicleDef(kind).seats === 1 ? 0 : 1)

/**
 * Where seat `index` of a `kind` puts its occupant's head, in the hull frame, with a seat on a turning
 * mount turned to `gunYaw`; null for a crew that does not show. Mirrors `seat_head`.
 */
export const seatHead = (kind: number | undefined, index: number, gunYaw = 0): readonly [number, number, number] | null => {
  const def = vehicleDef(kind)
  if (def.mountSeat?.seat === index) {
    const [x, y, z] = def.mountSeat.head
    const [px, py, pz] = def.mountSeat.pivot
    const sin = Math.sin(gunYaw)
    const cos = Math.cos(gunYaw)
    return [px + x * cos + z * sin, py + y, pz - x * sin + z * cos]
  }
  if (def.crew === 'sealed') return null
  return def.seatHeads[index] ?? null
}

/** Whether seat `index` of a `kind` fires its occupant's own guns: a car's passengers. */
export const crewGun = (kind: number | undefined, index: number): boolean => vehicleDef(kind).crewGuns && index !== 0

/** A walker or a hen: a hull on two spring legs, boarded from between its feet. */
export const isLegged = (kind: number | undefined): boolean => kind !== undefined && vehicleDef(kind).legged

/** A tracked tank: the tank or the Strv S, which turn on the spot, rut soft ground and rumble alike. */
export const isTracked = (kind: number | undefined): boolean => kind !== undefined && vehicleDef(kind).heavy

/** Whether `kind` is a drone, flown from a remote seat by whoever launched it. Mirrors `is_drone`. */
export const isDrone = (kind: number | undefined): boolean => kind !== undefined && vehicleDef(kind).drone
