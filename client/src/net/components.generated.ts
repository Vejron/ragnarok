// GENERATED from crates/shared/src/replicated.rs by its tests. Do not edit: change the Rust and
// run `UPDATE_PARITY=1 cargo test -p shared`. What each component means is documented there.

/** Bit 31 of an update's `set` mask: a `cleared` mask follows it. */
export const CLEARED = 2147483648

/** What a field's integer counts (`UNITS` in the Rust): how the client turns it into its own units, and blends it. */
export type Unit = 'cm' | 'mrad' | 'quat' | 'axis' | 'byte' | 'dm'

export interface ItemSlot {
  item: number
  count: number
  ammo: number
}

export interface Place {
  x: number
  y: number
  z: number
}

export interface Heading {
  yaw: number
}

export interface Motion {
  vy: number
  knock_x: number
  knock_z: number
}

export interface Flags {
  flags: number
  stance: number
}

export interface Orientation {
  qx: number
  qy: number
  qz: number
}

export interface Steer {
  steer: number
}

export interface Turret {
  yaw: number
  pitch: number
  shots: number
  range: number
  flags: number
}

export interface Limbs {
  lost: number
}

export interface Thrust {
  air: number
  flags: number
}

export interface Arms {
  shots: number
  heat: number
  rear_heat: number
  bombs: number
  flags: number
}

export interface Health {
  hp: number
}

export interface Damage {
  parts: number[]
}

export interface Velocity {
  vx: number
  vy: number
  vz: number
}

export interface Kind {
  kind: number
}

export interface Projectile {
  kind: number
}

export interface Pickup {
  item: number
}

export interface Vitals {
  hp: number
  progress: number
  spent: number
  winded: number
  parts: number[]
  down: number
}

export interface Hotbar {
  slots: ItemSlot[]
}

export interface Drive {
  vx: number
  vy: number
  vz: number
  wx: number
  wy: number
  wz: number
  steer: number
  spin: number
}

export interface Seated {
  vehicle: number
  seat: number
}

export interface Look {
  pitch: number
}

export interface Carry {
  what: number
}

export interface Struck {
  count: number
  part: number
  amount: number
  yaw: number
  x: number
  y: number
  z: number
}

export interface Pintle {
  yaw: number
  pitch: number
  shots: number
  heat: number
  flags: number
}

/** An entity as it is replicated: the components it has. */
export interface Components {
  place?: Place
  heading?: Heading
  motion?: Motion
  flags?: Flags
  orientation?: Orientation
  steer?: Steer
  turret?: Turret
  limbs?: Limbs
  thrust?: Thrust
  arms?: Arms
  health?: Health
  damage?: Damage
  velocity?: Velocity
  kind?: Kind
  projectile?: Projectile
  pickup?: Pickup
  vitals?: Vitals
  hotbar?: Hotbar
  drive?: Drive
  seated?: Seated
  look?: Look
  carry?: Carry
  struck?: Struck
  pintle?: Pintle
}

export type ComponentName = keyof Components

/** A field, in wire order: its unit if it has one, and the struct's fields if it holds a list of structs. */
export interface FieldSchema {
  name: string
  unit?: Unit
  struct?: readonly FieldSchema[]
}

/** Every component in wire-id order: its bit, whether it is sent as a difference and only to its owner, and its fields. */
export const SCHEMA: readonly { id: number; name: ComponentName; diff: boolean; owner: boolean; fields: readonly FieldSchema[] }[] = [
  { id: 0, name: 'place', diff: true, owner: false, fields: [{ name: 'x', unit: 'cm' }, { name: 'y', unit: 'cm' }, { name: 'z', unit: 'cm' }] },
  { id: 1, name: 'heading', diff: true, owner: false, fields: [{ name: 'yaw', unit: 'mrad' }] },
  { id: 2, name: 'motion', diff: true, owner: false, fields: [{ name: 'vy', unit: 'cm' }, { name: 'knock_x', unit: 'cm' }, { name: 'knock_z', unit: 'cm' }] },
  { id: 3, name: 'flags', diff: false, owner: false, fields: [{ name: 'flags' }, { name: 'stance' }] },
  { id: 4, name: 'orientation', diff: true, owner: false, fields: [{ name: 'qx', unit: 'quat' }, { name: 'qy', unit: 'quat' }, { name: 'qz', unit: 'quat' }] },
  { id: 5, name: 'steer', diff: true, owner: false, fields: [{ name: 'steer', unit: 'axis' }] },
  { id: 6, name: 'turret', diff: false, owner: false, fields: [{ name: 'yaw', unit: 'mrad' }, { name: 'pitch', unit: 'mrad' }, { name: 'shots' }, { name: 'range', unit: 'dm' }, { name: 'flags' }] },
  { id: 7, name: 'limbs', diff: false, owner: false, fields: [{ name: 'lost' }] },
  { id: 8, name: 'thrust', diff: false, owner: false, fields: [{ name: 'air', unit: 'byte' }, { name: 'flags' }] },
  { id: 9, name: 'arms', diff: false, owner: false, fields: [{ name: 'shots' }, { name: 'heat', unit: 'byte' }, { name: 'rear_heat', unit: 'byte' }, { name: 'bombs' }, { name: 'flags' }] },
  { id: 11, name: 'health', diff: false, owner: false, fields: [{ name: 'hp' }] },
  { id: 12, name: 'damage', diff: false, owner: false, fields: [{ name: 'parts' }] },
  { id: 13, name: 'velocity', diff: true, owner: false, fields: [{ name: 'vx', unit: 'cm' }, { name: 'vy', unit: 'cm' }, { name: 'vz', unit: 'cm' }] },
  { id: 14, name: 'kind', diff: false, owner: false, fields: [{ name: 'kind' }] },
  { id: 15, name: 'projectile', diff: false, owner: false, fields: [{ name: 'kind' }] },
  { id: 16, name: 'pickup', diff: false, owner: false, fields: [{ name: 'item' }] },
  { id: 17, name: 'vitals', diff: false, owner: true, fields: [{ name: 'hp' }, { name: 'progress', unit: 'byte' }, { name: 'spent' }, { name: 'winded' }, { name: 'parts' }, { name: 'down' }] },
  { id: 18, name: 'hotbar', diff: false, owner: true, fields: [{ name: 'slots', struct: [{ name: 'item' }, { name: 'count' }, { name: 'ammo' }] }] },
  { id: 19, name: 'drive', diff: false, owner: true, fields: [{ name: 'vx', unit: 'cm' }, { name: 'vy', unit: 'cm' }, { name: 'vz', unit: 'cm' }, { name: 'wx', unit: 'mrad' }, { name: 'wy', unit: 'mrad' }, { name: 'wz', unit: 'mrad' }, { name: 'steer', unit: 'mrad' }, { name: 'spin', unit: 'mrad' }] },
  { id: 20, name: 'seated', diff: false, owner: false, fields: [{ name: 'vehicle' }, { name: 'seat' }] },
  { id: 21, name: 'look', diff: true, owner: false, fields: [{ name: 'pitch', unit: 'mrad' }] },
  { id: 22, name: 'carry', diff: false, owner: false, fields: [{ name: 'what' }] },
  { id: 23, name: 'struck', diff: false, owner: false, fields: [{ name: 'count' }, { name: 'part' }, { name: 'amount' }, { name: 'yaw', unit: 'mrad' }, { name: 'x', unit: 'cm' }, { name: 'y', unit: 'cm' }, { name: 'z', unit: 'cm' }] },
  { id: 24, name: 'pintle', diff: false, owner: false, fields: [{ name: 'yaw', unit: 'mrad' }, { name: 'pitch', unit: 'mrad' }, { name: 'shots' }, { name: 'heat', unit: 'byte' }, { name: 'flags' }] },
]
