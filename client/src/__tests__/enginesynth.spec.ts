import { describe, expect, it } from 'vitest'
import { EngineSynth, type SynthSpec } from '@/sound/enginesynth'
import { GritSynth } from '@/sound/gritsynth'
import { DIESEL_FOUR, METEOR_V12, PETROL_V8, ROCKET, TURBOSHAFT, TWO_STROKE_TWIN } from '@/kinds/engines'

const RATE = 48000

/** `seconds` of `spec` at `rpm` and `load`, in 128-sample blocks as the worklet renders, after a second to settle. */
function render(spec: SynthSpec, redline: number, rpm: number, load: number, seconds = 0.5, seed = 1) {
  const synth = new EngineSynth(spec, redline, RATE, seed)
  const block = new Float32Array(128)
  for (let i = 0; i < RATE / 128; i++) synth.render(block, rpm, load)
  const out = new Float32Array(Math.round((RATE * seconds) / 128) * 128)
  for (let at = 0; at < out.length; at += 128) {
    synth.render(block, rpm, load)
    out.set(block, at)
  }
  return out
}

/** The lag, in samples, within `[low, high]` at which `x` best matches itself. */
function period(x: Float32Array, low: number, high: number) {
  let best = low
  let most = -Infinity
  for (let lag = low; lag <= high; lag++) {
    let sum = 0
    for (let i = 0; i + lag < x.length; i++) sum += x[i]! * x[i + lag]!
    if (sum > most) {
      most = sum
      best = lag
    }
  }
  return best
}

/** Where the energy of `x` sits, Hz: the centroid of its spectrum up to 6 kHz. */
function centroid(x: Float32Array) {
  const n = 4096
  let weighted = 0
  let total = 0
  for (let bin = 1; bin * (RATE / n) < 6000; bin++) {
    let re = 0
    let im = 0
    for (let i = 0; i < n; i++) {
      const angle = (2 * Math.PI * bin * i) / n
      re += x[i]! * Math.cos(angle)
      im -= x[i]! * Math.sin(angle)
    }
    const power = re * re + im * im
    weighted += power * bin * (RATE / n)
    total += power
  }
  return weighted / total
}

const rms = (x: Float32Array) => Math.sqrt(x.reduce((sum, v) => sum + v * v, 0) / x.length)

describe('EngineSynth', () => {
  it('fires at the rate its cylinders and revs give', () => {
    // An even V12 four-stroke at 1800 rpm fires 30 × 12 / 2 = 180 times a second; a twin two-stroke at
    // 6000 rpm, 100 × 2 = 200.
    for (const [spec, rpm, hz] of [
      [{ ...METEOR_V12, imbalance: 0, jitter: 0 }, 1800, 180],
      [{ ...TWO_STROKE_TWIN, imbalance: 0, jitter: 0 }, 6000, 200],
      [{ ...DIESEL_FOUR, imbalance: 0, jitter: 0 }, 2400, 80],
    ] as const) {
      const x = render(spec, 8000, rpm, 0.7)
      const expected = RATE / hz
      expect(Math.abs(period(x, Math.round(expected * 0.6), Math.round(expected * 1.4)) - expected) / expected).toBeLessThan(0.04)
    }
  })

  it('sounds like itself: a V12 low, a two-stroke twin bright', () => {
    const tank = centroid(render(METEOR_V12, 2550, 1600, 0.8))
    const sled = centroid(render(TWO_STROKE_TWIN, 8200, 7000, 0.8))
    expect(sled).toBeGreaterThan(tank * 1.5)
  })

  it('is louder pulling than coasting', () => {
    expect(rms(render(PETROL_V8, 7200, 4000, 1))).toBeGreaterThan(rms(render(PETROL_V8, 7200, 4000, 0.1)) * 1.3)
  })

  it('is the same every time for a seed, and different for another', () => {
    expect(render(PETROL_V8, 7200, 3000, 0.5, 0.1, 3)).toEqual(render(PETROL_V8, 7200, 3000, 0.5, 0.1, 3))
    expect(render(PETROL_V8, 7200, 3000, 0.5, 0.1, 3)).not.toEqual(render(PETROL_V8, 7200, 3000, 0.5, 0.1, 4))
  })

  it('never clips or breaks, from standstill to past the redline, on and off the throttle', () => {
    for (const [spec, redline] of [[METEOR_V12, 2550], [PETROL_V8, 7200], [TWO_STROKE_TWIN, 8200], [TURBOSHAFT, 30000], [ROCKET, 6000]] as const) {
      for (const rpm of [0, redline * 0.3, redline * 1.2])
        for (const load of [0, 1]) {
          const x = render(spec, redline, rpm, load, 0.1)
          expect(x.every((v) => Number.isFinite(v) && Math.abs(v) < 1)).toBe(true)
        }
    }
    // Stopped, it is silent.
    expect(rms(render(METEOR_V12, 2550, 0, 0, 0.1))).toBeLessThan(1e-3)
  })

  it('pops on the overrun when its pipe crackles', () => {
    // Off the throttle high in the revs, a crackling V8 throws louder peaks than one whose pipe does not.
    const peaks = (x: Float32Array) => x.filter((v) => Math.abs(v) > 0.6).length
    expect(peaks(render(PETROL_V8, 7200, 6000, 0, 1))).toBeGreaterThan(peaks(render({ ...PETROL_V8, crackle: 0 }, 7200, 6000, 0, 1)) * 2 + 10)
  })
})

describe('GritSynth', () => {
  it('is silent with no grains and crunches with them', () => {
    const synth = new GritSynth(RATE)
    const out = new Float32Array(RATE / 4)
    synth.render(out, { rate: 0, level: 1, hz: 2000, ring: 0.6, length: 3 })
    expect(rms(out)).toBeLessThan(1e-4)
    synth.render(out, { rate: 300, level: 0.6, hz: 2600, ring: 0.6, length: 3 })
    expect(rms(out)).toBeGreaterThan(0.01)
    expect(out.every((v) => Number.isFinite(v) && Math.abs(v) < 1)).toBe(true)
  })

  it('stays stable at the edges of its range', () => {
    const synth = new GritSynth(RATE)
    const out = new Float32Array(RATE / 4)
    synth.render(out, { rate: 2000, level: 1, hz: 20000, ring: 2, length: 50 })
    expect(out.every((v) => Number.isFinite(v) && Math.abs(v) < 1)).toBe(true)
  })
})
