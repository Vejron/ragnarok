import { describe, expect, it } from 'vitest'
import { BUTTON_JUMP, BUTTON_RUN, HELI_HOVER_LEVER, HELI_LEVER_RATE, TICK_DT } from '@/sim/consts'
import { CollectiveLever } from '@/sim/collective'

describe('a helicopter pilot\'s collective lever', () => {
  it('rises on jump, falls on run and stays where it is let go', () => {
    const lever = new CollectiveLever()
    for (let tick = 0; tick < 60; tick++) lever.step(BUTTON_JUMP)
    expect(lever.value).toBeCloseTo(HELI_LEVER_RATE, 5)
    for (let tick = 0; tick < 60; tick++) lever.step(0)
    expect(lever.value).toBeCloseTo(HELI_LEVER_RATE, 5)
    expect(lever.step(BUTTON_RUN)).toBe(Math.round((HELI_LEVER_RATE - HELI_LEVER_RATE * TICK_DT) * 255))
    expect(lever.step(BUTTON_JUMP | BUTTON_RUN)).toBe(lever.byte)
  })

  it('stops at either end of its travel', () => {
    const lever = new CollectiveLever()
    expect(lever.step(BUTTON_RUN)).toBe(0)
    for (let tick = 0; tick < 600; tick++) lever.step(BUTTON_JUMP)
    expect(lever.value).toBe(1)
    expect(lever.byte).toBe(255)
  })

  it('starts down on the ground and where it hovers in the air', () => {
    const lever = new CollectiveLever()
    lever.take(1, 0.2)
    expect(lever.value).toBe(0)
    lever.take(1, 30)
    expect(lever.value).toBe(HELI_HOVER_LEVER)
    // Falling with its rotor stopped, there is nothing to hover on.
    lever.take(0, 30)
    expect(lever.value).toBe(0)
  })
})
