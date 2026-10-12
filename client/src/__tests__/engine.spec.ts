import { describe, expect, it } from 'vitest'
import { crashSize, gearing, groundGain, landingThud, mixOf, squealGain, type SoundInput } from '@/sim/engine'
import { SPORTS_MAX_SPEED } from '@/sim/consts'
import { KIND_HELI, KIND_JEEP, KIND_SPORTS, KIND_TANK } from '@/net/codec'
import { SURFACE_ASPHALT, SURFACE_DIRT, SURFACE_WATER } from '@/sim/surface'

const running = (over: Partial<SoundInput> = {}): SoundInput => ({
  kind: KIND_JEEP, speed: 10, accel: 1, climb: 0, powered: true, hp: 100, maxHp: 100, slip: 0, surface: SURFACE_ASPHALT, grounded: true, wheels: null, ...over,
})

describe('gearing', () => {
  it('climbs through a gear and drops back at each shift', () => {
    let last = gearing(KIND_SPORTS, 0)
    let shifts = 0
    for (let v = 0.5; v <= SPORTS_MAX_SPEED; v += 0.5) {
      const now = gearing(KIND_SPORTS, v)
      if (now.gear === last.gear) expect(now.revs).toBeGreaterThanOrEqual(last.revs)
      else {
        shifts++
        expect(now.gear).toBe(last.gear + 1)
        expect(now.revs).toBeLessThan(last.revs)
      }
      last = now
    }
    expect(shifts).toBe(4)
    expect(last.revs).toBeGreaterThan(0.95)
  })

  it('idles at a standstill, is the same either way round, and a tank has one gear', () => {
    expect(gearing(KIND_JEEP, 0).revs).toBeCloseTo(0.25)
    expect(gearing(KIND_JEEP, -12)).toEqual(gearing(KIND_JEEP, 12))
    expect(gearing(KIND_TANK, 11).gear).toBe(0)
  })
})

describe('mixOf', () => {
  it('is silent in an empty car and has no engine in a wreck', () => {
    expect(mixOf(running({ powered: false })).engine).toBe(0)
    const wreck = mixOf(running({ hp: 0 }))
    expect(wreck.engine).toBe(0)
    expect(wreck.fire).toBe(true)
  })

  it('revs higher and louder when going faster and pulling', () => {
    const slow = mixOf(running({ speed: 3, accel: 0 }))
    const fast = mixOf(running({ speed: 9, accel: 0 }))
    expect(fast.pitch).toBeGreaterThan(slow.pitch)
    const coasting = mixOf(running({ accel: -4 }))
    const pulling = mixOf(running({ accel: 4 }))
    expect(pulling.engine).toBeGreaterThan(coasting.engine)
    expect(pulling.tone).toBeGreaterThan(coasting.tone)
  })

  it('gives each kind its own voice', () => {
    const pitch = (kind: number) => mixOf(running({ kind, speed: 8 })).pitch
    expect(pitch(KIND_SPORTS)).toBeGreaterThan(pitch(KIND_JEEP))
    expect(pitch(KIND_TANK)).toBeLessThan(pitch(KIND_JEEP))
    expect(mixOf(running({ kind: KIND_HELI, speed: 0 })).chop).toBeGreaterThan(8)
    expect(mixOf(running({ kind: KIND_TANK, speed: 8 })).chop).toBeGreaterThan(10)
    expect(mixOf(running({ kind: KIND_JEEP })).chop).toBe(0)
  })

  it('beats the blades faster when the helicopter climbs', () => {
    const hover = mixOf(running({ kind: KIND_HELI, speed: 0, climb: 0 }))
    const climbing = mixOf(running({ kind: KIND_HELI, speed: 0, climb: 6 }))
    expect(climbing.chop).toBeGreaterThan(hover.chop)
  })

  it('stutters and hisses as health falls', () => {
    expect(mixOf(running({ hp: 90 })).stutter).toBe(0)
    expect(mixOf(running({ hp: 30 })).stutter).toBeGreaterThan(0)
    expect(mixOf(running({ hp: 30 })).hiss).toBe(0)
    expect(mixOf(running({ hp: 10 })).hiss).toBeGreaterThan(0)
    expect(mixOf(running({ hp: 5 })).stutter).toBeGreaterThan(mixOf(running({ hp: 30 })).stutter)
  })

  it('rattles for a bent wheel and grinds for a lost one, but only while rolling', () => {
    const bent = mixOf(running({ wheels: [0, 140, 0, 0] })).rattle
    const lost = mixOf(running({ wheels: [0, 255, 0, 0] })).rattle
    expect(bent).toBeGreaterThan(0)
    expect(lost).toBeGreaterThan(bent)
    expect(mixOf(running({ wheels: [0, 255, 0, 0], speed: 0 })).rattle).toBe(0)
    expect(mixOf(running({ wheels: [0, 0, 0, 0] })).rattle).toBe(0)
  })

  it('squeals with the slip that leaves skidmarks, not in the air and not when parked', () => {
    expect(mixOf(running({ slip: 0.8 })).squeal).toBeGreaterThan(0.4)
    expect(mixOf(running({ slip: 0.05 })).squeal).toBe(0)
    expect(mixOf(running({ slip: 0.8, grounded: false })).squeal).toBe(0)
    expect(mixOf(running({ slip: 0.8, speed: 0 })).squeal).toBe(0)
    // A parked car's tyres still crunch, so an unpowered one keeps its ground noise.
    expect(mixOf(running({ powered: false, speed: 15, surface: SURFACE_DIRT })).ground).toBeGreaterThan(0)
  })
})

describe('sound levels', () => {
  it('squeals loudest on tarmac, less on dirt and not in water', () => {
    const tarmac = squealGain(0.8, SURFACE_ASPHALT, 20)
    expect(squealGain(0.8, SURFACE_DIRT, 20)).toBeLessThan(tarmac)
    expect(squealGain(0.8, SURFACE_DIRT, 20)).toBeGreaterThan(0)
    expect(squealGain(0.8, SURFACE_WATER, 20)).toBe(0)
  })

  it('crunches gravel off-road in proportion to speed', () => {
    expect(groundGain(20, SURFACE_DIRT)).toBeGreaterThan(groundGain(5, SURFACE_DIRT))
    expect(groundGain(20, SURFACE_DIRT)).toBeGreaterThan(groundGain(20, SURFACE_ASPHALT))
  })

  it('thuds only for a real drop, harder the further', () => {
    expect(landingThud(2)).toBe(0)
    expect(landingThud(6)).toBeGreaterThan(0)
    expect(landingThud(14)).toBeGreaterThan(landingThud(6))
    expect(landingThud(40)).toBeLessThanOrEqual(1)
  })

  it('picks a crash size by speed', () => {
    expect([crashSize(3), crashSize(10), crashSize(25)]).toEqual([0, 1, 2])
  })
})
