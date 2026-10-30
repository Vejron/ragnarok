import { describe, expect, it } from 'vitest'
import { LocalPlayer } from '@/localplayer'
import { HEAD_HP, PLAYER_MAX_HP, RESPAWN_TICKS, TICK_MS } from '@/sim/consts'
import { seatHint, aimsReticle } from '@/seathints'
import type { Entity } from '@/entities/units'

const vitals = (hp: number) => ({ vitals: { hp, progress: 9, spent: 0, winded: 0, parts: [10, 5, 5, 5, 5], down: 0 } }) as unknown as Entity

describe('the local player', () => {
  it('counts the wait to come back from when they died, and not again while dead', () => {
    const player = new LocalPlayer()
    player.receive(vitals(0), 1000)
    player.receive(vitals(0), 2000)
    expect(player.alive).toBe(false)
    expect(player.respawnIn(1000)).toBeCloseTo((RESPAWN_TICKS * TICK_MS) / 1000)
    player.receive(vitals(50), 3000)
    expect(player.respawnIn(3000)).toBe(0)
  })

  it('is whole again after a reset, with nothing left of the last session', () => {
    const player = new LocalPlayer()
    player.receive({ ...vitals(0), limbs: { lost: 1 }, carry: { what: 1 } } as unknown as Entity, 1000)
    expect(player.carrying).toBe(true)
    player.reset()
    expect(player.hp).toBe(PLAYER_MAX_HP)
    expect(player.limbs).toBe(0)
    expect(player.carrying).toBe(false)
    expect(player.parts[0]).toBe(HEAD_HP)
    expect(player.progress).toBe(0)
    expect(player.body()).toBeNull()
  })
})

describe('seat hints', () => {
  const stats = { ammo: null, reload: 1, range: 120.4, canSwitchSeat: false, jammed: false, heat: null }

  it('say nothing out of a vehicle', () => {
    expect(seatHint(null, stats)).toBeNull()
  })

  it('offer a seat switch only where there is one', () => {
    expect(seatHint('driver', stats)).not.toContain('R switch seat')
    expect(seatHint('driver', { ...stats, canSwitchSeat: true })).toContain('R switch seat')
  })

  it('read the state of the gun', () => {
    expect(seatHint('gunner', { ...stats, jammed: true })).toContain('jammed!')
    expect(seatHint('gunner', { ...stats, ammo: 'HE', reload: 0.5 })).toContain('Q load AP · HE reloading…')
    expect(seatHint('mortar', stats)).toContain('range 120 m')
  })

  it('put a reticle only on seats that aim at the middle of the screen', () => {
    expect(aimsReticle('gunner')).toBe(true)
    expect(aimsReticle('driver')).toBe(false)
    expect(aimsReticle(null)).toBe(false)
  })
})
