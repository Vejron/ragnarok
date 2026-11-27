import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Effects, type GunScene } from '@/render/effects'
import { AUTOCANNON, CANNON, PISTOL, RIFLE, SHOTGUN, SNIPER, smallArmsOf } from '@/render/gunfire'
import { ITEM_AK5, ITEM_SNIPER } from '@/sim/items'
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
    const smoke: ParticleSpec[] = [], glow: ParticleSpec[] = [], blood: ParticleSpec[] = [], halo: ParticleSpec[] = []
    const pool = (into: ParticleSpec[]) => ({ spawn: (s: ParticleSpec) => into.push(s) })
    const effects = new Effects({ smoke: pool(smoke), glow: pool(glow), blood: pool(blood), halo: pool(halo) }, 64)
    return { effects, smoke, glow, blood, halo }
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
    effects.barrelSmoke(at, ahead, 1, 0.5, CANNON, scene())
    effects.strike(at)
    effects.pelletStrike(at)
    effects.burst(at, 4)
    effects.dust(at, 2)
    effects.pierce(at, { x: 0, y: 1, z: 0 }, true)
    effects.deflect(at, { x: 0, y: 1, z: 0 })
    effects.impact(at, 'vehicle')
    effects.struck(at)
    expect(effects.root.count).toBe(0)
  })

  it('a muzzle flare is a glare with a cone of fire and a star of petals round it, at a new roll each shot', () => {
    const { effects, glow } = pools()
    effects.gunfire(at, ahead, RIFLE, 1, scene())
    expect(glow.filter((s) => s.shape === 'glare').length).toBeGreaterThanOrEqual(2)
    // Two streaks down the bore and two to each of four petals.
    expect(effects.root.count).toBe(10)
  })

  it('a flash brings a wider, fainter halo of its fire’s colour, its white heart none, and fire is its own light', () => {
    const { effects, smoke, glow, halo } = pools()
    effects.gunfire(at, ahead, RIFLE, 1, scene())
    effects.strike(at)
    effects.burst(at, 4)
    const flashes = glow.filter((s) => s.shape === 'glare')
    // The gun's flash and the blast's each have a white-hot heart inside them, which has no halo.
    expect(halo.length).toBe(flashes.length - 2)
    for (const h of halo) {
      const flash = flashes.find((f) => f.color === h.color && f.life < h.life)!
      expect(flash).toBeDefined()
      expect(h.shape).toBe('glare')
      expect(h.size[1]).toBeGreaterThan(flash.size[1] * 1.5)
      expect(h.glow!).toBeLessThan(flash.glow! / 2)
    }
    expect(halo.some((h) => h.color === 0xffffff)).toBe(false)
    const flames = smoke.filter((s) => (s.glow ?? 0) > 1)
    expect(flames.length).toBeGreaterThan(0)
    expect(flames.every((s) => s.emissive)).toBe(true)
  })

  it('a launcher’s backblast is a cone of fire with fireballs in it, and leaves smoke', () => {
    const { effects, smoke, glow } = pools()
    effects.backblast(at, { x: 0, y: 0, z: -1 })
    expect(effects.root.count).toBe(2)
    expect(smoke.filter((s) => (s.glow ?? 0) > 0).length).toBeGreaterThanOrEqual(10)
    expect(smoke.some((s) => s.hold)).toBe(true)
    expect(glow.some((s) => s.shape === 'glare')).toBe(true)
  })

  it('a shot down its own scope has no flare and only a wisp of its smoke', () => {
    const open = pools(), scoped = pools()
    open.effects.gunfire(at, ahead, SNIPER, 1, scene())
    scoped.effects.gunfire(at, ahead, SNIPER, 1, scene(), true)
    expect(scoped.effects.root.count).toBe(0)
    expect(scoped.glow.some((s) => s.shape === 'glare')).toBe(false)
    expect(scoped.halo.length).toBe(0)
    expect(open.halo.length).toBeGreaterThan(0)
    const puffs = (smoke: ParticleSpec[]) => smoke.filter((s) => s.hold).length
    expect(puffs(scoped.smoke)).toBeGreaterThan(0)
    expect(puffs(scoped.smoke)).toBeLessThan(puffs(open.smoke) / 2)
  })

  it('small arms smoke too, a sniper more than a rifle and a rifle a little against a cannon', () => {
    const puffs = (gun: typeof CANNON) => {
      const { effects, smoke } = pools()
      effects.gunfire(at, ahead, gun, 1, scene())
      return smoke.filter((s) => s.hold).length
    }
    for (const gun of [PISTOL, RIFLE, SNIPER, SHOTGUN]) expect(puffs(gun)).toBeGreaterThan(0)
    expect(puffs(SNIPER)).toBeGreaterThan(puffs(RIFLE))
    expect(puffs(CANNON)).toBeGreaterThan(puffs(SNIPER) * 2)
    expect(smallArmsOf(ITEM_SNIPER)).toBe(SNIPER)
    expect(smallArmsOf(ITEM_AK5)).toBe(RIFLE)
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
    expect(glow.length).toBeGreaterThan(0)
    expect(glow.every((s) => s.shape === 'glare')).toBe(true)
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
