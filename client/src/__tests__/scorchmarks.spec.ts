import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Effects, type GunScene } from '@/render/effects'
import { AUTOCANNON, CANNON } from '@/render/gunfire'
import type { ParticleSpec } from '@/render/particles'
import { Scorchmarks } from '@/render/scorchmarks'

const heights = (marks: Scorchmarks) => (marks.root.geometry.positions as Float32Array).filter((_, i) => i % 3 === 1)

describe('scorch marks', () => {
  it('lies on the slope of the ground under it, at every corner', () => {
    const slope = (x: number) => x * 0.5
    const marks = new Scorchmarks(slope, 8)
    marks.burn(10, 0, 0, 1)
    const positions = marks.root.geometry.positions
    for (let corner = 0; corner < 4; corner++) {
      const [x, y] = [positions[corner * 3]!, positions[corner * 3 + 1]!]
      expect(y).toBeCloseTo(slope(x) + 0.1, 3)
    }
  })

  it('stays as far above the ground as the hit was, as on a road', () => {
    const marks = new Scorchmarks(() => 2, 8)
    marks.sweep(null, { x: 0, y: 2.2, z: 0 }, 1)
    expect(heights(marks)[0]).toBeCloseTo(2.3, 3)
  })

  it('burns the whole way along a sweep, not just where the beam was seen', () => {
    const marks = new Scorchmarks(() => 0, 100)
    marks.sweep({ x: 0, y: 0, z: 0 }, { x: 8, y: 0, z: 0 }, 1)
    expect(marks.laid).toBe(21)
    const positions = marks.root.geometry.positions
    const middles = Array.from({ length: marks.laid }, (_, q) => (positions[q * 12]! + positions[q * 12 + 9]!) / 2)
    expect(Math.max(...middles.map((x, i) => (i ? x - middles[i - 1]! : 0)))).toBeLessThan(0.5)
    expect(middles[middles.length - 1]).toBeCloseTo(8, 3)
  })

  it('starts a new scar rather than drawing a line to a far hit', () => {
    const marks = new Scorchmarks(() => 0, 100)
    marks.sweep({ x: 0, y: 0, z: 0 }, { x: 90, y: 0, z: 0 }, 1)
    expect(marks.laid).toBe(1)
  })

  it('reuses the ring once it comes round', () => {
    const marks = new Scorchmarks(() => 0, 3)
    for (let i = 0; i < 7; i++) marks.burn(i, 0, 0, 1)
    expect(marks.laid).toBe(7)
    expect(marks.root.geometry.positions.length).toBe(3 * 4 * 3)
  })
})

describe('shot effects', () => {
  const pools = () => {
    const smoke: ParticleSpec[] = [], glow: ParticleSpec[] = [], blood: ParticleSpec[] = []
    const effects = new Effects({ smoke: { spawn: (s: ParticleSpec) => smoke.push(s) }, glow: { spawn: (s: ParticleSpec) => glow.push(s) }, blood: { spawn: (s: ParticleSpec) => blood.push(s) } }, 64)
    return { effects, smoke, glow, blood }
  }
  const at = { x: 1, y: 2, z: 3 }
  const ahead = { x: 0, y: 0, z: 1 }
  /** The world round a gun: the camera `far` metres from it, level ground at 0 and `alive` puffs of smoke in the air already. */
  const scene = (far = 10, alive = 0): GunScene => ({ camera: { x: at.x + far, y: at.y, z: at.z }, groundAt: () => 0, smoke: { alive } })

  // The effects scatter by `Math.random`: the same draws every run, so what is thrown is the same too.
  beforeEach(() => {
    let seed = 1
    vi.spyOn(Math, 'random').mockImplementation(() => (seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296)
  })
  afterEach(() => vi.restoreAllMocks())

  it('burns with billowing black smoke and flame, from the particle pools', () => {
    const { effects, smoke, glow } = pools()
    effects.laserBurn(at)
    expect(smoke.length).toBeGreaterThan(4)
    expect(smoke.every((s) => s.shape === 'puff')).toBe(true)
    expect(glow.some((s) => s.shape === 'glare')).toBe(true)
    expect(glow.some((s) => s.bounce)).toBe(true)
  })

  it('draws nothing round as a flash record: only streaks and rings go there', () => {
    const { effects } = pools()
    effects.gunfire(at, ahead, CANNON, 1, scene())
    effects.barrelSmoke(at, ahead, 1, 0.5, CANNON, scene())
    effects.blast(at, { x: 0, y: 0, z: 1 })
    effects.strike(at)
    effects.pelletStrike(at)
    effects.burst(at, 4)
    effects.dust(at, 2)
    effects.pierce(at, { x: 0, y: 1, z: 0 }, true)
    effects.deflect(at, { x: 0, y: 1, z: 0 })
    effects.impact(at, 'vehicle')
    effects.struck(at)
    effects.muzzle(at)
    expect(effects.root.count).toBe(0)
  })

  it('a tank’s gun throws more smoke than an autocannon’s, with sparks that bounce and dust off the ground under it', () => {
    const tank = pools()
    tank.effects.gunfire(at, ahead, CANNON, 1, scene())
    const auto = pools()
    auto.effects.gunfire(at, ahead, AUTOCANNON, 1, scene())
    const puffs = (smoke: ParticleSpec[]) => smoke.filter((s) => s.hold).length
    expect(puffs(tank.smoke)).toBeGreaterThan(puffs(auto.smoke) * 3)
    expect(tank.glow.some((s) => s.bounce && s.stretch)).toBe(true)
    // The muzzle is 2 m up: the cannon's blast reaches the ground, and dust has no hold on it.
    expect(tank.smoke.some((s) => !s.hold && s.color === 0xcbc2b0)).toBe(true)
    expect(auto.smoke.some((s) => s.color === 0xcbc2b0)).toBe(false)
  })

  it('a gun’s smoke rolls in the air and leans with the wind', () => {
    const { effects, smoke } = pools()
    effects.gunfire(at, ahead, CANNON, 1, scene())
    const bank = smoke.filter((s) => s.hold)
    expect(bank.length).toBeGreaterThan(10)
    expect(bank.every((s) => s.turbulence! > 0 && s.wind === 1 && s.shape === 'puff')).toBe(true)
  })

  it('a gun far from the camera flashes but makes no smoke', () => {
    const { effects, smoke, glow } = pools()
    effects.gunfire(at, ahead, CANNON, 1, scene(300))
    expect(smoke).toEqual([])
    expect(glow).toHaveLength(1)
    expect(glow[0]!.shape).toBe('glare')
  })

  it('a gun makes less smoke at a distance, and when the air is already full of it', () => {
    const count = (far: number, alive: number) => {
      const { effects, smoke } = pools()
      effects.gunfire(at, ahead, CANNON, 1, scene(far, alive))
      return smoke.filter((s) => s.hold).length
    }
    expect(count(100, 0)).toBeLessThan(count(10, 0))
    expect(count(10, 2000)).toBeLessThan(count(10, 0))
  })

  it('a rotary cannon that fired three rounds in a frame shows three', () => {
    const count = (fired: number) => {
      const { effects, smoke } = pools()
      effects.gunfire(at, ahead, AUTOCANNON, fired, scene())
      return smoke.filter((s) => s.hold).length
    }
    expect(count(3)).toBe(count(1) * 3)
    // A gap in the stream is not a burst: no more than three are drawn.
    expect(count(40)).toBe(count(3))
  })

  it('a gun that has fired smokes from its muzzle, and thins to nothing', () => {
    const wisps = (heat: number) => {
      const { effects, smoke } = pools()
      for (let frame = 0; frame < 60; frame++) effects.barrelSmoke(at, ahead, heat, 1 / 60, CANNON, scene())
      return smoke.filter((s) => s.hold)
    }
    const hot = wisps(1), cooling = wisps(0.3)
    expect(hot.length).toBeGreaterThan(8)
    expect(cooling.length).toBeLessThan(hot.length / 2)
    expect(cooling.length).toBeGreaterThan(0)
    expect(hot[0]!.alpha).toBeGreaterThan(cooling[0]!.alpha)
    // Far off it is not drawn at all.
    const { effects, smoke } = pools()
    effects.barrelSmoke(at, ahead, 1, 1, CANNON, scene(200))
    expect(smoke).toEqual([])
  })

  it('throws blood out of a body and sparks off a hull', () => {
    const body = pools()
    body.effects.impact(at, 'body')
    expect(body.blood.length).toBeGreaterThan(0)
    const hull = pools()
    hull.effects.impact(at, 'vehicle')
    expect(hull.blood.length).toBe(0)
  })
})
