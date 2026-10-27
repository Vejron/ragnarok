/**
 * An armoured vehicle's parts, as `crates/shared/src/armor.rs` has them: their slots, the state the
 * snapshot carries for each (two bits apiece in `DAMAGE_PARTS`), and what the shooter and crew are told
 * when a round meets the armour (`Armor` messages).
 */

import { ARMOR_BLOCKED, ARMOR_PIERCED, ARMOR_RICOCHET, ARMOR_SLIT, BURNING, DAMAGE_FIRE, DAMAGE_PARTS } from '@/net/codec'

/** `Part::slot`. */
export const PART_ENGINE = 0
export const PART_TRANSMISSION = 1
export const PART_TRACK_LEFT = 2
export const PART_TRACK_RIGHT = 3
export const PART_BREECH = 4
export const PART_AMMO = 5
export const PART_FUEL = 6
export const PARTS = 7

/** `armor::State`. */
export const SOUND = 0
export const HURT = 1
export const GONE = 2

/** The state of the part in `slot`, from a vehicle's damage as the snapshot carries it. */
export function partState(damage: readonly number[], slot: number): number {
  const bits = (damage[DAMAGE_PARTS] ?? 0) | ((damage[DAMAGE_PARTS + 1] ?? 0) << 8)
  return Math.min(GONE, (bits >> (2 * slot)) & 3)
}

/** Whether its fuel is alight. */
export function burning(damage: readonly number[]): boolean {
  return damage[DAMAGE_FIRE] === BURNING
}

const PART_NAMES = ['Engine', 'Transmission', 'Left track', 'Right track', 'Gun breech', 'Ammo rack', 'Fuel tank']
const GONE_WORDS = ['destroyed', 'destroyed', 'broken', 'broken', 'destroyed', 'cooked off!', 'on fire!']
const SEAT_NAMES = ['Driver', 'Gunner']

/** What an `Armor` message says, as it arrives. */
export interface ArmorMessage {
  role: number
  result: number
  hurt: number
  gone: number
  killed: number
}

/** One line of a callout, and how it reads: good news for the shooter, bad for the crew, or neither. */
export interface CalloutLine {
  text: string
  tone: 'good' | 'bad' | 'plain'
}

/**
 * The lines a round's `Armor` message makes, most telling first. `dealt`: for the shooter, who wants to
 * know it went in; otherwise for the crew, who want to know what it broke.
 */
export function calloutLines({ result, hurt, gone, killed }: ArmorMessage, dealt: boolean): CalloutLine[] {
  const good = dealt ? 'good' : 'bad'
  const lines: CalloutLine[] = []
  if (result === ARMOR_RICOCHET) lines.push({ text: 'Ricochet', tone: 'plain' })
  else if (result === ARMOR_BLOCKED) lines.push({ text: dealt ? 'No penetration' : 'Bounced off the armour', tone: 'plain' })
  else if (result === ARMOR_SLIT) lines.push({ text: dealt ? 'Through the slit!' : 'Shot through the slit!', tone: good })
  else if (result === ARMOR_PIERCED) lines.push({ text: dealt ? 'Penetration' : 'Penetrated!', tone: good })
  for (let seat = 0; seat < 8; seat++) {
    if (killed & (1 << seat)) lines.push({ text: `${SEAT_NAMES[seat] ?? 'Crew'} killed`, tone: good })
  }
  for (let slot = 0; slot < PARTS; slot++) {
    const bit = 1 << slot
    if (gone & bit) lines.push({ text: `${PART_NAMES[slot]} ${GONE_WORDS[slot]}`, tone: good })
    else if (hurt & bit) lines.push({ text: `${PART_NAMES[slot]} hit`, tone: good })
  }
  return lines
}
