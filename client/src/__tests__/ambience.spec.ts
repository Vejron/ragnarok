import { describe, expect, it } from 'vitest'
import { Ambience, birdsong, callRates, mix, windOf, type Surround } from '@/ambience'
import { STRIKE_SLOT, strikeDistance, strikeIn } from '@/render/weather'
import { FakeContext } from './fakeaudio'

const surround = (over: Partial<Surround> = {}): Surround => ({
  at: [0, 0, 0], phase: 0.5, seconds: null, cover: 0.3, rain: 0, winter: false, forest: 0.5, water: 0, flowing: 0, height: 1.7, indoors: false, ...over,
})

/** A fixed draw, so every call comes the same way each run. */
const steady = () => 0.5

const ambience = () => {
  const ctx = new FakeContext()
  return { ctx, ambience: new Ambience(ctx as unknown as BaseAudioContext, steady) }
}

/** Runs `seconds` of updates at 60 a second, moving the context's clock along. */
const run = (ctx: FakeContext, a: Ambience, s: Surround, seconds: number) => {
  for (let i = 0; i < seconds * 60; i++) {
    ctx.currentTime += 1 / 60
    a.update(s, 1 / 60)
  }
}

describe('Ambience', () => {
  it('does nothing without a context', () => {
    const a = new Ambience(undefined, steady)
    expect(() => a.dispose()).not.toThrow()
  })

  it('has the birds sing at dawn, less at noon, and fall silent at night', () => {
    expect(birdsong(0.27)).toBeGreaterThan(birdsong(0.5))
    expect(birdsong(0.5)).toBeGreaterThan(0.2)
    expect(birdsong(0)).toBeLessThan(0.01)
    expect(callRates(surround({ phase: 0 })).bird).toBeLessThan(0.01)
    expect(callRates(surround({ phase: 0.27, forest: 1 })).bird).toBeGreaterThan(callRates(surround({ phase: 0.27, forest: 0 })).bird)
  })

  it('has the rain and the winter quieten the birds', () => {
    const fine = callRates(surround({ phase: 0.27 })).bird
    expect(callRates(surround({ phase: 0.27, rain: 1 })).bird).toBeLessThan(0.01)
    expect(callRates(surround({ phase: 0.27, winter: true })).bird).toBeLessThan(fine * 0.2)
  })

  it('brings crickets and owls out at night, and no crickets in winter', () => {
    const night = surround({ phase: 0, forest: 1 })
    expect(mix(night, 0.3).crickets).toBeGreaterThan(0)
    expect(mix(surround({ phase: 0.5 }), 0.3).crickets).toBeLessThan(0.001)
    expect(mix({ ...night, winter: true }, 0.3).crickets).toBe(0)
    expect(callRates(night).owl).toBeGreaterThan(0)
    expect(callRates(surround({ phase: 0.5, forest: 1 })).owl).toBeLessThan(0.001)
  })

  it('drums the rain on the roof indoors instead of hissing on the ground outside', () => {
    const out = mix(surround({ rain: 1 }), 0.5)
    const inside = mix(surround({ rain: 1, indoors: true }), 0.5)
    expect(out.rain).toBeGreaterThan(0)
    expect(out.roof).toBe(0)
    expect(inside.rain).toBe(0)
    expect(inside.roof).toBeGreaterThan(0)
  })

  it('blows harder in a storm and higher up, and rustles the leaves only among trees', () => {
    expect(windOf(surround({ rain: 1, cover: 1 }), 0.5)).toBeGreaterThan(windOf(surround(), 0.5))
    expect(windOf(surround({ height: 40 }), 0.5)).toBeGreaterThan(windOf(surround(), 0.5))
    expect(mix(surround({ forest: 0 }), 0.5).leaves).toBe(0)
    expect(mix(surround({ forest: 1 }), 0.5).leaves).toBeGreaterThan(0)
  })

  it('hears water only near it, and gulls only by still water', () => {
    expect(mix(surround({ water: 0 }), 0.5).water).toBe(0)
    expect(mix(surround({ water: 1 }), 0.5).water).toBeGreaterThan(0)
    expect(callRates(surround({ water: 1 })).gull).toBeGreaterThan(0)
    expect(callRates(surround({ water: 1, flowing: 1 })).gull).toBe(0)
  })

  it('runs only the beds that are heard, and stops one that has fallen silent', () => {
    const { ctx, ambience: a } = ambience()
    run(ctx, a, surround({ phase: 0.5, forest: 0, rain: 0 }), 1)
    const quiet = a.running
    expect(a.levels?.roof).toBe(0)
    run(ctx, a, surround({ rain: 1, indoors: true }), 1)
    expect(a.running).toBeGreaterThan(quiet)
    run(ctx, a, surround({ phase: 0.5, forest: 0, rain: 0 }), 7)
    expect(a.running).toBe(quiet)
  })

  it('sings birds in a wood at dawn', () => {
    const { ctx, ambience: a } = ambience()
    run(ctx, a, surround({ phase: 0.27, forest: 1 }), 10)
    expect(ctx.log.sources.filter((s) => s.kind === 'osc').length).toBeGreaterThan(10)
  })

  it('makes the thunder follow the flash by its distance, the same for everyone', () => {
    // A slot that strikes, and the moment just after its flash.
    let slot = 0
    while (strikeIn(slot) === null) slot++
    const strike = strikeIn(slot)!
    expect(strike).toBeGreaterThanOrEqual(slot * STRIKE_SLOT)
    const { ctx, ambience: a } = ambience()
    ctx.currentTime = 100
    a.update(surround({ rain: 1, seconds: strike + 0.1 }), 1 / 60)
    expect(a.thunders).toHaveLength(1)
    expect(a.thunders[0]!.distance).toBe(strikeDistance(slot))
    expect(a.thunders[0]!.at).toBeCloseTo(100 + strikeDistance(slot) / 343 - 0.1, 5)
    // Heard once, and never in rain too light for lightning.
    a.update(surround({ rain: 1, seconds: strike + 0.2 }), 1 / 60)
    expect(a.thunders).toHaveLength(1)
    const { ambience: calm } = ambience()
    calm.update(surround({ rain: 0.3, seconds: strike + 0.1 }), 1 / 60)
    expect(calm.thunders).toHaveLength(0)
  })
})
