import { decode, encode } from '@msgpack/msgpack'
import { MAX_PACKET, PROTOCOL_VERSION } from '@/sim/consts'

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

export const KIND_STICKMAN = 0
export const FLAG_GROUNDED = 1 << 0

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
}

export type ServerMessage =
  | { type: 'welcome'; version: number; id: number; tickRate: number; snapshotEvery: number; worldHash: number }
  | { type: 'snapshot'; tick: number; ack: number; entities: EntityState[] }
  | { type: 'pong'; nonce: number }
  | { type: 'leave'; id: number }

const CM = 100
const MRAD = 1000

export const toCm = (metres: number) => (Number.isFinite(metres) ? Math.round(metres * CM) : 0)
export const wrapAngle = (radians: number) => radians - Math.PI * 2 * Math.floor((radians + Math.PI) / (Math.PI * 2))
export const toMrad = (radians: number) => (Number.isFinite(radians) ? Math.round(wrapAngle(radians) * MRAD) : 0)
export const fromMrad = (mrad: number) => mrad / MRAD
export const toAxis = (value: number) => (Number.isFinite(value) ? Math.round(Math.max(-1, Math.min(1, value)) * 127) : 0)
export const fromAxis = (value: number) => Math.max(-1, Math.min(1, value / 127))

export function encodeInput(frames: readonly InputFrame[]): Uint8Array {
  return encode([C_INPUT, PROTOCOL_VERSION, frames.map((f) => [f.seq, f.strafe, f.forward, f.yaw, f.buttons])])
}

export function encodePing(nonce: number): Uint8Array {
  return encode([C_PING, nonce])
}

const isInt = (value: unknown): value is number => typeof value === 'number' && Number.isInteger(value)
const ints = (values: unknown[]) => values.every(isInt)

function decodeEntity(raw: unknown): EntityState | null {
  if (!Array.isArray(raw) || raw.length !== 8 || !ints(raw)) return null
  const [id, kind, x, y, z, yaw, vy, flags] = raw as number[]
  return {
    id: id!, kind: kind!,
    x: x! / CM, y: y! / CM, z: z! / CM,
    yaw: yaw! / MRAD,
    vy: vy! / CM,
    grounded: (flags! & FLAG_GROUNDED) !== 0,
  }
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
      if (raw.length !== 4 || !isInt(raw[1]) || !isInt(raw[2]) || !Array.isArray(raw[3])) return null
      const entities: EntityState[] = []
      for (const item of raw[3] as unknown[]) {
        const entity = decodeEntity(item)
        if (!entity) return null
        entities.push(entity)
      }
      return { type: 'snapshot', tick: raw[1], ack: raw[2], entities }
    }
    case S_PONG:
      return raw.length === 2 && isInt(raw[1]) ? { type: 'pong', nonce: raw[1] } : null
    case S_LEAVE:
      return raw.length === 2 && isInt(raw[1]) ? { type: 'leave', id: raw[1] } : null
    default:
      return null
  }
}
