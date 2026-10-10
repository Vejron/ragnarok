import { describe, expect, it } from 'vitest'
import { decayKnock, desiredTranslation, wishVelocity } from '@/sim/movement'
import { BUTTON_JUMP, BUTTON_RUN, RUN_SPEED, TICK_DT, WALK_SPEED } from '@/sim/consts'

const frame = (strafe: number, forward: number, yaw: number, buttons = 0) => ({ seq: 1, strafe, forward, yaw, buttons, pitch: 0, view: 0 })

describe('movement', () => {
  it('walks towards -Z at yaw 0', () => {
    const [x, z] = wishVelocity(frame(0, 127, 0))
    expect(x).toBeCloseTo(0, 6)
    expect(z).toBeCloseTo(-WALK_SPEED, 5)
  })

  it('does not run faster diagonally', () => {
    const [x, z] = wishVelocity(frame(127, 127, 0, BUTTON_RUN))
    expect(Math.hypot(x, z)).toBeCloseTo(RUN_SPEED, 4)
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
