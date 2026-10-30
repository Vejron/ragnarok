import { MAX_NAME_CHARS, MAX_SPAWNS, PLAYER_COLOR_COUNT } from '@/sim/consts'
import { HOTBAR_SLOTS, ITEM_NONE, LOADOUT_ITEMS } from '@/sim/items'

/**
 * Who the player is: the settings page (`/settings.html`) edits it, the game sends it when it
 * connects (`profileQuery`), and the server tells everyone what everyone chose (`profiles` message).
 */

/** Bright, goofy outfit colours. A player picks one by index; the first 8 are the old by-id palette. */
export const PLAYER_COLORS: readonly { hex: number; name: string }[] = [
  { hex: 0xff5a5f, name: 'Coral' },
  { hex: 0x2ec4b6, name: 'Teal' },
  { hex: 0xffbe0b, name: 'Sun' },
  { hex: 0x8338ec, name: 'Violet' },
  { hex: 0x3a86ff, name: 'Blue' },
  { hex: 0xfb5607, name: 'Orange' },
  { hex: 0x06d6a0, name: 'Mint' },
  { hex: 0xff006e, name: 'Magenta' },
  { hex: 0x8ac926, name: 'Lime' },
  { hex: 0xff99c8, name: 'Pink' },
  { hex: 0x7f5539, name: 'Bark' },
  { hex: 0xf1faee, name: 'Snow' },
]
if (PLAYER_COLORS.length !== PLAYER_COLOR_COUNT) throw new Error('PLAYER_COLORS must hold PLAYER_COLOR_COUNT colours')

export const cssColor = (hex: number) => `#${hex.toString(16).padStart(6, '0')}`

/** What the server said each player in the room chose, and which side they are on. */
const known = new Map<number, { name: string; color: number; team: number; bot: boolean }>()

export function setPlayer(id: number, name: string, color: number, team = 0, bot = false) {
  known.set(id, { name, color, team, bot })
}

export function forgetPlayer(id: number) {
  known.delete(id)
}

export function forgetAllPlayers() {
  known.clear()
}

/** Entity `id`'s outfit colour: the one they chose, or one picked by id until we know. */
export const colorFor = (id: number) => PLAYER_COLORS[(known.get(id)?.color ?? id) % PLAYER_COLORS.length]!.hex

/** Entity `id`'s outfit colour as an index into `PLAYER_COLORS`, as `colorFor` picks it. */
export const colorIndexFor = (id: number) => (known.get(id)?.color ?? id) % PLAYER_COLORS.length

/** Entity `id`'s name, as the server knows it. */
export const nameFor = (id: number) => known.get(id)?.name ?? `Player ${id}`

/** Entity `id`'s team: 0 for people, who are all against all, and until we know. */
export const teamFor = (id: number) => known.get(id)?.team ?? 0

/** Whether entity `id` is a bot. */
export const isBot = (id: number) => known.get(id)?.bot ?? false

/** Whether `a` and `b` are on one side: the same team, other than 0. Nobody is their own ally. */
export const allies = (a: number, b: number) => a !== b && teamFor(a) !== 0 && teamFor(a) === teamFor(b)

/** A spawn point `[x, z]`, centimetres in the world's local frame, as the server takes them. */
export type SpawnPoint = [number, number]

export interface Profile {
  version: 1
  /** Empty means the server names the player after their id. */
  name: string
  /** Index into `PLAYER_COLORS`, or null for one picked by id. */
  color: number | null
  /** Spawn points per world, keyed by world hash, in the order they are used. */
  spawns: Record<string, SpawnPoint[]>
  /** The item in each hotbar slot at the start of a life, `ITEM_NONE` for empty; each item at most once. Null is the standard loadout. */
  loadout: number[] | null
}

const STORAGE_KEY = 'ragnarok.profile'

export const emptyProfile = (): Profile => ({ version: 1, name: '', color: null, spawns: {}, loadout: null })

/** Characters that are invisible or reorder text. Mirrors `hidden` in `crates/server/src/profile.rs`. */
const HIDDEN = /[\p{Cc}​-‏‪-‮⁠-⁩﻿]/gu

/** `name` the way the server will show it: no hidden characters or runs of spaces, at most `MAX_NAME_CHARS`. */
export function sanitizeName(name: string): string {
  // As in Rust, whitespace becomes a space before the hidden characters go; JS counts U+FEFF as both.
  const words = name.replace(/\s/g, (c) => (c === '﻿' ? '' : ' ')).replace(HIDDEN, '').split(' ').filter(Boolean).join(' ')
  return [...words].slice(0, MAX_NAME_CHARS).join('').trimEnd()
}

const isPoint = (value: unknown): value is SpawnPoint =>
  Array.isArray(value) && value.length === 2 && value.every((v) => Number.isInteger(v) && Math.abs(v) < 2 ** 31)

/** Whether `value` is a loadout the server will take. Mirrors `parse_loadout` in `crates/server/src/profile.rs`. */
const isLoadout = (value: unknown): value is number[] => {
  if (!Array.isArray(value) || value.length !== HOTBAR_SLOTS) return false
  const items = value.filter((item) => item !== ITEM_NONE)
  return items.every((item) => LOADOUT_ITEMS.includes(item)) && new Set(items).size === items.length
}

/** Whatever is stored, made safe to use: anything malformed is dropped. */
export function parseProfile(raw: unknown): Profile {
  const profile = emptyProfile()
  if (!raw || typeof raw !== 'object') return profile
  const { name, color, spawns, loadout } = raw as Record<string, unknown>
  if (typeof name === 'string') profile.name = sanitizeName(name)
  if (Number.isInteger(color) && (color as number) >= 0 && (color as number) < PLAYER_COLOR_COUNT) profile.color = color as number
  if (spawns && typeof spawns === 'object') {
    for (const [world, points] of Object.entries(spawns)) {
      if (Array.isArray(points)) profile.spawns[world] = points.filter(isPoint).slice(0, MAX_SPAWNS)
    }
  }
  if (isLoadout(loadout)) profile.loadout = loadout
  return profile
}

export function loadProfile(): Profile {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    return parseProfile(stored ? JSON.parse(stored) : null)
  } catch {
    return emptyProfile()
  }
}

export function saveProfile(profile: Profile) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(profile))
  } catch {}
}

/** The key spawn points are kept under: the world they were placed in. */
export const worldKey = (worldHash: number) => (worldHash >>> 0).toString(16)

/** The query string the game connects with, for the world with `worldHash`. */
export function profileQuery(profile: Profile, worldHash: number): string {
  const params = new URLSearchParams()
  const name = sanitizeName(profile.name)
  if (name) params.set('name', name)
  if (profile.color !== null) params.set('color', String(profile.color))
  for (const [x, z] of (profile.spawns[worldKey(worldHash)] ?? []).slice(0, MAX_SPAWNS)) params.append('spawn', `${x},${z}`)
  if (profile.loadout) params.set('loadout', profile.loadout.join(','))
  return params.toString()
}
