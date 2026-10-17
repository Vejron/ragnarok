import { decode, encode } from '@msgpack/msgpack'
import { CAR_SEATS, MAX_PACKET, PROTOCOL_VERSION, STANCE_PRONE } from '@/sim/consts'
import { HOTBAR_SLOTS, type ItemSlot } from '@/sim/items'

/**
 * Wire format: MessagePack arrays with a leading tag, fixed-point integers, no field names.
 * Mirrors `crates/shared/src/protocol.rs`; NETWORKING.md documents every message.
 */

const C_INPUT = 0
const C_PING = 1
const C_RTC = 2

const S_WELCOME = 0
const S_SNAPSHOT = 1
const S_PONG = 2
const S_LEAVE = 3
const S_KILL = 4
const S_SCORES = 5
const S_DESTROYED = 6
const S_DAMAGED = 7
const S_HIT = 8
const S_RTC = 9
const S_CARVED = 10
const S_PROFILES = 11
const S_BARK = 12
/** A `Hit` bearing for harm with no direction. */
const BEARING_NONE = -32768

/** `Destroyed` entry kinds: what part of the static world fell. */
export const OBJ_TREE = 0
export const OBJ_BUILDING = 1

export const KIND_STICKMAN = 0
export const KIND_JEEP = 1
export const KIND_TANK = 2
export const KIND_HELI = 3
export const KIND_SPORTS = 4
/** A Hedemora rooster, an NPC. */
export const KIND_ROOSTER = 5
/** The triplane. */
export const KIND_PLANE = 6
/** The two-legged walker. */
export const KIND_WALKER = 7
/** The turretless Strv S tank, one seat. */
export const KIND_STRV = 8
/** The towed field mortar, one seat for its gunner. */
export const KIND_MORTAR = 9
/** The Tesla Roadster: a sports car with cold-gas thrusters (boost, hop and hover). Two seats. */
export const KIND_ROADSTER = 10
/** An FPV drone, flown from afar by the player who launched it. */
export const KIND_DRONE = 11
/**
 * The Orange Plague, a 40 m boss, sent to everyone wherever they are. For him `held` is his `PLAGUE_*`
 * mode, `dead` means he is toppling, `stance` is how far through his mode he is (`sim/plague-body.ts`),
 * `vy` is his health (tenths of a percent, so 10 is full) plus his `FACE_*` bits times
 * `PLAGUE_FACE_SCALE` (see `plagueHealth`), and `knockX`, `knockZ` are where his wind-up lands relative
 * to his feet, in tenths of a metre.
 */
export const KIND_PLAGUE = 12
export const PLAGUE_WALK = 0
/** A foot is raised over the target. */
export const PLAGUE_STOMP = 1
/** On the phone: an airstrike is coming down on the target. */
export const PLAGUE_STRIKE = 2
export const PLAGUE_RECOVER = 3
/** What of his face has been shot out, as bits: each eye, each ear (his left first) and his mouth. */
export const FACE_EYE_LEFT = 1 << 0
export const FACE_EYE_RIGHT = 1 << 1
export const FACE_EAR_LEFT = 1 << 2
export const FACE_EAR_RIGHT = 1 << 3
export const FACE_MOUTH = 1 << 4
export const PLAGUE_FACE_SCALE = 1024

/** The Orange Plague's health (0..1) and `FACE_*` bits, from his entity's `vy`. */
export function plagueHealth(vy: number): { health: number; face: number } {
  const raw = Math.round(vy * CM)
  const face = Math.floor(raw / PLAGUE_FACE_SCALE)
  return { health: Math.max(0, Math.min(1, (raw - face * PLAGUE_FACE_SCALE) / 1000)), face }
}
/** The Orange Plague's lines, as `BARK_*` triggers: what he is talking about. */
export const BARK_ARRIVE = 0
export const BARK_STOMP = 1
export const BARK_STRIKE = 2
export const BARK_HURT = 3
export const BARK_FIRED = 4
export const BARK_ADAPT_INFANTRY = 5
export const BARK_ADAPT_ARMOR = 6
export const BARK_ADAPT_AIR = 7
export const BARK_REZONE = 8
export const BARK_IDLE = 9
export const BARK_DOWN = 10
export const BARK_LEAVE = 11
export const BARK_NEMESIS = 12
/** An eye shot out (the subject did it); both gone; an ear shot off; his mouth shot shut. */
export const BARK_EYE = 13
export const BARK_BLIND = 14
export const BARK_EAR = 15
export const BARK_GAGGED = 16
export const FLAG_GROUNDED = 1 << 0
export const FLAG_DEAD = 1 << 1
export const FLAG_CROUCHING = 1 << 2
/** Bits 3..=6 hold the item in the player's hands. */
export const FLAG_HELD_SHIFT = 3
export const FLAG_HELD_MASK = 0b1111 << FLAG_HELD_SHIFT
/** A rooster is mid-crow. */
export const FLAG_CROWING = 1 << 7

/** The AK5. */
export const WEAPON_RIFLE = 0
export const WEAPON_SHELL = 1
export const WEAPON_ROCKET = 2
export const WEAPON_WRECK = 3
export const WEAPON_REVOLVER = 4
export const WEAPON_GRENADE = 5
/** The helicopter's nose autocannon: a shell's burst, smaller. */
export const WEAPON_AUTOCANNON = 6
export const WEAPON_SNIPER = 7
/** Shot events only: vehicle `shooter` crashed into something there. */
export const WEAPON_CRASH = 8
/** Shot events only: vehicle `shooter` is scraping along something there, throwing sparks. */
export const WEAPON_SCRAPE = 9
/** A plane's machine guns. Shot events only for a round that hit something; every round's tracer is drawn from the gun's counter. */
export const WEAPON_MG = 10
export const WEAPON_BOMB = 11
/** A vehicle running a stickman down; only ever a kill's weapon. */
export const WEAPON_ROADKILL = 12
/** Ten pellets in one pull; the shot event is where the middle of the spread ended. */
export const WEAPON_SHOTGUN = 13
/** Each further pellet of a multi-ray pull: only where it landed is drawn. */
export const WEAPON_PELLET = 14
/** A flying bullet ended here: the shot event's `shooter` is the bullet's id (`BulletSpawn.id`). */
export const WEAPON_BULLET = 15
/** A shot glanced off vehicle `shooter` at the event's point: a spark and a ping. */
export const WEAPON_RICOCHET = 16
/** The turret of tank `shooter` was shot off at the event's point. */
export const WEAPON_TURRET = 17
/** A mortar bomb going off. */
export const WEAPON_MORTAR = 18
/** An FPV drone's charge going off. */
export const WEAPON_DRONE = 19
/** The Orange Plague's foot coming down. */
export const WEAPON_STOMP = 20
/** A rocket of the Orange Plague's airstrike going off. */
export const WEAPON_AIRSTRIKE = 21

/** How a stickman died, as in a `Kill`. Mirrors `DEATH_*`. */
export const DEATH_BODY = 0
export const DEATH_HEAD = 1
export const DEATH_BLAST = 2
export const DEATH_ROADKILL = 3
export const DEATH_OVERKILL = 4

/** Indices into `VehicleState.damage`: the body's sides, then the wheels FL, FR, RL, RR. Mirrors `DAMAGE_*`. */
export const DAMAGE_FRONT = 0
export const DAMAGE_REAR = 1
export const DAMAGE_LEFT = 2
export const DAMAGE_RIGHT = 3
export const DAMAGE_WHEELS = 4
/** A tank's slot 4 (its wheels are never hurt): `TURRET_GONE` once the turret has been shot off. */
export const DAMAGE_TURRET = 4
export const TURRET_GONE = 255
/** A wheel damaged this much has come off. */
export const WHEEL_GONE = 255
const NO_DAMAGE: readonly number[] = [0, 0, 0, 0, 0, 0, 0, 0]

export const PROJECTILE_ROCKET = 0
export const PROJECTILE_GRENADE = 1
export const PROJECTILE_SHELL = 2
/** An autocannon round: flies like a shell, bursts smaller. */
export const PROJECTILE_CANNON = 3
export const PROJECTILE_BULLET = 4
/** A bomb dropped from a plane. */
export const PROJECTILE_BOMB = 5
/** A mortar bomb in flight. */
export const PROJECTILE_MORTAR = 6
/** A rocket of the Orange Plague's airstrike, falling steeply. */
export const PROJECTILE_STRIKE = 7

/** `Turret`'s flags: a field mortar is hitched to a car, so it is drawn folded up for travel. Mirrors `TURRET_STOWED`. */
export const TURRET_STOWED = 1 << 0

/** `Arms.flags`: the blip switch is cutting the engine, and either set of guns is jammed. Mirrors `ARMS_*`. */
export const ARMS_CUT = 1 << 0
export const ARMS_JAMMED = 1 << 1
export const ARMS_REAR_JAMMED = 1 << 2

/** `Thrust.flags`: the rear (boost) nozzles and the lift (hop and hover) nozzles are firing. Mirrors `THRUST_*`. */
export const THRUST_BOOST = 1 << 0
export const THRUST_LIFT = 1 << 1
/** A drone's: its pilot still flies it. A drone sends its battery as `air`. */
export const THRUST_LINKED = 1 << 2

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
  /** Lying down. */
  prone: boolean
  /** Knockback velocity from being hit by a car, m/s. */
  knockX: number
  knockZ: number
  /** `ITEM_*` in their hands. */
  held: number
  /** A rooster mid-crow. */
  crowing: boolean
  /** `STANCE_*` as sent; for the Orange Plague, how far through his mode he is. */
  stance: number
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
  /** Crash damage, 0..255 each, indexed by `DAMAGE_*`; all zeros when unhurt. */
  damage: readonly number[]
  /** A plane's pilot weapons and engine; `null` for anything else. */
  arms: Arms | null
  /** A Roadster's air tank and nozzles; `null` for anything else. */
  thrust: Thrust | null
}

/** A Roadster's thrusters. */
export interface Thrust {
  /** What is left in the tank, 0..1 of full. */
  air: number
  /** `THRUST_*`: the nozzles firing this tick. */
  flags: number
}

/** A plane's forward guns, the heat of both sets of guns, its bombs and its engine. */
export interface Arms {
  /** Forward rounds fired, modulo 256; the left gun fires the even ones. */
  shots: number
  /** 0..1; the guns jam at 1. */
  heat: number
  rearHeat: number
  bombs: number
  /** `ARMS_*`. */
  flags: number
}

/**
 * A bullet leaving a gun, in metres and m/s. It is announced once and flown by the client from this
 * (`sim/ballistics.ts`); `tick` is the server tick it was fired on.
 */
export interface BulletSpawn {
  id: number
  shooter: number
  /** The gun, `ITEM_*`. */
  item: number
  x: number
  y: number
  z: number
  vx: number
  vy: number
  vz: number
  tick: number
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

/** A vehicle's gun in the hull frame. `shots` counts shells modulo 256; `range` is a field mortar's range dial, metres. */
export interface Turret {
  /** + turns left. */
  yaw: number
  /** + is up. */
  pitch: number
  shots: number
  range: number
  /** A hitched field mortar, folded up for travel. */
  stowed: boolean
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

/** A building or tree that is gone, named by its tile and index in the tile's list. */
export interface DestroyedObject {
  /** `OBJ_*`. */
  kind: number
  col: number
  row: number
  index: number
  /** Radians: the way a tree falls (yaw 0 faces -Z, as everywhere), or where a blast came from. */
  yaw: number
}

/** A building that is hurt but stands: how badly, and where a blast marked it. */
export interface DamagedBuilding {
  col: number
  row: number
  index: number
  /** 0 (untouched) to `DAMAGE_STAGES`; the last is about to collapse. */
  stage: number
  /** Where a blast landed, in metres, when it leaves a scorch mark. */
  mark: [number, number, number] | null
}

/** Cells a blast took out of a standing building, by `Shell` cell number (`sim/carve.ts`). */
export interface CarvedBuilding {
  col: number
  row: number
  index: number
  /** Where the blast landed, metres. */
  point: [number, number, number]
  /** The cells it broke. */
  blasted: number[]
  /** The cells it left with nothing under them, which came down. */
  fell: number[]
}

export interface Score {
  id: number
  kills: number
  deaths: number
}

/**
 * A standing entity as it travels: `[id, kind, x, y, z, yaw, vy, flags, stance, knockX, knockZ]`, all
 * integers (centimetres, milliradians, cm/s). Scenes are kept in this form so delta arithmetic is
 * exact; `entityState` converts one for the game.
 */
export type RawEntity = number[]

/** A vehicle as it travels: `[id, kind, x, y, z, qx, qy, qz, steer, seats, turret | null, hp, damage | null, arms | null, thrust | null]`. */
export type RawVehicle = [number, number, number, number, number, number, number, number, number, number[], number[] | null, number, number[] | null, number[] | null, number[] | null]

/** `[id, dx, dy, dz, dyaw, dvy, flags, stance, dknockX, dknockZ]`: differences, except `id`, `flags` and `stance`. */
export type RawEntityDelta = number[]

/** `[id, dx, dy, dz, dqx, dqy, dqz, dsteer, seats | null, turret | null, hp, damage | null, arms | null, thrust | null]`. */
export type RawVehicleDelta = [number, number, number, number, number, number, number, number, number[] | null, number[] | null, number, number[] | null, number[] | null, number[] | null]

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
      bullets: BulletSpawn[]
    }
  | { type: 'pong'; nonce: number }
  | { type: 'leave'; id: number }
  /**
   * `killer` is 0 when the world did it. Totals are the new ones. `style` is `DEATH_*`; `yaw` is the way
   * the body is thrown (0 faces -Z, as for a look) and `power` how fast, m/s.
   */
  | { type: 'kill'; killer: number; victim: number; weapon: number; killerKills: number; victimDeaths: number; style: number; yaw: number; power: number }
  | { type: 'scores'; scores: Score[] }
  /** The Orange Plague says something: a `BARK_*` line, which of them, and the player it is about (0 for nobody). */
  | { type: 'bark'; trigger: number; variant: number; subject: number }
  /** What players chose: everyone's for a joiner, then each new player's. `color` indexes `PLAYER_COLORS`. */
  | { type: 'profiles'; players: { id: number; name: string; color: number }[] }
  /** Buildings and trees destroyed since the last such message; the first one a joiner gets is all of them. */
  | { type: 'destroyed'; objects: DestroyedObject[] }
  /** Buildings hurt since the last such message; the first one a joiner gets is every hurt one. */
  | { type: 'damaged'; buildings: DamagedBuilding[] }
  /** Cells blasted out of standing buildings. `live` is false for the joiner's catch-up: state, not news. */
  | { type: 'carved'; live: boolean; buildings: CarvedBuilding[] }
  /** Damage you dealt (role 0) or took (role 1); `bearing` is where a taken hit came from, null if nowhere. */
  /** Voice signaling from player `from`, relayed by the server: a session description or ICE candidate as JSON. */
  | { type: 'rtc'; from: number; kind: number; json: string }
  | { type: 'hit'; role: number; kind: number; amount: number; weapon: number; bearing: number | null; x: number; y: number; z: number; target: number }

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

/** `Rtc` kinds. */
export const RTC_OFFER = 0
export const RTC_ANSWER = 1
export const RTC_ICE = 2

/** Voice signaling for player `peer`. Goes down the signaling stream, not as a datagram. */
export function encodeRtc(peer: number, kind: number, json: string): Uint8Array {
  return encode([C_RTC, peer, kind, json])
}

export function encodePing(nonce: number): Uint8Array {
  return encode([C_PING, nonce])
}

const isInt = (value: unknown): value is number => typeof value === 'number' && Number.isInteger(value)
const ints = (values: unknown[]) => values.every(isInt)

function readEntity(raw: unknown): RawEntity | null {
  return Array.isArray(raw) && raw.length === 11 && ints(raw) ? (raw as number[]) : null
}

function readEntityDelta(raw: unknown): RawEntityDelta | null {
  return Array.isArray(raw) && raw.length === 10 && ints(raw) ? (raw as number[]) : null
}

/** Converts a validated raw entity to source units. */
export function entityState(raw: readonly number[]): EntityState {
  const [id, kind, x, y, z, yaw, vy, flags, stance, knockX, knockZ] = raw as number[]
  return {
    id: id!, kind: kind!,
    x: x! / CM, y: y! / CM, z: z! / CM,
    yaw: yaw! / MRAD,
    vy: vy! / CM,
    grounded: (flags! & FLAG_GROUNDED) !== 0,
    dead: (flags! & FLAG_DEAD) !== 0,
    crouching: (flags! & FLAG_CROUCHING) !== 0,
    prone: stance === STANCE_PRONE,
    knockX: knockX! / CM,
    knockZ: knockZ! / CM,
    held: (flags! & FLAG_HELD_MASK) >> FLAG_HELD_SHIFT,
    crowing: (flags! & FLAG_CROWING) !== 0,
    stance: stance!,
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

function decodeBullet(raw: unknown): BulletSpawn | null {
  if (!Array.isArray(raw) || raw.length !== 10 || !ints(raw)) return null
  const [id, shooter, item, x, y, z, vx, vy, vz, tick] = raw as number[]
  return { id: id!, shooter: shooter!, item: item!, x: x! / CM, y: y! / CM, z: z! / CM, vx: vx! / CM, vy: vy! / CM, vz: vz! / CM, tick: tick! }
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

/** A turret as five integers, `null` when the vehicle has none, `undefined` when malformed. */
function readTurret(raw: unknown): number[] | null | undefined {
  if (raw === null) return null
  return Array.isArray(raw) && raw.length === 5 && ints(raw) ? (raw as number[]) : undefined
}

function turretState(raw: readonly number[]): Turret {
  const [yaw, pitch, shots, range, flags] = raw as number[]
  return { yaw: yaw! / MRAD, pitch: pitch! / MRAD, shots: shots!, range: range! / 10, stowed: (flags! & TURRET_STOWED) !== 0 }
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

/** `damage`: `null`, or eight bytes. */
function isDamage(raw: unknown): boolean {
  return raw === null || (Array.isArray(raw) && raw.length === 8 && ints(raw))
}

/** `arms`: `null`, or five bytes. */
function isArms(raw: unknown): boolean {
  return raw === null || (Array.isArray(raw) && raw.length === 5 && ints(raw))
}

/** `thrust`: `null`, or two bytes. */
function isThrust(raw: unknown): boolean {
  return raw === null || (Array.isArray(raw) && raw.length === 2 && ints(raw))
}

function readVehicle(raw: unknown): RawVehicle | null {
  if (!Array.isArray(raw) || raw.length !== 15 || !isInt(raw[11]) || !ints(raw.slice(0, 9)) || !isDamage(raw[12]) || !isArms(raw[13]) || !isThrust(raw[14])) return null
  const seats = readSeats(raw[9])
  const turret = readTurret(raw[10])
  if (!seats || turret === undefined) return null
  return raw as RawVehicle
}

function readVehicleDelta(raw: unknown): RawVehicleDelta | null {
  if (!Array.isArray(raw) || raw.length !== 14 || !isInt(raw[10]) || !ints(raw.slice(0, 8)) || !isDamage(raw[11]) || !isArms(raw[12]) || !isThrust(raw[13])) return null
  if (raw[8] !== null && !readSeats(raw[8])) return null
  if (raw[9] !== null && !readTurret(raw[9])) return null
  return raw as RawVehicleDelta
}

function armsState(raw: readonly number[]): Arms {
  const [shots, heat, rearHeat, bombs, flags] = raw as number[]
  return { shots: shots!, heat: heat! / 255, rearHeat: rearHeat! / 255, bombs: bombs!, flags: flags! }
}

/** Converts a validated raw vehicle to source units. */
export function vehicleState(raw: RawVehicle): VehicleState {
  const [id, kind, x, y, z, qx, qy, qz, steer, seats, turret, hp, damage, arms, thrust] = raw
  const [fx, fy, fz] = [qx / QUAT, qy / QUAT, qz / QUAT]
  return {
    id, kind,
    x: x / CM, y: y / CM, z: z / CM,
    qx: fx, qy: fy, qz: fz, qw: Math.sqrt(Math.max(0, 1 - fx * fx - fy * fy - fz * fz)),
    steer: Math.max(-1, Math.min(1, steer / 127)),
    seats,
    turret: turret ? turretState(turret) : null,
    hp,
    damage: damage ?? NO_DAMAGE,
    arms: arms ? armsState(arms) : null,
    thrust: thrust ? { air: thrust[0]! / 255, flags: thrust[1]! } : null,
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
    entities.push([e[0]!, e[1]!, e[2]! + d[1]!, e[3]! + d[2]!, e[4]! + d[3]!, e[5]! + d[4]!, e[6]! + d[5]!, d[6]!, d[7]!, e[9]! + d[8]!, e[10]! + d[9]!])
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
    // A damage of all zeros is a repair: the vehicle is as if it was never hurt.
    const damage = d[11] === null ? v[12] : d[11].some((value) => value !== 0) ? d[11] : null
    vehicles.push([v[0], v[1], v[2] + d[1], v[3] + d[2], v[4] + d[3], v[5] + d[4], v[6] + d[5], v[7] + d[6], v[8] + d[7], d[8] ?? v[9], d[9] ?? v[10], d[10], damage, d[12] ?? v[13], d[13] ?? v[14]])
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
      if (raw.length !== 16 || !isInt(raw[1]) || !isInt(raw[2]) || !isInt(raw[3]) || !isInt(raw[4])) return null
      const [lists, removed] = [[raw[5], raw[6], raw[7], raw[8]], raw[9]]
      if (!lists.every(Array.isArray) || !Array.isArray(removed) || !ints(removed)) return null
      const me = decodeSelf(raw[11])
      if (!me || !Array.isArray(raw[12]) || !Array.isArray(raw[13]) || !Array.isArray(raw[14]) || !Array.isArray(raw[15])) return null
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
      const bullets: BulletSpawn[] = []
      for (const item of raw[15] as unknown[]) {
        const bullet = decodeBullet(item)
        if (!bullet) return null
        bullets.push(bullet)
      }
      return {
        type: 'snapshot', tick: raw[1], baseline: raw[2], ack: raw[3], ride: raw[4], delta, drive,
        hp: me.hp, progress: me.progress, inventory: me.inventory, shots, rockets, pickups, bullets,
      }
    }
    case S_PONG:
      return raw.length === 2 && isInt(raw[1]) ? { type: 'pong', nonce: raw[1] } : null
    case S_LEAVE:
      return raw.length === 2 && isInt(raw[1]) ? { type: 'leave', id: raw[1] } : null
    case S_KILL:
      if (raw.length !== 9 || !ints(raw)) return null
      return {
        type: 'kill', killer: raw[1] as number, victim: raw[2] as number, weapon: raw[3] as number, killerKills: raw[4] as number, victimDeaths: raw[5] as number,
        style: raw[6] as number, yaw: (raw[7] as number) / MRAD, power: (raw[8] as number) / 5,
      }
    case S_BARK:
      if (raw.length !== 4 || !ints(raw)) return null
      return { type: 'bark', trigger: raw[1] as number, variant: raw[2] as number, subject: raw[3] as number }
    case S_SCORES: {
      if (raw.length !== 2 || !Array.isArray(raw[1])) return null
      const scores: Score[] = []
      for (const item of raw[1] as unknown[]) {
        if (!Array.isArray(item) || item.length !== 3 || !ints(item)) return null
        scores.push({ id: item[0] as number, kills: item[1] as number, deaths: item[2] as number })
      }
      return { type: 'scores', scores }
    }
    case S_PROFILES: {
      if (raw.length !== 2 || !Array.isArray(raw[1])) return null
      const players: { id: number; name: string; color: number }[] = []
      for (const item of raw[1] as unknown[]) {
        if (!Array.isArray(item) || item.length !== 3 || !isInt(item[0]) || typeof item[1] !== 'string' || !isInt(item[2])) return null
        players.push({ id: item[0], name: item[1], color: item[2] })
      }
      return { type: 'profiles', players }
    }
    case S_DESTROYED: {
      if (raw.length !== 2 || !Array.isArray(raw[1])) return null
      const objects: DestroyedObject[] = []
      for (const item of raw[1] as unknown[]) {
        if (!Array.isArray(item) || item.length !== 5 || !ints(item)) return null
        objects.push({ kind: item[0] as number, col: item[1] as number, row: item[2] as number, index: item[3] as number, yaw: (item[4] as number) / MRAD })
      }
      return { type: 'destroyed', objects }
    }
    case S_DAMAGED: {
      if (raw.length !== 2 || !Array.isArray(raw[1])) return null
      const buildings: DamagedBuilding[] = []
      for (const item of raw[1] as unknown[]) {
        if (!Array.isArray(item) || item.length !== 8 || !ints(item)) return null
        const [col, row, index, stage, marked, x, y, z] = item as number[]
        buildings.push({ col: col!, row: row!, index: index!, stage: stage!, mark: marked ? [x! / CM, y! / CM, z! / CM] : null })
      }
      return { type: 'damaged', buildings }
    }
    case S_CARVED: {
      if (raw.length !== 3 || !isInt(raw[1]) || !Array.isArray(raw[2])) return null
      const buildings: CarvedBuilding[] = []
      for (const item of raw[2] as unknown[]) {
        if (!Array.isArray(item) || item.length !== 8) return null
        const [col, row, index, x, y, z, blasted, fell] = item as [number, number, number, number, number, number, unknown, unknown]
        if (!ints([col, row, index, x, y, z]) || !Array.isArray(blasted) || !Array.isArray(fell) || !ints(blasted) || !ints(fell)) return null
        buildings.push({ col, row, index, point: [x / CM, y / CM, z / CM], blasted: blasted as number[], fell: fell as number[] })
      }
      return { type: 'carved', live: raw[1] !== 0, buildings }
    }
    case S_RTC:
      if (raw.length !== 4 || !isInt(raw[1]) || !isInt(raw[2]) || typeof raw[3] !== 'string') return null
      return { type: 'rtc', from: raw[1], kind: raw[2], json: raw[3] }
    case S_HIT:
      if (raw.length !== 10 || !ints(raw)) return null
      return {
        type: 'hit', role: raw[1] as number, kind: raw[2] as number, amount: raw[3] as number, weapon: raw[4] as number,
        bearing: raw[5] === BEARING_NONE ? null : (raw[5] as number) / MRAD,
        x: (raw[6] as number) / CM, y: (raw[7] as number) / CM, z: (raw[8] as number) / CM, target: raw[9] as number,
      }
    default:
      return null
  }
}
