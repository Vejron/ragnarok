import { describe, expect, it } from 'vitest'
import { kindView } from '@/kinds'
import { crashSize, gasOf, landingThud, mixOf as mixWith, Revs, squealGain, type Motor, type RevState, type SoundInput } from '@/sim/engine'
import { groundOf, rolling, type Ground } from '@/sim/groundsound'
import { SPORTS_MAX_SPEED } from '@/sim/consts'
import { KIND_CB90, KIND_HELI, KIND_JEEP, KIND_PLANE, KIND_SEAPLANE, KIND_SNOWMOBILE, KIND_SPORTS, KIND_SUBMARINE, KIND_TANK } from '@/net/codec'
import { SURFACE_ASPHALT, SURFACE_DIRT, SURFACE_GRASS, SURFACE_MUD, SURFACE_SAND, SURFACE_WATER } from '@/sim/surface'

/** The engine of `kind`, which has one. */
function motorOf(kind: number): Motor {
  const engine = kindView(kind).sound.engine
  if (engine.name === 'none') throw new Error(`kind ${kind} has no engine`)
  return engine
}

const running = (over: Partial<SoundInput> = {}): SoundInput => ({
  kind: KIND_JEEP, speed: 10, accel: 1, climb: 0, throttle: 0.6, powered: true, hp: 100, maxHp: 100, slip: 0, ground: 'tarmac', grounded: true, yawRate: 0, wheels: null, ...over,
})

/** Runs `kind`'s revs for `seconds` with `input`, and gives where they end. */
function settle(kind: number, input: Partial<SoundInput>, seconds = 8, revs = new Revs()): RevState {
  let state: RevState = { rpm: 0, load: 0, gear: 0 }
  for (let t = 0; t < seconds; t += 1 / 60) state = revs.step(motorOf(kind), running({ kind, ...input }), 1 / 60)
  return state
}

/** The mix of the kind `input` names, with its revs settled. */
const mixOf = (input: SoundInput) => mixWith(input, kindView(input.kind).sound.engine, settle(input.kind, input))

describe('Revs', () => {
  it('idles at a standstill with no throttle and revs up when the driver blips it', () => {
    const motor = motorOf(KIND_JEEP)
    const idle = settle(KIND_JEEP, { speed: 0, throttle: 0 })
    expect(idle.rpm).toBeCloseTo(motor.idle, -1)
    // Out of gear and floored, it climbs high: the clutch is still slipping.
    const revved = settle(KIND_JEEP, { speed: 0, throttle: 1 })
    expect(revved.rpm).toBeGreaterThan(motor.idle + (motor.redline - motor.idle) * 0.45)
    expect(revved.load).toBe(1)
  })

  it('climbs through the gears, falling back at each shift and lifting off for it', () => {
    const motor = motorOf(KIND_SPORTS)
    const revs = new Revs()
    let last = revs.step(motor, running({ kind: KIND_SPORTS, speed: 0, throttle: 1 }), 1 / 60)
    let shifts = 0
    let lifted = false
    for (let v = 0; v <= SPORTS_MAX_SPEED * 0.95; v += 0.05) {
      const now = revs.step(motor, running({ kind: KIND_SPORTS, speed: v, throttle: 1 }), 1 / 60)
      if (now.gear > last.gear) shifts++
      if (now.load === 0) lifted = true
      last = now
    }
    expect(shifts).toBe(4)
    expect(lifted).toBe(true)
    expect(last.rpm).toBeGreaterThan(motor.redline * 0.8)
  })

  it('runs free when the wheels spin or leave the ground', () => {
    const gripping = settle(KIND_JEEP, { speed: 8, throttle: 1 })
    expect(settle(KIND_JEEP, { speed: 8, throttle: 1, slip: 0.8 }).rpm).toBeGreaterThan(gripping.rpm)
    expect(settle(KIND_JEEP, { speed: 8, throttle: 1, grounded: false }).rpm).toBeGreaterThan(gripping.rpm)
  })

  it('bounces off the limiter held flat out', () => {
    const motor = motorOf(KIND_JEEP)
    const revs = new Revs()
    const loads: number[] = []
    for (let t = 0; t < 3; t += 1 / 60) {
      const state = revs.step(motor, running({ speed: 0, throttle: 1, grounded: false }), 1 / 60)
      expect(state.rpm).toBeLessThanOrEqual(motor.redline)
      if (t > 2) loads.push(state.load)
    }
    expect(loads).toContain(0)
    expect(loads).toContain(1)
  })

  it('a snowmobile’s belt holds its revs while the speed builds', () => {
    const motor = motorOf(KIND_SNOWMOBILE)
    const slow = settle(KIND_SNOWMOBILE, { kind: KIND_SNOWMOBILE, speed: 4, throttle: 1 })
    const fast = settle(KIND_SNOWMOBILE, { kind: KIND_SNOWMOBILE, speed: 16, throttle: 1 })
    expect(slow.rpm).toBeGreaterThan(motor.idle + (motor.redline - motor.idle) * 0.5)
    expect(fast.rpm - slow.rpm).toBeLessThan((motor.redline - motor.idle) * 0.4)
    // Let go, and the belt lets the engine fall back to idle.
    expect(settle(KIND_SNOWMOBILE, { kind: KIND_SNOWMOBILE, speed: 10, throttle: 0 }).rpm).toBeLessThan(motor.idle * 1.4)
  })

  it('a tank revs slowly, a sports car snaps', () => {
    const share = (kind: number) => {
      const motor = motorOf(kind)
      return (settle(kind, { kind, speed: 0, throttle: 1, grounded: false }, 0.3).rpm - motor.idle) / (motor.redline - motor.idle)
    }
    expect(share(KIND_SPORTS)).toBeGreaterThan(share(KIND_TANK) * 2)
  })

  it('turns a boat’s propeller as the throttle says, however the waves rock its speed', () => {
    for (const kind of [KIND_SUBMARINE, KIND_CB90]) {
      const motor = motorOf(kind)
      const revs = new Revs()
      settle(kind, { kind, speed: 4, throttle: 0.6, ground: 'water' }, 8, revs)
      const seen: number[] = []
      for (let i = 0; i < 240; i++) seen.push(revs.step(motor, running({ kind, speed: 4 + 0.6 * Math.sin(i / 6), throttle: 0.6, ground: 'water' }), 1 / 60).rpm)
      expect((Math.max(...seen) - Math.min(...seen)) / motor.redline).toBeLessThan(0.03)
      expect(settle(kind, { kind, speed: 4, throttle: 1, ground: 'water' }).rpm).toBeGreaterThan(settle(kind, { kind, speed: 4, throttle: 0.3, ground: 'water' }).rpm)
    }
  })

  it('runs down when nobody drives it', () => {
    const revs = new Revs()
    settle(KIND_JEEP, { throttle: 1 }, 2, revs)
    expect(settle(KIND_JEEP, { powered: false }, 3, revs).rpm).toBeLessThan(50)
  })

  it('takes the stick against the roll as the brake, not the gas', () => {
    expect(gasOf(-1, 10)).toBe(0)
    expect(gasOf(-1, 0)).toBe(1)
    expect(gasOf(-0.5, -6)).toBe(0.5)
  })
})

describe('mixOf', () => {
  it('is silent in an empty car and has no engine in a wreck', () => {
    expect(mixOf(running({ powered: false })).engine).toBe(0)
    const wreck = mixOf(running({ hp: 0 }))
    expect(wreck.engine).toBe(0)
    expect(wreck.fire).toBe(true)
  })

  it('gives each kind its own revs', () => {
    const rpm = (kind: number) => mixOf(running({ kind, speed: 8 })).rpm
    expect(rpm(KIND_SPORTS)).toBeGreaterThan(rpm(KIND_JEEP))
    expect(rpm(KIND_TANK)).toBeLessThan(rpm(KIND_JEEP))
    expect(mixOf(running({ kind: KIND_HELI, speed: 0 })).chop).toBeGreaterThan(8)
    expect(mixOf(running({ kind: KIND_TANK, speed: 8 })).chop).toBeGreaterThan(10)
    expect(mixOf(running({ kind: KIND_JEEP })).chop).toBe(0)
  })

  it('beats the blades harder when the helicopter’s collective is up', () => {
    const hover = mixOf(running({ kind: KIND_HELI, speed: 0, throttle: 0 }))
    const climbing = mixOf(running({ kind: KIND_HELI, speed: 0, throttle: 1 }))
    expect(climbing.chop).toBeGreaterThan(hover.chop)
    expect(climbing.load).toBeGreaterThan(hover.load)
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
    expect(mixOf(running({ powered: false, speed: 15, ground: 'gravel' })).ground).toBeGreaterThan(0)
    expect(mixOf(running({ powered: false, speed: 15, ground: 'gravel' })).grit.rate).toBeGreaterThan(0)
  })

  it('squeals a tank’s tracks turning on the spot, and clanks them harder on tarmac than in the snow', () => {
    const tank = (over: Partial<SoundInput>) => mixOf(running({ kind: KIND_TANK, ...over }))
    expect(tank({ speed: 0.5, yawRate: 0.8 }).squeal).toBeGreaterThan(tank({ speed: 0.5, yawRate: 0 }).squeal)
    expect(tank({ speed: 8, ground: 'tarmac' }).chopLevel).toBeGreaterThan(tank({ speed: 8, ground: 'snow' }).chopLevel)
    expect(tank({ speed: 8, ground: 'tarmac' }).chopTone).toBeGreaterThan(tank({ speed: 8, ground: 'snow' }).chopTone)
  })

  it('washes a hull through the water, and slaps it with a wave only now and then', () => {
    expect(mixOf(running({ kind: KIND_HELI, ground: 'water', speed: 6, grounded: false })).ground).toBeGreaterThan(0)
    for (const kind of [KIND_SUBMARINE, KIND_SEAPLANE, KIND_CB90]) {
      const afloat = mixOf(running({ kind, ground: 'water', speed: 5 }))
      expect(afloat.ground).toBeGreaterThan(0)
      expect(afloat.grit.rate).toBeLessThan(5)
      expect(afloat.squeal).toBe(0)
    }
  })

  it('hears no waves under the surface, only the water rushing past', () => {
    const dived = mixOf(running({ kind: KIND_SUBMARINE, ground: 'water', speed: 5, submerged: true }))
    expect(dived.grit.level).toBe(0)
    expect(dived.ground).toBeGreaterThan(0)
    expect(dived.groundHz).toBeLessThan(300)
  })

  it('gives a plane afloat the water under it, not the wind in its wires', () => {
    const afloat = mixOf(running({ kind: KIND_SEAPLANE, ground: 'water', speed: 4, grounded: true }))
    expect(afloat.groundHz).toBe(rolling('water', 4, 0, false).rollHz)
  })
})

describe('the ground', () => {
  it('hears each surface as what it is, and winter as snow and ice', () => {
    expect(groundOf(SURFACE_ASPHALT, false, false)).toBe('tarmac')
    expect(groundOf(SURFACE_DIRT, false, false)).toBe('gravel')
    expect(groundOf(SURFACE_GRASS, false, false)).toBe('grass')
    expect(groundOf(SURFACE_SAND, false, false)).toBe('sand')
    expect(groundOf(SURFACE_MUD, false, false)).toBe('mud')
    expect(groundOf(SURFACE_WATER, false, false)).toBe('water')
    expect(groundOf(SURFACE_ASPHALT, true, true)).toBe('tarmac')
    expect(groundOf(SURFACE_GRASS, true, true)).toBe('snow')
    expect(groundOf(SURFACE_MUD, true, true)).toBe('ice')
    expect(groundOf(SURFACE_WATER, true, true)).toBe('ice')
    expect(groundOf(SURFACE_WATER, true, false)).toBe('water')
  })

  it('sounds different on every ground', () => {
    const grounds: Ground[] = ['tarmac', 'gravel', 'grass', 'sand', 'mud', 'snow', 'ice', 'water']
    const heard = grounds.map((ground) => JSON.stringify(rolling(ground, 12, 0, false)))
    expect(new Set(heard).size).toBe(grounds.length)
  })

  it('crunches gravel in proportion to speed, more than tarmac rumbles', () => {
    expect(rolling('gravel', 20, 0, false).grit.rate).toBeGreaterThan(rolling('gravel', 5, 0, false).grit.rate)
    expect(rolling('gravel', 20, 0, false).roll).toBeGreaterThan(rolling('tarmac', 20, 0, false).roll)
    expect(rolling('tarmac', 20, 0, false).grit.level).toBe(0)
    // Spinning wheels churn more of it.
    expect(rolling('mud', 5, 0.8, false).grit.rate).toBeGreaterThan(rolling('mud', 5, 0, false).grit.rate)
  })

  it('squeals loudest on tarmac, less on gravel, and not on ice or in water', () => {
    const tarmac = squealGain(0.8, 'tarmac', 20)
    expect(squealGain(0.8, 'gravel', 20)).toBeLessThan(tarmac)
    expect(squealGain(0.8, 'gravel', 20)).toBeGreaterThan(0)
    expect(squealGain(0.8, 'ice', 20)).toBe(0)
    expect(squealGain(0.8, 'water', 20)).toBe(0)
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

describe('a plane’s rotary', () => {
  const flying = (over: Partial<SoundInput> = {}) => running({ kind: KIND_PLANE, grounded: false, accel: 0, throttle: 0, ...over })

  it('has no gears: its revs rise with the airspeed and the throttle', () => {
    let last = mixOf(flying({ speed: 0 })).rpm
    for (let v = 5; v <= 45; v += 5) {
      const now = mixOf(flying({ speed: v })).rpm
      expect(now).toBeGreaterThan(last)
      last = now
    }
    expect(mixOf(flying({ speed: 30, throttle: 1 })).rpm).toBeGreaterThan(mixOf(flying({ speed: 30, throttle: 0 })).rpm)
    expect(mixOf(flying({ speed: 35 })).chop).toBeGreaterThan(20)
  })

  it('cuts out and drops its revs on the blip switch', () => {
    const open = mixOf(flying({ speed: 35 }))
    const cut = mixOf(flying({ speed: 35, cut: true }))
    expect(cut.cut).toBe(true)
    expect(open.cut).toBe(false)
    expect(cut.load).toBe(0)
    expect(cut.rpm).toBeLessThan(open.rpm)
  })

  it('whistles in the wind in the air, louder the faster, even without a pilot', () => {
    expect(mixOf(flying({ speed: 40 })).ground).toBeGreaterThan(mixOf(flying({ speed: 20 })).ground)
    expect(mixOf(flying({ speed: 40, powered: false })).ground).toBeGreaterThan(0.2)
    expect(mixOf(running({ kind: KIND_PLANE, speed: 0, grounded: true })).ground).toBe(0)
  })
})
