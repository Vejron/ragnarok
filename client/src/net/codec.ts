import { decode, encode } from '@msgpack/msgpack'
import { MAX_PACKET, PROTOCOL_VERSION } from '@/sim/consts'
import { HOTBAR_SLOTS, type ItemSlot } from '@/sim/items'
import { decodeUpdates, type Update } from './replicated'

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
const S_NOTICE = 12
/** `Notice` kinds: what a world's scenario is telling everyone. */
export const NOTICE_SAY = 0
export const NOTICE_OBJECTIVE = 1
export const NOTICE_WON = 2
export const NOTICE_LOST = 3
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
/** The hen: a giant robot hen on the walker's legs that leaps, flaps and glides, and lays egg bombs. */
export const KIND_HEN = 12
/** A loitering drone: a Shahed-sized delta wing with a big warhead, flown from afar and always cruising. */
export const KIND_LOITER = 13

/** Whether `kind` is a drone, flown from a remote seat by whoever launched it. Mirrors `is_drone` in protocol.rs. */
export function isDrone(kind: number | undefined): boolean {
  return kind === KIND_DRONE || kind === KIND_LOITER
}
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
/** A hen's egg going off. */
export const WEAPON_EGG = 20
/** A loitering drone's warhead going off. */
export const WEAPON_LOITER = 21
/** A flare gun fired from `shooter`'s hand. */
export const WEAPON_FLARE = 22

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
/** A hen's egg, bouncing until its fuse runs out or smashing where it lands hard. */
export const PROJECTILE_EGG = 7
/** A flare: it climbs, then hangs under its parachute, burning brightly as it drifts down. */
export const PROJECTILE_FLARE = 8

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
/** A hen's: its wings are spread and still, gliding. A hen sends its stamina as `air`, and `THRUST_LIFT` while it leaps or flaps. */
export const THRUST_GLIDE = 1 << 3
/** A hen's: neither foot is on the ground. */
export const THRUST_AIR = 1 << 4

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

/** A building or tree that is gone, named by its tile and index in the tile's list. */
export interface DestroyedObject {
  /** `OBJ_*`. */
  kind: number
  col: number
  row: number
  index: number
  /** Radians: the way a tree falls (yaw 0 faces -Z, as everywhere), or where a blast came from. */
  yaw: number
  /** 0..1: how violently it went. A tank plowing through it or a rocket is near 1; 0 for the catch-up on joining. */
  force: number
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

export type ServerMessage =
  | { type: 'welcome'; version: number; id: number; tickRate: number; snapshotEvery: number; worldHash: number }
  | {
      type: 'snapshot'
      tick: number
      /** The tick of the snapshot `updates` are relative to; 0 means to nothing, so they bring everything in range whole. */
      baseline: number
      ack: number
      ride: number
      /** One per entity new or changed since `baseline` (`replicated.ts`). */
      updates: Update[]
      /** Ids that were in the baseline and are not any more. */
      removed: number[]
      drive: DriveState | null
      /** The local player's own health; 0 is dead. */
      hp: number
      /** How far a reload or a bandage has got, 0..255, or 0 when neither is going on. */
      progress: number
      /** The local player's hotbar. */
      inventory: ItemSlot[]
      shots: ShotEvent[]
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
  /** What players chose: everyone's for a joiner, then each new player's. `color` indexes `PLAYER_COLORS`. */
  | { type: 'profiles'; players: { id: number; name: string; color: number; team: number; bot: boolean }[] }
  /** The scenario says something (`NOTICE_*`): a message, the objective (empty clears it), or that it was won or lost. */
  | { type: 'notice'; kind: number; text: string }
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


function decodeShot(raw: unknown): ShotEvent | null {
  if (!Array.isArray(raw) || raw.length !== 5 || !ints(raw)) return null
  const [shooter, weapon, x, y, z] = raw as number[]
  return { shooter: shooter!, weapon: weapon!, x: x! / CM, y: y! / CM, z: z! / CM }
}


function decodeBullet(raw: unknown): BulletSpawn | null {
  if (!Array.isArray(raw) || raw.length !== 10 || !ints(raw)) return null
  const [id, shooter, item, x, y, z, vx, vy, vz, tick] = raw as number[]
  return { id: id!, shooter: shooter!, item: item!, x: x! / CM, y: y! / CM, z: z! / CM, vx: vx! / CM, vy: vy! / CM, vz: vz! / CM, tick: tick! }
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


function decodeDrive(raw: unknown): DriveState | null | undefined {
  if (raw === null) return null
  if (!Array.isArray(raw) || raw.length !== 8 || !ints(raw)) return undefined
  const [vx, vy, vz, wx, wy, wz, steer, spin] = raw as number[]
  return { vx: vx! / CM, vy: vy! / CM, vz: vz! / CM, wx: wx! / MRAD, wy: wy! / MRAD, wz: wz! / MRAD, steer: steer! / MRAD, spin: spin! / MRAD }
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
      if (raw.length !== 11 || !isInt(raw[1]) || !isInt(raw[2]) || !isInt(raw[3]) || !isInt(raw[4])) return null
      const updates = decodeUpdates(raw[5])
      const removed = raw[6]
      if (!updates || !Array.isArray(removed) || !ints(removed)) return null
      const drive = decodeDrive(raw[7])
      const me = decodeSelf(raw[8])
      if (drive === undefined || !me || !Array.isArray(raw[9]) || !Array.isArray(raw[10])) return null
      const shots: ShotEvent[] = []
      for (const item of raw[9] as unknown[]) {
        const shot = decodeShot(item)
        if (!shot) return null
        shots.push(shot)
      }
      const bullets: BulletSpawn[] = []
      for (const item of raw[10] as unknown[]) {
        const bullet = decodeBullet(item)
        if (!bullet) return null
        bullets.push(bullet)
      }
      return {
        type: 'snapshot', tick: raw[1], baseline: raw[2], ack: raw[3], ride: raw[4], updates, removed: removed as number[], drive,
        hp: me.hp, progress: me.progress, inventory: me.inventory, shots, bullets,
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
      const players: { id: number; name: string; color: number; team: number; bot: boolean }[] = []
      for (const item of raw[1] as unknown[]) {
        if (!Array.isArray(item) || item.length !== 5 || !isInt(item[0]) || typeof item[1] !== 'string' || !isInt(item[2]) || !isInt(item[3]) || !isInt(item[4])) return null
        players.push({ id: item[0], name: item[1], color: item[2], team: item[3], bot: item[4] === 1 })
      }
      return { type: 'profiles', players }
    }
    case S_NOTICE:
      if (raw.length !== 3 || !isInt(raw[1]) || typeof raw[2] !== 'string') return null
      return { type: 'notice', kind: raw[1], text: raw[2] }
    case S_DESTROYED: {
      if (raw.length !== 2 || !Array.isArray(raw[1])) return null
      const objects: DestroyedObject[] = []
      for (const item of raw[1] as unknown[]) {
        if (!Array.isArray(item) || item.length !== 6 || !ints(item)) return null
        objects.push({ kind: item[0] as number, col: item[1] as number, row: item[2] as number, index: item[3] as number, yaw: (item[4] as number) / MRAD, force: (item[5] as number) / 255 })
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
