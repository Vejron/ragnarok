import { decode, encode } from '@msgpack/msgpack'
import { MAX_PACKET, PROTOCOL_VERSION } from '@/sim/consts'
import { decodeMessage, writeClientInput, writePing, writeRtc, type InputFrame, type ServerMessage } from './messages.generated'

/**
 * Wire format: MessagePack arrays with a leading tag, fixed-point integers, no field names. The
 * messages and their decoders are generated from `crates/shared/src/protocol.rs`
 * (`messages.generated.ts`); this is their framing. NETWORKING.md documents every message.
 */

export type {
  BulletSpawn, Built, CarvedBuilding, DamagedBuilding, DestroyedObject, InputFrame, ProfileEntry, Score, ServerMessage, ShotEvent,
} from './messages.generated'

/** `Notice` kinds: what a world's scenario is telling everyone. */
export const NOTICE_SAY = 0
export const NOTICE_OBJECTIVE = 1
export const NOTICE_WON = 2
export const NOTICE_LOST = 3

/** `Destroyed` entry kinds: what part of the static world fell. */
export const OBJ_TREE = 0
export const OBJ_BUILDING = 1

/** Entity kinds, generated from `consts.rs`. A vehicle kind's definition is `vehicleDef`. */
export {
  KIND_DRONE, KIND_HELI, KIND_HEN, KIND_JEEP, KIND_LOITER, KIND_MORTAR, KIND_PLANE, KIND_ROADSTER, KIND_ROOSTER, KIND_SPORTS, KIND_STICKMAN, KIND_STRV, KIND_TANK, KIND_WALKER,
} from '@/sim/consts'
export { isDrone } from '@/sim/vehicles'

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
export const WEAPON_RUBBLE = 23
export const WEAPON_LAUNCHER = 24

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
  return encode(writeClientInput({ version: PROTOCOL_VERSION, baseline, frames: frames as InputFrame[] }))
}

/** `Rtc` kinds. */
export const RTC_OFFER = 0
export const RTC_ANSWER = 1
export const RTC_ICE = 2

/** Voice signaling for player `peer`. Goes down the signaling stream, not as a datagram. */
export function encodeRtc(peer: number, kind: number, json: string): Uint8Array {
  return encode(writeRtc({ peer, kind, json }))
}

export function encodePing(nonce: number): Uint8Array {
  return encode(writePing({ nonce }))
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
  return (Array.isArray(raw) && decodeMessage(raw)) || null
}
