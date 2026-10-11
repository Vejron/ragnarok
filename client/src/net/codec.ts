import { decode, encode } from '@msgpack/msgpack'
import { CAR_SEATS, MAX_PACKET, PROTOCOL_VERSION } from '@/sim/consts'
import { HOTBAR_SLOTS, type ItemSlot } from '@/sim/items'

/**
 * Wire format: MessagePack arrays with a leading tag, fixed-point integers, no field names.
 * Mirrors `crates/shared/src/protocol.rs`; NETWORKING.md documents every message.
 */

const C_INPUT = 0
const C_PING = 1

const S_WELCOME = 0
const S_SNAPSHOT = 1
const S_PONG = 2
const S_LEAVE = 3
const S_KILL = 4
const S_SCORES = 5

export const KIND_STICKMAN = 0
export const KIND_JEEP = 1
export const KIND_TANK = 2
export const KIND_HELI = 3
export const KIND_SPORTS = 4
export const FLAG_GROUNDED = 1 << 0
export const FLAG_DEAD = 1 << 1
export const FLAG_CROUCHING = 1 << 2
/** Bits 3..=5 hold the item in the player's hands. */
export const FLAG_HELD_SHIFT = 3
export const FLAG_HELD_MASK = 0b111 << FLAG_HELD_SHIFT

/** The AK5. */
export const WEAPON_RIFLE = 0
export const WEAPON_SHELL = 1
export const WEAPON_ROCKET = 2
export const WEAPON_WRECK = 3
export const WEAPON_REVOLVER = 4
export const WEAPON_GRENADE = 5
/** The helicopter's nose autocannon: a shell's burst, smaller. */
export const WEAPON_AUTOCANNON = 6

export const PROJECTILE_ROCKET = 0
export const PROJECTILE_GRENADE = 1
export const PROJECTILE_SHELL = 2
/** An autocannon round: flies like a shell, bursts smaller. */
export const PROJECTILE_CANNON = 3

/** One tick of player intent, already quantized: what the client predicts with is what it sends. */
export interface InputFrame {
  seq: number
  /** -127 ..= 127 */
  strafe: number
  /** -127 ..= 127 */
  forward: number
  /** milliradians, [-π, π) */
  yaw: number
  buttons: number
  /** milliradians, + is up. Aims a tank's gun and the rifle. */
  pitch: number
  /** The server tick the picture showed, modulo 65536, so the server can rewind shots to it. */
  view: number
  /** The hotbar slot in hand, 0..HOTBAR_SLOTS. */
  slot: number
}

/** One entity of a snapshot, in source units (metres, radians, m/s). */
export interface EntityState {
  id: number
  kind: number
  x: number
  y: number
  z: number
  yaw: number
  vy: number
  grounded: boolean
  /** Dead and waiting to respawn: lying where they fell. */
  dead: boolean
  /** Holding crouch. */
  crouching: boolean
  /** Knockback velocity from being hit by a car, m/s. */
  knockX: number
  knockZ: number
  /** `ITEM_*` in their hands. */
  held: number
}

/**
 * One vehicle of a snapshot. Players sitting in it are not stickman entities: `seats` lists the
 * player id in each seat (0 for empty, seat 0 is the driver).
 */
export interface VehicleState {
  id: number
  kind: number
  x: number
  y: number
  z: number
  /** Chassis orientation, unit quaternion. */
  qx: number
  qy: number
  qz: number
  qw: number
  /** Front wheel steering as a fraction of the kind's `*_MAX_STEER`, + is right. */
  steer: number
  seats: number[]
  /** The gun, for vehicles that have one. */
  turret: Turret | null
  /** Health; 0 is a burning wreck. */
  hp: number
}

/** A rocket, grenade or tank shell in flight, in metres and m/s. */
export interface RocketState {
  id: number
  /** `PROJECTILE_*`. */
  kind: number
  x: number
  y: number
  z: number
  vx: number
  vy: number
  vz: number
}

/** A vehicle's gun in the hull frame. `shots` counts shells modulo 256; `range` is unused (shells fly now) and always 0. */
export interface Turret {
  /** + turns left. */
  yaw: number
  /** + is up. */
  pitch: number
  shots: number
  range: number
}

/**
 * The exact motion of the vehicle the local player drives, which its pose does not carry. Present
 * only in the driver's snapshots. Source units: m/s, rad/s, radians.
 */
export interface DriveState {
  vx: number
  vy: number
  vz: number
  wx: number
  wy: number
  wz: number
  /** Front wheel angle, + is right. */
  steer: number
  /** The yaw rate a tank has ramped to, + is clockwise. */
  spin: number
}

/** A rifle shot, for drawing its tracer: from the shooter to where it ended, in metres. */
/** Something that happened: a rifle shot (a tracer to x, y, z), or a rocket or wreck bursting there. */
export interface ShotEvent {
  shooter: number
  weapon: number
  x: number
  y: number
  z: number
}

/** An item lying on the ground, in metres. */
export interface PickupState {
  id: number
  item: number
  x: number
  y: number
  z: number
}

export interface Score {
  id: number
  kills: number
  deaths: number
}

/**
 * A standing entity as it travels: `[id, kind, x, y, z, yaw, vy, flags, knockX, knockZ]`, all
 * integers (centimetres, milliradians, cm/s). Scenes are kept in this form so delta arithmetic is
 * exact; `entityState` converts one for the game.
 */
export type RawEntity = number[]

/** A vehicle as it travels: `[id, kind, x, y, z, qx, qy, qz, steer, seats, turret | null, hp]`. */
export type RawVehicle = [number, number, number, number, number, number, number, number, number, number[], number[] | null, number]

/** `[id, dx, dy, dz, dyaw, dvy, flags, dknockX, dknockZ]`: differences, except `id` and `flags`. */
export type RawEntityDelta = number[]

/** `[id, dx, dy, dz, dqx, dqy, dqz, dsteer, seats | null, turret | null, hp]`. */
export type RawVehicleDelta = [number, number, number, number, number, number, number, number, number[] | null, number[] | null, number]

/** Everything a client knows of the world at one snapshot, each list ordered by id. */
export interface WireScene {
  entities: RawEntity[]
  vehicles: RawVehicle[]
}

/** What a snapshot lists, relative to the baseline it names. */
export interface WireDelta {
  newEntities: RawEntity[]
  entities: RawEntityDelta[]
  newVehicles: RawVehicle[]
  vehicles: RawVehicleDelta[]
  /** Ids, of either kind, that were in the baseline and are not any more. */
  removed: number[]
}

export type ServerMessage =
  | { type: 'welcome'; version: number; id: number; tickRate: number; snapshotEvery: number; worldHash: number }
  | {
      type: 'snapshot'
      tick: number
      /** The tick of the snapshot `delta` is relative to; 0 means to nothing, so it lists everything in range. */
      baseline: number
      ack: number
      ride: number
      delta: WireDelta
      drive: DriveState | null
      /** The local player's own health; 0 is dead. */
      hp: number
      /** How far a reload or a bandage has got, 0..255, or 0 when neither is going on. */
      progress: number
      /** The local player's hotbar. */
      inventory: ItemSlot[]
      shots: ShotEvent[]
      rockets: RocketState[]
      pickups: PickupState[]
    }
  | { type: 'pong'; nonce: number }
  | { type: 'leave'; id: number }
  /** `killer` is 0 when the world did it. Totals are the new ones. */
  | { type: 'kill'; killer: number; victim: number; weapon: number; killerKills: number; victimDeaths: number }
  | { type: 'scores'; scores: Score[] }

const CM = 100
const MRAD = 1000
const QUAT = 32767

export const toCm = (metres: number) => (Number.isFinite(metres) ? Math.round(metres * CM) : 0)
export const wrapAngle = (radians: number) => radians - Math.PI * 2 * Math.floor((radians + Math.PI) / (Math.PI * 2))
export const toMrad = (radians: number) => (Number.isFinite(radians) ? Math.round(wrapAngle(radians) * MRAD) : 0)
export const fromMrad = (mrad: number) => mrad / MRAD
export const toAxis = (value: number) => (Number.isFinite(value) ? Math.round(Math.max(-1, Math.min(1, value)) * 127) : 0)
export const fromAxis = (value: number) => Math.max(-1, Math.min(1, value / 127))

/** `baseline` is the tick of the newest snapshot rebuilt so far, or 0: the server diffs against it. */
export function encodeInput(frames: readonly InputFrame[], baseline = 0): Uint8Array {
  return encode([C_INPUT, PROTOCOL_VERSION, baseline, frames.map((f) => [f.seq, f.strafe, f.forward, f.yaw, f.buttons, f.pitch, f.view, f.slot])])
}

export function encodePing(nonce: number): Uint8Array {
  return encode([C_PING, nonce])
}

const isInt = (value: unknown): value is number => typeof value === 'number' && Number.isInteger(value)
const ints = (values: unknown[]) => values.every(isInt)

function readEntity(raw: unknown): RawEntity | null {
  return Array.isArray(raw) && raw.length === 10 && ints(raw) ? (raw as number[]) : null
}

function readEntityDelta(raw: unknown): RawEntityDelta | null {
  return Array.isArray(raw) && raw.length === 9 && ints(raw) ? (raw as number[]) : null
}

/** Converts a validated raw entity to source units. */
export function entityState(raw: readonly number[]): EntityState {
  const [id, kind, x, y, z, yaw, vy, flags, knockX, knockZ] = raw as number[]
  return {
    id: id!, kind: kind!,
    x: x! / CM, y: y! / CM, z: z! / CM,
    yaw: yaw! / MRAD,
    vy: vy! / CM,
    grounded: (flags! & FLAG_GROUNDED) !== 0,
    dead: (flags! & FLAG_DEAD) !== 0,
    crouching: (flags! & FLAG_CROUCHING) !== 0,
    knockX: knockX! / CM,
    knockZ: knockZ! / CM,
    held: (flags! & FLAG_HELD_MASK) >> FLAG_HELD_SHIFT,
  }
}

function decodeShot(raw: unknown): ShotEvent | null {
  if (!Array.isArray(raw) || raw.length !== 5 || !ints(raw)) return null
  const [shooter, weapon, x, y, z] = raw as number[]
  return { shooter: shooter!, weapon: weapon!, x: x! / CM, y: y! / CM, z: z! / CM }
}

function decodeRocket(raw: unknown): RocketState | null {
  if (!Array.isArray(raw) || raw.length !== 8 || !ints(raw)) return null
  const [id, kind, x, y, z, vx, vy, vz] = raw as number[]
  return { id: id!, kind: kind!, x: x! / CM, y: y! / CM, z: z! / CM, vx: vx! / CM, vy: vy! / CM, vz: vz! / CM }
}

function decodePickup(raw: unknown): PickupState | null {
  if (!Array.isArray(raw) || raw.length !== 5 || !ints(raw)) return null
  const [id, item, x, y, z] = raw as number[]
  return { id: id!, item: item!, x: x! / CM, y: y! / CM, z: z! / CM }
}

/** `[hp, progress, [[item, count, ammo], ...]]`: what the server tells a player about themselves. */
function decodeSelf(raw: unknown): { hp: number; progress: number; inventory: ItemSlot[] } | null {
  if (!Array.isArray(raw) || raw.length !== 3 || !isInt(raw[0]) || !isInt(raw[1]) || !Array.isArray(raw[2])) return null
  if (raw[2].length > HOTBAR_SLOTS) return null
  const inventory: ItemSlot[] = []
  for (const slot of raw[2] as unknown[]) {
    if (!Array.isArray(slot) || slot.length !== 3 || !ints(slot)) return null
    inventory.push({ item: slot[0] as number, count: slot[1] as number, ammo: slot[2] as number })
  }
  return { hp: raw[0], progress: raw[1], inventory }
}

/** A turret as four integers, `null` when the vehicle has none, `undefined` when malformed. */
function readTurret(raw: unknown): number[] | null | undefined {
  if (raw === null) return null
  return Array.isArray(raw) && raw.length === 4 && ints(raw) ? (raw as number[]) : undefined
}

function turretState(raw: readonly number[]): Turret {
  const [yaw, pitch, shots, range] = raw as number[]
  return { yaw: yaw! / MRAD, pitch: pitch! / MRAD, shots: shots!, range: range! / 10 }
}

function decodeDrive(raw: unknown): DriveState | null | undefined {
  if (raw === null) return null
  if (!Array.isArray(raw) || raw.length !== 8 || !ints(raw)) return undefined
  const [vx, vy, vz, wx, wy, wz, steer, spin] = raw as number[]
  return { vx: vx! / CM, vy: vy! / CM, vz: vz! / CM, wx: wx! / MRAD, wy: wy! / MRAD, wz: wz! / MRAD, steer: steer! / MRAD, spin: spin! / MRAD }
}

function readSeats(raw: unknown): number[] | null {
  return Array.isArray(raw) && raw.length <= CAR_SEATS && ints(raw) ? (raw as number[]) : null
}

function readVehicle(raw: unknown): RawVehicle | null {
  if (!Array.isArray(raw) || raw.length !== 12 || !isInt(raw[11]) || !ints(raw.slice(0, 9))) return null
  const seats = readSeats(raw[9])
  const turret = readTurret(raw[10])
  if (!seats || turret === undefined) return null
  return raw as RawVehicle
}

function readVehicleDelta(raw: unknown): RawVehicleDelta | null {
  if (!Array.isArray(raw) || raw.length !== 11 || !isInt(raw[10]) || !ints(raw.slice(0, 8))) return null
  if (raw[8] !== null && !readSeats(raw[8])) return null
  if (raw[9] !== null && !readTurret(raw[9])) return null
  return raw as RawVehicleDelta
}

/** Converts a validated raw vehicle to source units. */
export function vehicleState(raw: RawVehicle): VehicleState {
  const [id, kind, x, y, z, qx, qy, qz, steer, seats, turret, hp] = raw
  const [fx, fy, fz] = [qx / QUAT, qy / QUAT, qz / QUAT]
  return {
    id, kind,
    x: x / CM, y: y / CM, z: z / CM,
    qx: fx, qy: fy, qz: fz, qw: Math.sqrt(Math.max(0, 1 - fx * fx - fy * fy - fz * fz)),
    steer: Math.max(-1, Math.min(1, steer / 127)),
    seats,
    turret: turret ? turretState(turret) : null,
    hp,
  }
}

/** The scene in the game's units. */
export function sceneStates(scene: WireScene): { entities: EntityState[]; vehicles: VehicleState[] } {
  return { entities: scene.entities.map(entityState), vehicles: scene.vehicles.map(vehicleState) }
}

/**
 * The scene a client holds after `delta` on top of `base`. Integer arithmetic only, mirroring
 * `shared::delta::apply`, so it agrees with what the server stored for the same snapshot.
 */
export function applyDelta(base: WireScene, delta: WireDelta): WireScene {
  const removed = new Set(delta.removed)
  const entityChanges = new Map(delta.entities.map((d) => [d[0]!, d]))
  const vehicleChanges = new Map(delta.vehicles.map((d) => [d[0], d]))
  const entities: RawEntity[] = []
  for (const e of base.entities) {
    if (removed.has(e[0]!)) continue
    const d = entityChanges.get(e[0]!)
    if (!d) {
      entities.push(e)
      continue
    }
    entities.push([e[0]!, e[1]!, e[2]! + d[1]!, e[3]! + d[2]!, e[4]! + d[3]!, e[5]! + d[4]!, e[6]! + d[5]!, d[6]!, e[8]! + d[7]!, e[9]! + d[8]!])
  }
  entities.push(...delta.newEntities)
  const vehicles: RawVehicle[] = []
  for (const v of base.vehicles) {
    if (removed.has(v[0])) continue
    const d = vehicleChanges.get(v[0])
    if (!d) {
      vehicles.push(v)
      continue
    }
    vehicles.push([v[0], v[1], v[2] + d[1], v[3] + d[2], v[4] + d[3], v[5] + d[4], v[6] + d[5], v[7] + d[6], v[8] + d[7], d[8] ?? v[9], d[9] ?? v[10], d[10]])
  }
  vehicles.push(...delta.newVehicles)
  return { entities: entities.sort((a, b) => a[0]! - b[0]!), vehicles: vehicles.sort((a, b) => a[0] - b[0]) }
}

/** Decodes any server message, or returns null for anything malformed or unknown. */
export function decodeServer(bytes: Uint8Array): ServerMessage | null {
  if (bytes.byteLength > MAX_PACKET * 4) return null
  let raw: unknown
  try {
    raw = decode(bytes)
  } catch {
    return null
  }
  if (!Array.isArray(raw) || !isInt(raw[0])) return null
  switch (raw[0]) {
    case S_WELCOME:
      if (raw.length !== 6 || !ints(raw)) return null
      {
        const [, version, id, tickRate, snapshotEvery, worldHash] = raw as number[]
        return { type: 'welcome', version: version!, id: id!, tickRate: tickRate!, snapshotEvery: snapshotEvery!, worldHash: worldHash! }
      }
    case S_SNAPSHOT: {
      if (raw.length !== 15 || !isInt(raw[1]) || !isInt(raw[2]) || !isInt(raw[3]) || !isInt(raw[4])) return null
      const [lists, removed] = [[raw[5], raw[6], raw[7], raw[8]], raw[9]]
      if (!lists.every(Array.isArray) || !Array.isArray(removed) || !ints(removed)) return null
      const me = decodeSelf(raw[11])
      if (!me || !Array.isArray(raw[12]) || !Array.isArray(raw[13]) || !Array.isArray(raw[14])) return null
      const delta: WireDelta = { newEntities: [], entities: [], newVehicles: [], vehicles: [], removed: removed as number[] }
      for (const item of raw[5] as unknown[]) {
        const entity = readEntity(item)
        if (!entity) return null
        delta.newEntities.push(entity)
      }
      for (const item of raw[6] as unknown[]) {
        const change = readEntityDelta(item)
        if (!change) return null
        delta.entities.push(change)
      }
      for (const item of raw[7] as unknown[]) {
        const vehicle = readVehicle(item)
        if (!vehicle) return null
        delta.newVehicles.push(vehicle)
      }
      for (const item of raw[8] as unknown[]) {
        const change = readVehicleDelta(item)
        if (!change) return null
        delta.vehicles.push(change)
      }
      const drive = decodeDrive(raw[10])
      if (drive === undefined) return null
      const shots: ShotEvent[] = []
      for (const item of raw[12] as unknown[]) {
        const shot = decodeShot(item)
        if (!shot) return null
        shots.push(shot)
      }
      const rockets: RocketState[] = []
      for (const item of raw[13] as unknown[]) {
        const rocket = decodeRocket(item)
        if (!rocket) return null
        rockets.push(rocket)
      }
      const pickups: PickupState[] = []
      for (const item of raw[14] as unknown[]) {
        const pickup = decodePickup(item)
        if (!pickup) return null
        pickups.push(pickup)
      }
      return {
        type: 'snapshot', tick: raw[1], baseline: raw[2], ack: raw[3], ride: raw[4], delta, drive,
        hp: me.hp, progress: me.progress, inventory: me.inventory, shots, rockets, pickups,
      }
    }
    case S_PONG:
      return raw.length === 2 && isInt(raw[1]) ? { type: 'pong', nonce: raw[1] } : null
    case S_LEAVE:
      return raw.length === 2 && isInt(raw[1]) ? { type: 'leave', id: raw[1] } : null
    case S_KILL:
      if (raw.length !== 6 || !ints(raw)) return null
      return { type: 'kill', killer: raw[1] as number, victim: raw[2] as number, weapon: raw[3] as number, killerKills: raw[4] as number, victimDeaths: raw[5] as number }
    case S_SCORES: {
      if (raw.length !== 2 || !Array.isArray(raw[1])) return null
      const scores: Score[] = []
      for (const item of raw[1] as unknown[]) {
        if (!Array.isArray(item) || item.length !== 3 || !ints(item)) return null
        scores.push({ id: item[0] as number, kills: item[1] as number, deaths: item[2] as number })
      }
      return { type: 'scores', scores }
    }
    default:
      return null
  }
}
