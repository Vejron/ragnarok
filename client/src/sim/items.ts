/** Mirrors `crates/shared/src/items.rs`. Keep the two in step. */

import { TICK_RATE } from './consts'

export const HOTBAR_SLOTS = 5

export const ITEM_NONE = 0
export const ITEM_REVOLVER = 1
export const ITEM_AK5 = 2
export const ITEM_GRENADE = 3
export const ITEM_BANDAGE = 4

export type ItemKind = 'gun' | 'grenade' | 'heal'

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
  stack: number
  /** What the player holds it as: picks the pose and the model. */
  stance: 'rifle' | 'pistol' | 'hold'
}

const DEFS: Record<number, ItemDef> = {
  [ITEM_REVOLVER]: { name: 'Revolver', kind: 'gun', auto: false, damage: 35, cooldownTicks: 30, mag: 6, reloadTicks: 2 * TICK_RATE, range: 150, stack: 1, stance: 'pistol' },
  [ITEM_AK5]: { name: 'AK5', kind: 'gun', auto: true, damage: 12, cooldownTicks: 6, mag: 30, reloadTicks: (5 * TICK_RATE) / 2, range: 200, stack: 1, stance: 'rifle' },
  [ITEM_GRENADE]: { name: 'Grenade', kind: 'grenade', auto: false, damage: 0, cooldownTicks: TICK_RATE, mag: 0, reloadTicks: 0, range: 0, stack: 3, stance: 'hold' },
  [ITEM_BANDAGE]: { name: 'Bandage', kind: 'heal', auto: false, damage: 40, cooldownTicks: 0, mag: 0, reloadTicks: 0, range: 0, stack: 5, stance: 'hold' },
}

/** The definition of `item`; undefined for an empty slot or an unknown id. */
export const itemDef = (item: number): ItemDef | undefined => DEFS[item]

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
