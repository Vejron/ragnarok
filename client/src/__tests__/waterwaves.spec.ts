import { describe, expect, it } from 'vitest'
import { MOST_STAMPS, Vec3, WaveField } from '@ragnarok/engine'
import { WAVE_GRID, WAVE_TEXEL, WaterWaves } from '@/render/waterwaves'
import { JetSpray } from '@/render/jetspray'
import { Particles } from '@/render/particles'
import { Skidmarks } from '@/render/skidmarks'
import { VehicleFx, type FxWorld } from '@/render/tyres'
import { KIND_CB90, KIND_SUBMARINE } from '@/net/codec'
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
  it('cover 96 m about the camera', () => {
    expect(WAVE_GRID * WAVE_TEXEL).toBe(96)
  })

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

describe('a boat in the waves', () => {
  const last = <T>(list: T[]) => list[list.length - 1]!
  /** A Stridsbåt 90 heading -Z from `speed` m/s, changing at `accel` m/s², for half a second: the hulls it pressed in and the splashes its drops made. */
  function run(speed: number, accel: number) {
    const hulls: { depth: number; churn: number; ahead: readonly [number, number]; velocity: readonly [number, number] }[] = []
    const pokes: number[] = []
    const stir = { hull: (_key: string, s: (typeof hulls)[number]) => hulls.push(s), splash: (_x: number, _z: number, r: number) => pokes.push(r) }
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
    const fx = new VehicleFx(1, KIND_CB90)
    const dt = 1 / 60
    for (let i = 0; i < 90; i++) {
      const t = i * dt
      fx.update(w, [0, 0.2, -(speed * t + 0.5 * accel * t * t)], [0, 0, 0, 1], 0, true, dt)
      jetSpray.update(dt)
    }
    return { hulls, pokes }
  }

  it('presses nothing in lying still, its hull along its heading under way, and digs in harder in a crash stop', () => {
    expect(run(0, 0).hulls).toHaveLength(0)
    const cruising = last(run(20, 0).hulls)
    expect(cruising.ahead[0]).toBeCloseTo(0, 6)
    expect(cruising.ahead[1]).toBeCloseTo(-1, 6)
    expect(cruising.depth).toBeGreaterThan(0)
    expect(cruising.velocity[1]).toBeCloseTo(-20, 0)
    const stopping = last(run(20, -10).hulls)
    expect(stopping.depth).toBeGreaterThan(cruising.depth)
    expect(stopping.churn).toBeGreaterThan(cruising.churn)
  })

  it('stirs the water where its spray comes down', () => {
    expect(run(0, 0).pokes).toHaveLength(0)
    expect(run(20, 0).pokes.length).toBeGreaterThan(3)
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
