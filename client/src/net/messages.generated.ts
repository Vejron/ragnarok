// GENERATED from crates/shared/src/protocol.rs by its tests. Do not edit: change the Rust and
// run `UPDATE_PARITY=1 cargo test -p shared`. What each message means is documented there.

import type { Update } from './replicated'
import * as w from './wire'

export const C_INPUT = 0
export const C_PING = 1
export const C_RTC = 2
export const S_WELCOME = 0
export const S_SNAPSHOT = 1
export const S_PONG = 2
export const S_LEAVE = 3
export const S_KILL = 4
export const S_SCORES = 5
export const S_DESTROYED = 6
export const S_DAMAGED = 7
export const S_HIT = 8
export const S_RTC = 9
export const S_CARVED = 10
export const S_PROFILES = 11
export const S_NOTICE = 12
export const S_BUILT = 13
export const S_CRATERED = 14

export interface BulletSpawn {
  id: number
  shooter: number
  item: number
  x: number
  y: number
  z: number
  vx: number
  vy: number
  vz: number
  tick: number
}

export interface InputFrame {
  seq: number
  strafe: number
  forward: number
  yaw: number
  buttons: number
  pitch: number
  view: number
  slot: number
}

export interface ClientInput {
  version: number
  baseline: number
  frames: InputFrame[]
}

export interface Ping {
  nonce: number
}

export interface Welcome {
  type: 'welcome'
  version: number
  id: number
  tickRate: number
  snapshotEvery: number
  worldHash: number
}

export interface ShotEvent {
  shooter: number
  weapon: number
  x: number
  y: number
  z: number
}

export interface Snapshot {
  type: 'snapshot'
  tick: number
  baseline: number
  ack: number
  ride: number
  updates: Update[]
  removed: number[]
  shots: ShotEvent[]
  bullets: BulletSpawn[]
}

export interface Pong {
  type: 'pong'
  nonce: number
}

export interface Leave {
  type: 'leave'
  id: number
}

export interface Kill {
  type: 'kill'
  killer: number
  victim: number
  weapon: number
  killerKills: number
  victimDeaths: number
  style: number
  yaw: number
  power: number
}

export interface Score {
  id: number
  kills: number
  deaths: number
}

export interface Scores {
  type: 'scores'
  scores: Score[]
}

export interface ProfileEntry {
  id: number
  name: string
  color: number
  team: number
  bot: boolean
}

export interface Profiles {
  type: 'profiles'
  players: ProfileEntry[]
}

export interface Notice {
  type: 'notice'
  kind: number
  text: string
}

export interface DestroyedObject {
  kind: number
  col: number
  row: number
  index: number
  yaw: number
  force: number
}

export interface Destroyed {
  type: 'destroyed'
  objects: DestroyedObject[]
}

export interface DamagedBuilding {
  col: number
  row: number
  index: number
  stage: number
  mark: [number, number, number] | null
}

export interface Damaged {
  type: 'damaged'
  buildings: DamagedBuilding[]
}

export interface CarvedBuilding {
  col: number
  row: number
  index: number
  point: [number, number, number]
  blasted: number[]
  fell: number[]
}

export interface Carved {
  type: 'carved'
  live: boolean
  buildings: CarvedBuilding[]
}

export interface BuiltCube {
  x: number
  y: number
  z: number
  color: number
  material: number
}

export interface CubePos {
  x: number
  y: number
  z: number
}

export interface Built {
  type: 'built'
  live: boolean
  placed: BuiltCube[]
  removed: CubePos[]
  fell: CubePos[]
}

export interface CraterEntry {
  x: number
  z: number
  radius: number
  depth: number
}

export interface Cratered {
  type: 'cratered'
  live: boolean
  craters: CraterEntry[]
}

export interface Hit {
  type: 'hit'
  role: number
  kind: number
  amount: number
  weapon: number
  bearing: number | null
  x: number
  y: number
  z: number
  target: number
}

export interface Rtc {
  type: 'rtc'
  peer: number
  kind: number
  json: string
}

export type ServerMessage = Welcome | Snapshot | Pong | Leave | Kill | Scores | Profiles | Notice | Destroyed | Damaged | Carved | Built | Cratered | Hit | Rtc

function decodeBulletSpawn(r: unknown): BulletSpawn | undefined {
  if (!Array.isArray(r) || r.length !== 10) return undefined
  const id = w.int(r[0])
  const shooter = w.int(r[1])
  const item = w.int(r[2])
  const x = w.int(r[3])
  const y = w.int(r[4])
  const z = w.int(r[5])
  const vx = w.int(r[6])
  const vy = w.int(r[7])
  const vz = w.int(r[8])
  const tick = w.int(r[9])
  if (id === undefined || shooter === undefined || item === undefined || x === undefined || y === undefined || z === undefined || vx === undefined || vy === undefined || vz === undefined || tick === undefined) return undefined
  return { id, shooter, item, x: x / 100, y: y / 100, z: z / 100, vx: vx / 100, vy: vy / 100, vz: vz / 100, tick }
}

function decodeShotEvent(r: unknown): ShotEvent | undefined {
  if (!Array.isArray(r) || r.length !== 5) return undefined
  const shooter = w.int(r[0])
  const weapon = w.int(r[1])
  const x = w.int(r[2])
  const y = w.int(r[3])
  const z = w.int(r[4])
  if (shooter === undefined || weapon === undefined || x === undefined || y === undefined || z === undefined) return undefined
  return { shooter, weapon, x: x / 100, y: y / 100, z: z / 100 }
}

function decodeScore(r: unknown): Score | undefined {
  if (!Array.isArray(r) || r.length !== 3) return undefined
  const id = w.int(r[0])
  const kills = w.int(r[1])
  const deaths = w.int(r[2])
  if (id === undefined || kills === undefined || deaths === undefined) return undefined
  return { id, kills, deaths }
}

function decodeProfileEntry(r: unknown): ProfileEntry | undefined {
  if (!Array.isArray(r) || r.length !== 5) return undefined
  const id = w.int(r[0])
  const name = w.text(r[1])
  const color = w.int(r[2])
  const team = w.int(r[3])
  const bot = w.bool(r[4])
  if (id === undefined || name === undefined || color === undefined || team === undefined || bot === undefined) return undefined
  return { id, name, color, team, bot }
}

function decodeDestroyedObject(r: unknown): DestroyedObject | undefined {
  if (!Array.isArray(r) || r.length !== 6) return undefined
  const kind = w.int(r[0])
  const col = w.int(r[1])
  const row = w.int(r[2])
  const index = w.int(r[3])
  const yaw = w.int(r[4])
  const force = w.int(r[5])
  if (kind === undefined || col === undefined || row === undefined || index === undefined || yaw === undefined || force === undefined) return undefined
  return { kind, col, row, index, yaw: yaw / 1000, force: force / 255 }
}

function decodeDamagedBuilding(r: unknown): DamagedBuilding | undefined {
  if (!Array.isArray(r) || r.length !== 5) return undefined
  const col = w.int(r[0])
  const row = w.int(r[1])
  const index = w.int(r[2])
  const stage = w.int(r[3])
  const mark = w.optional(r[4], (v) => w.ints(v, 3) as [number, number, number] | undefined)
  if (col === undefined || row === undefined || index === undefined || stage === undefined || mark === undefined) return undefined
  return { col, row, index, stage, mark: mark === null ? null : mark.map((v) => v / 100) as [number, number, number] }
}

function decodeCarvedBuilding(r: unknown): CarvedBuilding | undefined {
  if (!Array.isArray(r) || r.length !== 6) return undefined
  const col = w.int(r[0])
  const row = w.int(r[1])
  const index = w.int(r[2])
  const point = w.ints(r[3], 3) as [number, number, number] | undefined
  const blasted = w.ints(r[4])
  const fell = w.ints(r[5])
  if (col === undefined || row === undefined || index === undefined || point === undefined || blasted === undefined || fell === undefined) return undefined
  return { col, row, index, point: point.map((v) => v / 100) as [number, number, number], blasted, fell }
}

function decodeBuiltCube(r: unknown): BuiltCube | undefined {
  if (!Array.isArray(r) || r.length !== 5) return undefined
  const x = w.int(r[0])
  const y = w.int(r[1])
  const z = w.int(r[2])
  const color = w.int(r[3])
  const material = w.int(r[4])
  if (x === undefined || y === undefined || z === undefined || color === undefined || material === undefined) return undefined
  return { x, y, z, color, material }
}

function decodeCubePos(r: unknown): CubePos | undefined {
  if (!Array.isArray(r) || r.length !== 3) return undefined
  const x = w.int(r[0])
  const y = w.int(r[1])
  const z = w.int(r[2])
  if (x === undefined || y === undefined || z === undefined) return undefined
  return { x, y, z }
}

function decodeCraterEntry(r: unknown): CraterEntry | undefined {
  if (!Array.isArray(r) || r.length !== 4) return undefined
  const x = w.int(r[0])
  const z = w.int(r[1])
  const radius = w.int(r[2])
  const depth = w.int(r[3])
  if (x === undefined || z === undefined || radius === undefined || depth === undefined) return undefined
  return { x, z, radius, depth }
}

function decodeWelcome(r: readonly unknown[]): Welcome | undefined {
  if (r.length !== 6) return undefined
  const version = w.int(r[1])
  const id = w.int(r[2])
  const tickRate = w.int(r[3])
  const snapshotEvery = w.int(r[4])
  const worldHash = w.int(r[5])
  if (version === undefined || id === undefined || tickRate === undefined || snapshotEvery === undefined || worldHash === undefined) return undefined
  return { type: 'welcome', version, id, tickRate, snapshotEvery, worldHash }
}

function decodeSnapshot(r: readonly unknown[]): Snapshot | undefined {
  if (r.length !== 9) return undefined
  const tick = w.int(r[1])
  const baseline = w.int(r[2])
  const ack = w.int(r[3])
  const ride = w.int(r[4])
  const updates = w.updates(r[5])
  const removed = w.ints(r[6])
  const shots = w.list(r[7], decodeShotEvent)
  const bullets = w.list(r[8], decodeBulletSpawn)
  if (tick === undefined || baseline === undefined || ack === undefined || ride === undefined || updates === undefined || removed === undefined || shots === undefined || bullets === undefined) return undefined
  return { type: 'snapshot', tick, baseline, ack, ride, updates, removed, shots, bullets }
}

function decodePong(r: readonly unknown[]): Pong | undefined {
  if (r.length !== 2) return undefined
  const nonce = w.int(r[1])
  if (nonce === undefined) return undefined
  return { type: 'pong', nonce }
}

function decodeLeave(r: readonly unknown[]): Leave | undefined {
  if (r.length !== 2) return undefined
  const id = w.int(r[1])
  if (id === undefined) return undefined
  return { type: 'leave', id }
}

function decodeKill(r: readonly unknown[]): Kill | undefined {
  if (r.length !== 9) return undefined
  const killer = w.int(r[1])
  const victim = w.int(r[2])
  const weapon = w.int(r[3])
  const killerKills = w.int(r[4])
  const victimDeaths = w.int(r[5])
  const style = w.int(r[6])
  const yaw = w.int(r[7])
  const power = w.int(r[8])
  if (killer === undefined || victim === undefined || weapon === undefined || killerKills === undefined || victimDeaths === undefined || style === undefined || yaw === undefined || power === undefined) return undefined
  return { type: 'kill', killer, victim, weapon, killerKills, victimDeaths, style, yaw: yaw / 1000, power: power / 5 }
}

function decodeScores(r: readonly unknown[]): Scores | undefined {
  if (r.length !== 2) return undefined
  const scores = w.list(r[1], decodeScore)
  if (scores === undefined) return undefined
  return { type: 'scores', scores }
}

function decodeProfiles(r: readonly unknown[]): Profiles | undefined {
  if (r.length !== 2) return undefined
  const players = w.list(r[1], decodeProfileEntry)
  if (players === undefined) return undefined
  return { type: 'profiles', players }
}

function decodeNotice(r: readonly unknown[]): Notice | undefined {
  if (r.length !== 3) return undefined
  const kind = w.int(r[1])
  const text = w.text(r[2])
  if (kind === undefined || text === undefined) return undefined
  return { type: 'notice', kind, text }
}

function decodeDestroyed(r: readonly unknown[]): Destroyed | undefined {
  if (r.length !== 2) return undefined
  const objects = w.list(r[1], decodeDestroyedObject)
  if (objects === undefined) return undefined
  return { type: 'destroyed', objects }
}

function decodeDamaged(r: readonly unknown[]): Damaged | undefined {
  if (r.length !== 2) return undefined
  const buildings = w.list(r[1], decodeDamagedBuilding)
  if (buildings === undefined) return undefined
  return { type: 'damaged', buildings }
}

function decodeCarved(r: readonly unknown[]): Carved | undefined {
  if (r.length !== 3) return undefined
  const live = w.bool(r[1])
  const buildings = w.list(r[2], decodeCarvedBuilding)
  if (live === undefined || buildings === undefined) return undefined
  return { type: 'carved', live, buildings }
}

function decodeBuilt(r: readonly unknown[]): Built | undefined {
  if (r.length !== 5) return undefined
  const live = w.bool(r[1])
  const placed = w.list(r[2], decodeBuiltCube)
  const removed = w.list(r[3], decodeCubePos)
  const fell = w.list(r[4], decodeCubePos)
  if (live === undefined || placed === undefined || removed === undefined || fell === undefined) return undefined
  return { type: 'built', live, placed, removed, fell }
}

function decodeCratered(r: readonly unknown[]): Cratered | undefined {
  if (r.length !== 3) return undefined
  const live = w.bool(r[1])
  const craters = w.list(r[2], decodeCraterEntry)
  if (live === undefined || craters === undefined) return undefined
  return { type: 'cratered', live, craters }
}

function decodeHit(r: readonly unknown[]): Hit | undefined {
  if (r.length !== 10) return undefined
  const role = w.int(r[1])
  const kind = w.int(r[2])
  const amount = w.int(r[3])
  const weapon = w.int(r[4])
  const bearing = w.optional(r[5], (v) => w.int(v))
  const x = w.int(r[6])
  const y = w.int(r[7])
  const z = w.int(r[8])
  const target = w.int(r[9])
  if (role === undefined || kind === undefined || amount === undefined || weapon === undefined || bearing === undefined || x === undefined || y === undefined || z === undefined || target === undefined) return undefined
  return { type: 'hit', role, kind, amount, weapon, bearing: bearing === null ? null : bearing / 1000, x: x / 100, y: y / 100, z: z / 100, target }
}

function decodeRtc(r: readonly unknown[]): Rtc | undefined {
  if (r.length !== 4) return undefined
  const peer = w.int(r[1])
  const kind = w.int(r[2])
  const json = w.text(r[3])
  if (peer === undefined || kind === undefined || json === undefined) return undefined
  return { type: 'rtc', peer, kind, json }
}

/** A server message, from its MessagePack array: undefined if it is malformed or not one. */
export function decodeMessage(r: readonly unknown[]): ServerMessage | undefined {
  switch (r[0]) {
    case S_WELCOME:
      return decodeWelcome(r)
    case S_SNAPSHOT:
      return decodeSnapshot(r)
    case S_PONG:
      return decodePong(r)
    case S_LEAVE:
      return decodeLeave(r)
    case S_KILL:
      return decodeKill(r)
    case S_SCORES:
      return decodeScores(r)
    case S_PROFILES:
      return decodeProfiles(r)
    case S_NOTICE:
      return decodeNotice(r)
    case S_DESTROYED:
      return decodeDestroyed(r)
    case S_DAMAGED:
      return decodeDamaged(r)
    case S_CARVED:
      return decodeCarved(r)
    case S_BUILT:
      return decodeBuilt(r)
    case S_CRATERED:
      return decodeCratered(r)
    case S_HIT:
      return decodeHit(r)
    case S_RTC:
      return decodeRtc(r)
    default:
      return undefined
  }
}

export const writeInputFrame = (m: InputFrame): unknown[] => [m.seq, m.strafe, m.forward, m.yaw, m.buttons, m.pitch, m.view, m.slot]

export const writeClientInput = (m: ClientInput): unknown[] => [C_INPUT, m.version, m.baseline, m.frames.map(writeInputFrame)]

export const writePing = (m: Ping): unknown[] => [C_PING, m.nonce]

export const writeRtc = (m: Omit<Rtc, 'type'>): unknown[] => [C_RTC, m.peer, m.kind, m.json]
