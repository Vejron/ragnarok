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
  /** The seats that work one of its mounts, whose hands are on it rather than their own guns. */
  gunners: readonly number[]
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
  /** A giant with two arms, whose pilot moves its fists (a Titan). */
  fists: boolean
  /** Where it is boarded from in the hull frame, when not its middle: a ship's helm, a ship's gun's breech (`VehicleDef::board`). */
  board: Triple | null
  /** For a helicopter: where its lever hovers it, how fast the lever moves and where translational lift is (`Rotorcraft`). Null for anything else. */
  rotor: Rotor | null
  /** Its vision slits, which a round goes straight through (`Armor::slits`). */
  slits: readonly Slit[]
  /** The side doors its crew get in and out by, which the model has as `door_FL` and `door_FR` (`VehicleDef::doors`, DOORS.md). */
  doors: readonly Door[]
}

/** A side door, hung on its front edge, that whoever sits in `seat` gets in and out by. */
export interface Door {
  seat: number
  /** Its hinge in the hull frame: the front edge, outside, at its foot. A door on the left has a negative x and swings out to the left. */
  hinge: Triple
  /** How far it reaches back from the hinge and how tall it is, metres. */
  length: number
  height: number
  /** How far it swings open, radians. */
  swing: number
  /** Where whoever got out by it stands, in the hull frame, at the hinge's height (`DoorDef::spot`). */
  spot: Triple
}

/** What the client needs of how a helicopter flies: the rest is the sim's. */
export interface Rotor {
  /** The collective lever that hovers it out of ground effect, 0..1. */
  hoverLever: number
  /** How fast its lever moves, share of its travel per second. */
  leverRate: number
  /** Through the air at about this speed it flies into clean air and shudders, m/s. */
  etlSpeed: number
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
  board: row.board ? triple(row.board) : null,
  slits: row.slits.map((slit) => ({ ...slit, centre: triple(slit.centre), half: triple(slit.half), out: triple(slit.out), pivot: triple(slit.pivot) })),
  doors: row.doors.map((door) => ({ ...door, hinge: triple(door.hinge), spot: triple(door.spot) })),
}))

/** The door of seat `index` of a `kind`, if it has one. */
export const doorOf = (kind: number | undefined, index: number): Door | null => vehicleDef(kind).doors.find((door) => door.seat === index) ?? null

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

/** Whether seat `index` of a `kind` fires its occupant's own guns: a car's passengers, but not whoever works one of its mounts. Mirrors `crew_gun`. */
export const crewGun = (kind: number | undefined, index: number): boolean => {
  const def = vehicleDef(kind)
  return def.crewGuns && index !== 0 && !def.gunners.includes(index)
}

/** Whether `kind` is a helicopter, held up by a rotor and flown with a collective lever. */
export const isRotorcraft = (kind: number | undefined): boolean => kind !== undefined && vehicleDef(kind).rotor !== null

/** A walker or a hen: a hull on two spring legs, boarded from between its feet. */
export const isLegged = (kind: number | undefined): boolean => kind !== undefined && vehicleDef(kind).legged

/**
 * A tracked tank: the tank or the Strv S, which turn on the spot, rut soft ground and rumble alike. A
 * ship is as heavy, but has no tracks under her.
 */
export const isTracked = (kind: number | undefined): boolean => kind !== undefined && vehicleDef(kind).heavy && vehicleDef(kind).trackHalf > 0

/** Whether `kind` is a drone, flown from a remote seat by whoever launched it. Mirrors `is_drone`. */
export const isDrone = (kind: number | undefined): boolean => kind !== undefined && vehicleDef(kind).drone
