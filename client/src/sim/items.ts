/**
 * The items of `crates/shared/src/items.rs`. The table is built from `tables.generated.json`, which the
 * Rust tests write from the server's (`UPDATE_PARITY=1 cargo test -p shared parity`); only how an item
 * is held and sounds is the client's own, in `PRESENTATION`. The `ITEM_*` ids and the scalar constants
 * below are checked against the Rust source by `consts-parity.spec.ts`.
 */

import { BUTTON_ALT, TICK_RATE } from './consts'
import tables from './tables.generated.json'

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
/** Bricks: fire puts a cube where the crosshair is, aim and fire digs one out. Never used up. */
export const ITEM_BRICKS = 11
/** A Carl Gustaf over the shoulder: each shot a rocket, gone after the last; aim looks down its sight. */
export const ITEM_CARL_GUSTAF = 12
/** An anti-tank rifle: one hard-cored round at a time, through a tank's thin rear, roof and slits. */
export const ITEM_AT_RIFLE = 13

export type ItemKind = 'gun' | 'grenade' | 'heal' | 'drone' | 'flare' | 'build' | 'launcher'

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
  stance: 'rifle' | 'pistol' | 'hold' | 'shoulder'
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

/** What the client adds to an item: how it is held and what it sounds like. Every item needs one. */
type Presentation = Pick<ItemDef, 'stance' | 'sound'>

const PRESENTATION: Record<number, Presentation> = {
  [ITEM_REVOLVER]: { stance: 'pistol', sound: 'revolver' },
  [ITEM_AK5]: { stance: 'rifle', sound: 'rifle' },
  [ITEM_SNIPER]: { stance: 'rifle', sound: 'sniper' },
  [ITEM_SHOTGUN]: { stance: 'rifle', sound: 'shotgun' },
  [ITEM_GRENADE]: { stance: 'hold', sound: 'rifle' },
  [ITEM_BANDAGE]: { stance: 'hold', sound: 'rifle' },
  [ITEM_DRONE]: { stance: 'hold', sound: 'rifle' },
  [ITEM_LOITER]: { stance: 'hold', sound: 'rifle' },
  [ITEM_FLARE]: { stance: 'pistol', sound: 'revolver' },
  [ITEM_GOLDEN_EGG]: { stance: 'hold', sound: 'rifle' },
  [ITEM_BRICKS]: { stance: 'hold', sound: 'rifle' },
  [ITEM_CARL_GUSTAF]: { stance: 'shoulder', sound: 'sniper' },
  [ITEM_AT_RIFLE]: { stance: 'rifle', sound: 'sniper' },
}

/** The server's rows, as the generated table has them. */
type ItemRow = Omit<ItemDef, 'stance' | 'sound'> & { id: number }

const DEFS: Record<number, ItemDef> = Object.fromEntries(
  (tables.items as ItemRow[]).map(({ id, ...row }) => {
    const presentation = PRESENTATION[id]
    if (!presentation) throw new Error(`item ${id} (${row.name}) has no PRESENTATION entry in sim/items.ts`)
    return [id, { ...row, ...presentation }]
  }),
)

/** What a soldier carries, a slot each, unless their player chose otherwise. */
export const STANDARD_LOADOUT: readonly number[] = tables.standardLoadout

/** The items a player can start with: all but what only a rooster gives. Mirrors `can_start_with`. */
export const LOADOUT_ITEMS: readonly number[] = Object.keys(DEFS).map(Number).filter((item) => item !== ITEM_GOLDEN_EGG)

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

/** Walking speed multiplier for `item` in hand with `buttons`: a gun or a launcher slows its holder while aim is held. Mirrors `aim_speed_scale`. */
export const aimSpeedScale = (item: number, buttons: number): number => {
  const d = DEFS[item]
  return (d?.kind === 'gun' || d?.kind === 'launcher') && (buttons & BUTTON_ALT) !== 0 ? d.aimSpeed : 1
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
export const FLARE_BURN_TICKS = 20 * TICK_RATE
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
