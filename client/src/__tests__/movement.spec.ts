import { describe, expect, it } from 'vitest'
import { beginSlide, decayKnock, desiredTranslation, wishVelocity } from '@/sim/movement'
import { aimSpeedScale, ITEM_AK5, ITEM_GRENADE, ITEM_SNIPER } from '@/sim/items'
import { BUTTON_ALT, BUTTON_CROUCH, BUTTON_JUMP, BUTTON_RUN, BUTTON_SLIDE, CROUCH_SPEED, SLIDE_SPEED, RUN_SPEED, TICK_DT, WALK_SPEED } from '@/sim/consts'

const frame = (strafe: number, forward: number, yaw: number, buttons = 0) => ({ seq: 1, strafe, forward, yaw, buttons, pitch: 0, view: 0, slot: 0 })

describe('movement', () => {
  it('walks slower by the speed scale', () => {
    const [, z] = wishVelocity(frame(0, 127, 0, BUTTON_RUN), 0.5)
    expect(z).toBeCloseTo(-RUN_SPEED * 0.5, 4)
  })

  it('only a gun with aim held slows its holder', () => {
    expect(aimSpeedScale(ITEM_AK5, 0)).toBe(1)
    expect(aimSpeedScale(ITEM_AK5, BUTTON_ALT)).toBeLessThan(1)
    expect(aimSpeedScale(ITEM_SNIPER, BUTTON_ALT)).toBeLessThan(aimSpeedScale(ITEM_AK5, BUTTON_ALT))
    expect(aimSpeedScale(ITEM_GRENADE, BUTTON_ALT)).toBe(1)
  })

  it('walks towards -Z at yaw 0', () => {
    const [x, z] = wishVelocity(frame(0, 127, 0))
    expect(x).toBeCloseTo(0, 6)
    expect(z).toBeCloseTo(-WALK_SPEED, 5)
  })

  it('does not run faster diagonally', () => {
    const [x, z] = wishVelocity(frame(127, 127, 0, BUTTON_RUN))
    expect(Math.hypot(x, z)).toBeCloseTo(RUN_SPEED, 4)
  })

  it('crouches slowly even while running', () => {
    const [x, z] = wishVelocity(frame(127, 127, 0, BUTTON_CROUCH | BUTTON_RUN))
    expect(Math.hypot(x, z)).toBeCloseTo(CROUCH_SPEED, 4)
  })

  it('slides once on a run-crouch press and keeps more speed while crouched', () => {
    const press = frame(0, 127, 0, BUTTON_RUN | BUTTON_CROUCH | BUTTON_SLIDE)
    const knock = beginSlide([0, 0], true, press)
    expect(knock[0]).toBeCloseTo(0, 5)
    expect(knock[1]).toBeCloseTo(-SLIDE_SPEED, 5)
    expect(beginSlide(knock, true, press)).toEqual(knock)
    expect(beginSlide([0, 0], false, press)).toEqual([0, 0])
    let crouched: [number, number] = [...knock]
    let standing: [number, number] = [...knock]
    for (let i = 0; i < 30; i++) {
      crouched = decayKnock(crouched, true, true)
      standing = decayKnock(standing, true, false)
    }
    expect(Math.abs(crouched[1])).toBeGreaterThan(Math.abs(standing[1]) * 2)
  })

  it('jumps only from the ground', () => {
    expect(desiredTranslation(0, true, [0, 0], frame(0, 0, 0, BUTTON_JUMP), TICK_DT).vy).toBeGreaterThan(0)
    expect(desiredTranslation(0, false, [0, 0], frame(0, 0, 0, BUTTON_JUMP), TICK_DT).vy).toBeLessThan(0)
  })

  it('adds knockback to walking and lets it die out', () => {
    const { desired } = desiredTranslation(0, true, [6, -3], frame(0, 0, 0), TICK_DT)
    expect(desired[0]).toBeCloseTo(6 * TICK_DT, 6)
    expect(desired[2]).toBeCloseTo(-3 * TICK_DT, 6)
    let knock: [number, number] = [6, -3]
    for (let i = 0; i < 180; i++) knock = decayKnock(knock, true)
    expect(knock).toEqual([0, 0])
    let air: [number, number] = [6, 0]
    for (let i = 0; i < 30; i++) air = decayKnock(air, false)
    expect(air[0]).toBeGreaterThan(4)
  })
})
