/**
 * A stickman's limbs as the snapshot carries them (`limbs`, from `crates/server/src/mechanics/limbs.rs`):
 * the ones lost, and the stumps still bleeding above them.
 */

import { LIMB_ALL, LIMB_ARM_L, LIMB_ARM_R, LIMB_BLEED_SHIFT, LIMB_LEG_L } from '@/net/codec'

const NAMES: [number, string][] = [
  [LIMB_LEG_L, 'Left leg'],
  [LIMB_LEG_L << 1, 'Right leg'],
  [LIMB_ARM_L, 'Left arm'],
  [LIMB_ARM_R, 'Right arm'],
]

/** What the first limb in `limbs` (`LIMB_*` bits) is called, a leg before an arm. */
export function limbName(limbs: number): string {
  return NAMES.find(([bit]) => limbs & bit)?.[1] ?? 'Limb'
}

/** The limbs lost, from a `limbs` component's value. */
export const lostOf = (wire: number) => wire & LIMB_ALL

/** The stumps still bleeding, from a `limbs` component's value. */
export const bleedingOf = (wire: number) => (wire >> LIMB_BLEED_SHIFT) & LIMB_ALL
