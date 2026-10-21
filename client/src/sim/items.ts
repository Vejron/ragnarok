/** Mirrors `crates/shared/src/items.rs`. Keep the two in step: `parity.spec.ts` checks this file against `fixtures/parity/gameplay.json`, which the Rust tests generate. */

import { BUTTON_ALT, TICK_RATE } from './consts'

export const HOTBAR_SLOTS = 5

export const ITEM_NONE = 0
export const ITEM_REVOLVER = 1
export const ITEM_AK5 = 2
export const ITEM_GRENADE = 3
export const ITEM_BANDAGE = 4
export const ITEM_SNIPER = 5
/** Dropped by a rooster; heals to full. */
export const ITEM_GOLDEN_EGG = 6
export const ITEM_SHOTGUN = 7
/** Launched from the hand and flown in its owner's place until it blows up. */
export const ITEM_DRONE = 8
/** A loitering drone: launched high above and behind its owner, and flown into a target. */
export const ITEM_LOITER = 9
/** Fires a flare that hangs under a parachute, lighting the ground below. */
export const ITEM_FLARE = 10

export type ItemKind = 'gun' | 'grenade' | 'heal' | 'drone' | 'flare'

export interface ItemDef {
  name: string
  kind: ItemKind
  /** Held fire keeps shooting; otherwise each shot needs a fresh press. */
  auto: boolean
  /** Hitscan damage, or health restored by a heal. */
  damage: number
  cooldownTicks: number
  /** Rounds in a full magazine; zero for items that do not reload. */
  mag: number
  reloadTicks: number
  range: number
  /** Walking speed multiplier while aim is held with this gun in hand. */
  aimSpeed: number
  /** Can be scoped (aim held): first-person zoom, and the shot goes true along the look direction. */
  scope: boolean
  stack: number
  /** What the player holds it as: picks the pose and the model. */
  stance: 'rifle' | 'pistol' | 'hold'
  /** The `WEAPON_*` id its shot and kill events carry (`codec.ts`; not imported here, which would be a cycle). */
  weapon: number
  /** Fires a round that flies and hits on arrival, rather than a hitscan. */
  projectile: boolean
  /** Which gunshot sample a shot of it plays. */
  sound: 'rifle' | 'revolver' | 'sniper' | 'shotgun'
  /** Hitscan rays one trigger pull fires; each does `damage`. */
  pellets: number
  /** Full damage up to `falloffStart` metres, then down to `falloffMin` (a fraction) at `falloffEnd`. Mirrors `Falloff`. */
  falloffStart: number
  falloffEnd: number
  falloffMin: number
}

const DEFS: Record<number, ItemDef> = {
  [ITEM_REVOLVER]: { name: 'Revolver', kind: 'gun', auto: false, damage: 35, cooldownTicks: 30, mag: 6, reloadTicks: 2 * TICK_RATE, range: 150, aimSpeed: 0.8, scope: false, stack: 1, stance: 'pistol', weapon: 4, projectile: true, sound: 'revolver', pellets: 1, falloffStart: 30, falloffEnd: 150, falloffMin: 0.6 },
  [ITEM_AK5]: { name: 'AK5', kind: 'gun', auto: true, damage: 12, cooldownTicks: 6, mag: 30, reloadTicks: (5 * TICK_RATE) / 2, range: 200, aimSpeed: 0.6, scope: false, stack: 1, stance: 'rifle', weapon: 0, projectile: true, sound: 'rifle', pellets: 1, falloffStart: 25, falloffEnd: 200, falloffMin: 0.5 },
  [ITEM_SNIPER]: { name: 'Sniper', kind: 'gun', auto: false, damage: 90, cooldownTicks: 2 * TICK_RATE, mag: 5, reloadTicks: 3 * TICK_RATE, range: 400, aimSpeed: 0.4, scope: true, stack: 1, stance: 'rifle', weapon: 7, projectile: true, sound: 'sniper', pellets: 1, falloffStart: 0, falloffEnd: 0, falloffMin: 1 },
  [ITEM_SHOTGUN]: { name: 'Shotgun', kind: 'gun', auto: false, damage: 12, cooldownTicks: TICK_RATE, mag: 5, reloadTicks: 3 * TICK_RATE, range: 40, aimSpeed: 0.7, scope: false, stack: 1, stance: 'rifle', weapon: 13, projectile: false, sound: 'shotgun', pellets: 10, falloffStart: 8, falloffEnd: 40, falloffMin: 0.25 },
  [ITEM_GRENADE]: { name: 'Grenade', kind: 'grenade', auto: false, damage: 0, cooldownTicks: TICK_RATE, mag: 0, reloadTicks: 0, range: 0, aimSpeed: 1, scope: false, stack: 3, stance: 'hold', weapon: 5, projectile: false, sound: 'rifle', pellets: 1, falloffStart: 0, falloffEnd: 0, falloffMin: 1 },
  [ITEM_BANDAGE]: { name: 'Bandage', kind: 'heal', auto: false, damage: 40, cooldownTicks: 0, mag: 0, reloadTicks: 0, range: 0, aimSpeed: 1, scope: false, stack: 5, stance: 'hold', weapon: 0, projectile: false, sound: 'rifle', pellets: 1, falloffStart: 0, falloffEnd: 0, falloffMin: 1 },
  [ITEM_DRONE]: { name: 'FPV drone', kind: 'drone', auto: false, damage: 0, cooldownTicks: TICK_RATE, mag: 0, reloadTicks: 0, range: 0, aimSpeed: 1, scope: false, stack: 1, stance: 'hold', weapon: 19, projectile: false, sound: 'rifle', pellets: 1, falloffStart: 0, falloffEnd: 0, falloffMin: 1 },
  [ITEM_LOITER]: { name: 'Loitering drone', kind: 'drone', auto: false, damage: 0, cooldownTicks: TICK_RATE, mag: 0, reloadTicks: 0, range: 0, aimSpeed: 1, scope: false, stack: 1, stance: 'hold', weapon: 21, projectile: false, sound: 'rifle', pellets: 1, falloffStart: 0, falloffEnd: 0, falloffMin: 1 },
  [ITEM_FLARE]: { name: 'Flare gun', kind: 'flare', auto: false, damage: 0, cooldownTicks: TICK_RATE, mag: 0, reloadTicks: 0, range: 0, aimSpeed: 1, scope: false, stack: 3, stance: 'pistol', weapon: 22, projectile: false, sound: 'revolver', pellets: 1, falloffStart: 0, falloffEnd: 0, falloffMin: 1 },
  [ITEM_GOLDEN_EGG]: { name: 'Golden egg', kind: 'heal', auto: false, damage: 255, cooldownTicks: 0, mag: 0, reloadTicks: 0, range: 0, aimSpeed: 1, scope: false, stack: 1, stance: 'hold', weapon: 0, projectile: false, sound: 'rifle', pellets: 1, falloffStart: 0, falloffEnd: 0, falloffMin: 1 },
}

/** The definition of `item`; undefined for an empty slot or an unknown id. */
export const itemDef = (item: number): ItemDef | undefined => DEFS[item]

/** The hitscan gun whose shots carry `weapon` in a shot event, or undefined (a tank shell, a rocket, a sniper round in flight...). */
export const hitscanGunFor = (weapon: number): ItemDef | undefined =>
  Object.values(DEFS).find((d) => d.kind === 'gun' && !d.projectile && d.weapon === weapon)

/** The damage of one round of `def` after it flew `distance` metres. Mirrors `Falloff::apply`: never under 1. */
export const falloffDamage = (def: ItemDef, distance: number): number => {
  if (def.damage === 0) return 0
  const span = def.falloffEnd - def.falloffStart
  const along = distance <= def.falloffStart ? 0 : span > 0 ? Math.min(1, (distance - def.falloffStart) / span) : 1
  return Math.max(1, Math.round(def.damage * (1 - (1 - def.falloffMin) * along)))
}

/** Walking speed multiplier for `item` in hand with `buttons`: a gun slows its holder while aim is held. Mirrors `aim_speed_scale`. */
export const aimSpeedScale = (item: number, buttons: number): number => {
  const d = DEFS[item]
  return d?.kind === 'gun' && (buttons & BUTTON_ALT) !== 0 ? d.aimSpeed : 1
}

/** Ticks a bandage takes to apply. */
export const BANDAGE_TICKS = 2 * TICK_RATE
/** Grenades. Mirrors the grenade block of items.rs. */
export const GRENADE_BLAST_RADIUS = 5.0
export const GRENADE_FUSE_TICKS = 3 * TICK_RATE
/** Throwing. Mirrors `GRENADE_*` in items.rs. */
export const GRENADE_SPEED = 16.0
/** A throw is aimed this far above the look direction, radians, and starts this far in front of the eye. */
export const GRENADE_LOFT = 0.15
export const GRENADE_MUZZLE = 0.6
export const GRENADE_RADIUS = 0.1
/** How much of its speed a grenade keeps when it bounces. */
export const GRENADE_BOUNCE = 0.4
/** FPV drones: how long one flies, how far from its pilot, and what its charge does. Mirrors `DRONE_*` in items.rs. */
export const DRONE_ENERGY_TICKS = 25 * TICK_RATE
export const DRONE_RANGE = 250.0
export const DRONE_BLAST_RADIUS = 4.0
/** Loitering drones, the same. Mirrors `LOITER_*` in items.rs. */
export const LOITER_ENERGY_TICKS = 60 * TICK_RATE
export const LOITER_RANGE = 600.0
export const LOITER_BLAST_RADIUS = 12.0
/** Flares: how long one burns and how fast it sinks under its parachute. Mirrors `FLARE_*` in items.rs. */
export const FLARE_BURN_TICKS = 45 * TICK_RATE
export const FLARE_SINK = 2.5
/** The most pitch the server will accept, radians. */
export const MAX_AIM_PITCH = 1.5
/** A player in this many metres of an item can take it with use. */
export const PICKUP_RADIUS = 2.0

/** One hotbar slot as the server reports it. */
export interface ItemSlot {
  item: number
  count: number
  /** Rounds left in the magazine, for a gun. */
  ammo: number
}

export const emptyHotbar = (): ItemSlot[] => Array.from({ length: HOTBAR_SLOTS }, () => ({ item: ITEM_NONE, count: 0, ammo: 0 }))
