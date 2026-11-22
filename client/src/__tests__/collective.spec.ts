import { describe, expect, it } from 'vitest'
import { BUTTON_JUMP, BUTTON_RUN, HELI_HOVER_LEVER, HELI_LEVER_RATE, TICK_DT } from '@/sim/consts'
import { CollectiveLever } from '@/sim/collective'
import { vehicleDef } from '@/sim/vehicles'
import { KIND_AIR_AMBULANCE, KIND_HELI } from '@/net/codec'

const HELI = vehicleDef(KIND_HELI).rotor!
const AMBULANCE = vehicleDef(KIND_AIR_AMBULANCE).rotor!

describe('a helicopter pilot\'s collective lever', () => {
  it('rises on jump, falls on run and stays where it is let go', () => {
    const lever = new CollectiveLever()
    lever.take(HELI, 0, 0)
    for (let tick = 0; tick < 60; tick++) lever.step(BUTTON_JUMP)
    expect(lever.value).toBeCloseTo(HELI_LEVER_RATE, 5)
    for (let tick = 0; tick < 60; tick++) lever.step(0)
    expect(lever.value).toBeCloseTo(HELI_LEVER_RATE, 5)
    expect(lever.step(BUTTON_RUN)).toBe(Math.round((HELI_LEVER_RATE - HELI_LEVER_RATE * TICK_DT) * 255))
    expect(lever.step(BUTTON_JUMP | BUTTON_RUN)).toBe(lever.byte)
  })

  it('stops at either end of its travel', () => {
    const lever = new CollectiveLever()
    lever.take(HELI, 0, 0)
    expect(lever.step(BUTTON_RUN)).toBe(0)
    for (let tick = 0; tick < 600; tick++) lever.step(BUTTON_JUMP)
    expect(lever.value).toBe(1)
    expect(lever.byte).toBe(255)
  })

  it('starts down on the ground and where it hovers in the air', () => {
    const lever = new CollectiveLever()
    lever.take(HELI, 1, 0.2)
    expect(lever.value).toBe(0)
    lever.take(HELI, 1, 30)
    expect(lever.value).toBe(HELI_HOVER_LEVER)
    // Falling with its rotor stopped, there is nothing to hover on.
    lever.take(HELI, 0, 30)
    expect(lever.value).toBe(0)
  })

  it('starts where each helicopter hovers, not where the gunship does', () => {
    const lever = new CollectiveLever()
    lever.take(AMBULANCE, 1, 30)
    expect(lever.value).toBe(AMBULANCE.hoverLever)
    expect(lever.value).not.toBe(HELI_HOVER_LEVER)
  })
})
