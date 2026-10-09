import { describe, expect, it } from 'vitest'
import { desiredTranslation, wishVelocity } from '@/sim/movement'
import { BUTTON_JUMP, BUTTON_RUN, RUN_SPEED, TICK_DT, WALK_SPEED } from '@/sim/consts'

const frame = (strafe: number, forward: number, yaw: number, buttons = 0) => ({ seq: 1, strafe, forward, yaw, buttons })

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
    expect(desiredTranslation(0, true, frame(0, 0, 0, BUTTON_JUMP), TICK_DT).vy).toBeGreaterThan(0)
    expect(desiredTranslation(0, false, frame(0, 0, 0, BUTTON_JUMP), TICK_DT).vy).toBeLessThan(0)
  })
})
