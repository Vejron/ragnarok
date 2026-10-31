import { describe, expect, it } from 'vitest'
import { Effects } from '@/render/effects'
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
    effects.muzzleFlash(at, { x: 0, y: 0, z: 1 })
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

  it('throws blood out of a body and sparks off a hull', () => {
    const body = pools()
    body.effects.impact(at, 'body')
    expect(body.blood.length).toBeGreaterThan(0)
    const hull = pools()
    hull.effects.impact(at, 'vehicle')
    expect(hull.blood.length).toBe(0)
  })
})
