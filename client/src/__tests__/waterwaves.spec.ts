import { describe, expect, it } from 'vitest'
import { MOST_STAMPS, Vec3, WaveField } from '@ragnarok/engine'
import { WAVE_TEXEL, WaterWaves } from '@/render/waterwaves'
import { JetSpray } from '@/render/jetspray'
import { Particles } from '@/render/particles'
import { Skidmarks } from '@/render/skidmarks'
import { VehicleFx, type FxWorld } from '@/render/tyres'
import { KIND_CB90, KIND_RAFT, KIND_SUBMARINE } from '@/net/codec'
import { SURFACE_WATER } from '@/sim/surface'

const hull = (x: number, z: number, depth = 0.3) => ({ x, z, ahead: [0, -1] as const, halfWidth: 1.9, halfLength: 7.95, depth, velocity: [0, -20] as const, churn: 1 })

describe('the wave field', () => {
  it('steps at sixty a second whatever the frame rate, catching up at most four after a stall', () => {
    const field = new WaveField()
    field.stamp(hull(0, 0))
    field.update(1 / 120, 0, 0)
    expect(field.pending).toBe(0)
    field.update(1 / 120, 0, 0)
    expect(field.pending).toBe(1)
    field.update(1 / 30, 0, 0)
    expect(field.pending).toBe(2)
    field.update(2, 0, 0)
    expect(field.pending).toBe(4)
  })

  it('follows the camera on whole texels, and only in a frame that steps', () => {
    const field = new WaveField({ grid: 64, texel: 0.5 })
    field.stamp(hull(100, 0))
    field.update(1 / 60, 100.3, -7.1)
    expect(field.area[0]).toBeCloseTo(100 - 16, 6)
    expect(field.area[1]).toBeCloseTo(-7.5 - 16, 6)
    expect(field.area[2]).toBe(32)
    field.update(1 / 240, 200, 0)
    expect(field.area[0]).toBeCloseTo(100 - 16, 6)
  })

  it('rests once nothing has stirred it for a while', () => {
    const field = new WaveField()
    field.stamp(hull(0, 0))
    field.update(1 / 60, 0, 0)
    for (let i = 0; i < 60 * 9; i++) field.update(1 / 60, 0, 0)
    expect(field.pending).toBe(1)
    for (let i = 0; i < 60 * 2; i++) field.update(1 / 60, 0, 0)
    expect(field.pending).toBe(0)
  })

  it('keeps the splashes of a frame that takes no step for the next that does, but not its hulls', () => {
    const field = new WaveField()
    field.update(1 / 60, 0, 0)
    field.splash(0, 0, 1, 0.2, 1)
    field.stamp(hull(0, 0))
    field.update(1 / 120, 0, 0)
    expect(field.pressing).toBe(0)
    field.stamp(hull(1, 0))
    field.update(1 / 120, 0, 0)
    // The splash, and the hull where it is now; not the hull where it was a frame before.
    expect(field.pressing).toBe(2)
  })

  it('takes at most its share of stamps a frame', () => {
    const field = new WaveField()
    for (let i = 0; i < MOST_STAMPS + 10; i++) field.stamp(hull(i, 0))
    expect(field.stamped).toBe(MOST_STAMPS)
  })

  it('will not run waves faster than its grid can carry them', () => {
    expect(() => new WaveField({ texel: 0.1, speed: 6 })).toThrow()
    expect(() => new WaveField({ texel: WAVE_TEXEL, speed: 6 })).not.toThrow()
  })
})

describe('the water waves', () => {
  it('keep the nearest and biggest stirs when more come than the field takes, and ignore far ones', () => {
    const waves = new WaterWaves()
    waves.update(1 / 60, { x: 0, y: 0, z: 0 })
    waves.hull('far', hull(500, 0))
    expect(waves.waiting).toBe(0)
    for (let i = 0; i < MOST_STAMPS; i++) waves.splash(40, 0, 0.3, 0.05, 0)
    waves.hull('near', hull(5, 0, 0.5))
    waves.update(1 / 60, { x: 0, y: 0, z: 0 })
    expect(waves.field.pressing).toBe(MOST_STAMPS)
  })
})

describe('a hull in the water waves', () => {
  const eye = { x: 0, y: 0, z: 0 }
  const moving = (speed: number, depth = 0.3) => ({ ...hull(0, 0, depth), velocity: [0, -speed] as const })

  it('pushes the water as the square of its speed against the ripples, fully from their speed on', () => {
    const waves = new WaterWaves()
    waves.hull('swimmer', { ...moving(1.2, 0.25), halfWidth: 0.3, halfLength: 0.3 })
    waves.hull('boat', moving(20))
    waves.update(1 / 60, eye)
    expect(waves.pushing('swimmer')).toBeCloseTo(0.25 * (1.2 / 3) ** 2, 4)
    expect(waves.pushing('boat')).toBeCloseTo(0.3, 6)
  })

  it('on a river pushes and churns by its speed through the water, not over the ground', () => {
    // A current of 4 m/s towards -Z.
    const waves = new WaterWaves(() => [0, -4])
    waves.hull('drifting', moving(4))
    waves.hull('upstream', { ...moving(-1), x: 2 })
    waves.update(1 / 60, eye)
    expect(waves.pushing('drifting')).toBe(0)
    // Driven up the rapid at 5 m/s through it, it gains 1 m/s over the ground and pushes fully.
    expect(waves.pushing('upstream')).toBeCloseTo(0.3, 6)
  })

  it('counts a hull stamped twice in a frame once, and lifts it out where it last sat once it is gone', () => {
    const waves = new WaterWaves()
    waves.hull('boat', moving(20))
    waves.hull('boat', moving(20))
    waves.update(1 / 60, eye)
    expect(waves.field.pressing).toBe(1)
    waves.update(1 / 60, eye)
    expect(waves.field.pressing).toBe(1)
    expect(waves.pushing('boat')).toBe(0)
    waves.update(1 / 60, eye)
    expect(waves.field.pressing).toBe(0)
  })
})

describe('a river in the water waves', () => {
  const eye = { x: 0, y: 0, z: 0 }
  const rock = { x: 4, z: -6, depth: 0.3, flow: [3, 0] as const }

  it('has its boulders push the water that runs onto them, standing still, the nearest first', () => {
    const far = { x: 30, z: 0, depth: 0.2, flow: [3, 0] as const }
    const waves = new WaterWaves(() => [3, 0], () => [far, rock])
    waves.update(1 / 60, eye)
    expect(waves.rocky).toBe(2)
    expect(waves.field.pressing).toBe(2)
    const pressed = (waves.field as unknown as { ready: Float32Array }).ready
    // The nearer one first: where it is, as a round body, pushing the water at rest over the ground.
    expect([pressed[0], pressed[1]]).toEqual([4, -6])
    expect(pressed[4]).toBe(pressed[5])
    expect(pressed[6]).toBeGreaterThan(0.1)
    expect([pressed[8], pressed[9]]).toEqual([0, 0])
  })

  it('hands the field the current to carry its water along, and none on still water', () => {
    const river = new WaterWaves(() => [2, -1], () => [])
    river.update(1 / 60, eye)
    const flow = (river.field as unknown as { flow: { bytes: Uint8Array; area: number[]; step: number } | null }).flow!
    expect([flow.bytes[0], flow.bytes[1]]).toEqual([128 + 40, 128 - 20])
    expect(flow.area[2]).toBeGreaterThan(96)
    expect(flow.area[0]).toBeLessThanOrEqual(-48)
    const lake = new WaterWaves()
    lake.update(1 / 60, eye)
    expect((lake.field as unknown as { flow: unknown }).flow).toBeNull()
    expect(lake.field.pressing).toBe(0)
  })

  it('lets boats have the stamps first when there are more of both than the field takes', () => {
    const rocks = Array.from({ length: 30 }, (_, k) => ({ x: k, z: 0, depth: 0.2, flow: [3, 0] as const }))
    const waves = new WaterWaves(() => [0, 0], () => rocks)
    for (let k = 0; k < 60; k++) waves.hull(`boat ${k}`, hull(k % 10, -k % 7))
    waves.update(1 / 60, eye)
    expect(waves.field.pressing).toBe(64)
  })
})

describe('a boat in the waves', () => {
  const last = <T>(list: T[]) => list[list.length - 1]!
  type Pressed = { x: number; z: number; depth: number; churn: number; ahead: readonly [number, number]; velocity: readonly [number, number] }
  /**
   * A boat of `kind` (a Stridsbåt 90 unless said) heading -Z from `speed` m/s, changing at `accel` m/s²,
   * for `frames` sixtieths of a second: the hulls it pressed in, the white wash behind it, and the
   * splashes its drops made.
   */
  function run(speed: number, accel: number, kind = KIND_CB90, frames = 90) {
    const hulls: Pressed[] = []
    const washes: Pressed[] = []
    const pokes: number[] = []
    const stir = {
      hull: (key: string, s: Pressed) => (key.endsWith('wash') ? washes : hulls).push(s),
      splash: (_x: number, _z: number, r: number) => pokes.push(r),
    }
    const glow = new Particles(2000, true)
    const jetSpray = new JetSpray(glow, stir)
    const w: FxWorld = {
      smoke: new Particles(4000),
      glow,
      skids: new Skidmarks(500),
      groundAt: () => -5,
      surfaceAt: () => SURFACE_WATER,
      waterAt: () => 0,
      camera: new Vec3(0, 5, 10),
      waves: stir,
      jetSpray,
    }
    const fx = new VehicleFx(1, kind)
    const dt = 1 / 60
    let z = 0
    for (let i = 0; i < frames; i++) {
      const t = i * dt
      z = -(speed * t + 0.5 * accel * t * t)
      fx.update(w, [0, 0.2, z], [0, 0, 0, 1], 0, true, dt)
      jetSpray.update(dt)
    }
    return { hulls, washes, pokes, z }
  }

  it('presses nothing in lying still, and its hull along its heading under way', () => {
    expect(run(0, 0).hulls).toHaveLength(0)
    const cruising = last(run(20, 0).hulls)
    expect(cruising.ahead[0]).toBeCloseTo(0, 6)
    expect(cruising.ahead[1]).toBeCloseTo(-1, 6)
    expect(cruising.depth).toBeGreaterThan(0)
    expect(cruising.velocity[1]).toBeCloseTo(-20, 0)
  })

  it.each([
    ['Stridsbåt 90', KIND_CB90, 20],
    ['raft', KIND_RAFT, 12],
  ])('leaves a white wash behind the %s\'s transom under way, and none lying still', (_, kind, speed) => {
    expect(run(0, 0, kind).washes).toHaveLength(0)
    const { washes, hulls, z } = run(speed, 0, kind)
    const wash = last(washes)
    // Behind it, and churned harder than its hull churns the water.
    expect(wash.z).toBeGreaterThan(z)
    expect(wash.churn).toBeGreaterThan(last(hulls).churn)
  })

  it('stirs the water where its spray comes down', () => {
    expect(run(0, 0).pokes).toHaveLength(0)
    // The rooster tail's drops are nearly two seconds in the air.
    expect(run(20, 0, KIND_CB90, 180).pokes.length).toBeGreaterThan(3)
  })

  it('leaves the surface still above a dived submarine, and pushes it surfaced', () => {
    const depthAt = (y: number) => {
      const hulls: number[] = []
      const stir = { hull: (_key: string, st: { depth: number }) => hulls.push(st.depth), splash: () => {} }
      const w: FxWorld = { smoke: new Particles(100), glow: new Particles(100, true), skids: new Skidmarks(10), groundAt: () => -40, surfaceAt: () => SURFACE_WATER, waterAt: () => 0, camera: new Vec3(0, 5, 10), waves: stir }
      const fx = new VehicleFx(2, KIND_SUBMARINE)
      for (let i = 0; i < 30; i++) fx.update(w, [0, y, -i * 8 / 60], [0, 0, 0, 1], 0, true, 1 / 60)
      return hulls.length ? hulls[hulls.length - 1]! : 0
    }
    expect(depthAt(-10)).toBe(0)
    expect(depthAt(0.5)).toBeGreaterThan(0.1)
  })

  it('carried along at a river\'s speed carves no wake, and driven against it does', () => {
    const wakes = (current: readonly [number, number]) => {
      const hulls: number[] = []
      const stir = { hull: (_key: string, st: { depth: number }) => hulls.push(st.depth), splash: () => {} }
      const w: FxWorld = { smoke: new Particles(100), glow: new Particles(100, true), skids: new Skidmarks(10), groundAt: () => -40, surfaceAt: () => SURFACE_WATER, waterAt: () => 0, currentAt: () => current, camera: new Vec3(0, 5, 10), waves: stir }
      const fx = new VehicleFx(3, KIND_SUBMARINE)
      // Over the ground it goes 3 m/s towards -Z.
      // The first frames, before its velocity is known, are not counted.
      for (let i = 0; i < 30; i++) {
        if (i === 5) hulls.length = 0
        fx.update(w, [0, 0.5, -i * 3 / 60], [0, 0, 0, 1], 0, true, 1 / 60)
      }
      return hulls.length
    }
    expect(wakes([0, -3])).toBe(0)
    expect(wakes([0, 2])).toBeGreaterThan(0)
  })
})
