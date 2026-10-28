import { describe, expect, it } from 'vitest'
import { Callouts } from '@/callouts'
import { LIMB_ARM_R, LIMB_BLEED_SHIFT, LIMB_LEG_L, LIMB_LEG_R } from '@/net/codec'
import { bleedingOf, limbName, lostOf } from '@/sim/limbs'

describe('limbs as the snapshot carries them', () => {
  it('splits what is lost from what still bleeds, and names a leg before an arm', () => {
    const wire = LIMB_LEG_R | LIMB_ARM_R | (LIMB_ARM_R << LIMB_BLEED_SHIFT)
    expect(lostOf(wire)).toBe(LIMB_LEG_R | LIMB_ARM_R)
    expect(bleedingOf(wire)).toBe(LIMB_ARM_R)
    expect(limbName(LIMB_ARM_R | LIMB_LEG_R)).toBe('Right leg')
    expect(limbName(LIMB_LEG_L)).toBe('Left leg')
  })

  it('are called out like a round on armour, joined when it happens again at once', () => {
    const callouts = new Callouts()
    callouts.say({ text: 'Left leg off!', tone: 'good' }, 0)
    callouts.say({ text: 'Left leg off!', tone: 'good' }, 100)
    expect(callouts.view(200)).toEqual([{ text: 'Left leg off!', tone: 'good', count: 2, alpha: 1 }])
  })
})
