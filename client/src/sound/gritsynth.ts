/**
 * The crunch of what a wheel, a track or a foot rolls over: short grains of filtered noise at random
 * moments, as many a second as stones are struck or snow crystals break, each a little louder or softer
 * than the last. Gravel is many short bright grains, snow fewer, squeakier ones, mud slow, low, wet ones
 * and water a slap. Like `EngineSynth`, pure numbers: `engine.worklet.ts` runs it on the audio thread.
 */

/** Grains a second, how loud, where their tone sits (Hz), how sharp it is (0.2 broad .. 1.8 ringing) and how long each lasts (ms). */
export interface Grit {
  rate: number
  level: number
  hz: number
  ring: number
  length: number
}

export const SILENT_GRIT: Grit = { rate: 0, level: 0, hz: 1000, ring: 0.6, length: 4 }

/** A small seeded random source (as `EngineSynth`'s). */
function random(seed: number) {
  let state = (seed * 2246822519) >>> 0 || 0x2545f491
  return () => {
    state ^= state << 13
    state ^= state >>> 17
    state ^= state << 5
    state >>>= 0
    return state / 4294967296
  }
}

export class GritSynth {
  private readonly random: () => number
  private envelope = 0
  private low = 0
  private band = 0

  constructor(
    private readonly sampleRate: number,
    seed = 7,
  ) {
    this.random = random(seed)
  }

  /** Fills `out` with grains as `grit` has them. */
  render(out: Float32Array, grit: Grit) {
    const { sampleRate } = this
    const chance = grit.rate / sampleRate
    const decay = Math.exp(-1 / ((Math.max(0.3, grit.length) / 1000) * sampleRate))
    const f = 2 * Math.sin((Math.PI * Math.min(grit.hz, sampleRate * 0.12)) / sampleRate)
    // Damping of the band-pass: the higher `ring`, the narrower and more tonal each grain.
    const damping = Math.max(0.12, 2 - grit.ring)
    for (let i = 0; i < out.length; i++) {
      if (chance > 0 && this.random() < chance) this.envelope = Math.max(this.envelope, 0.25 + 0.75 * this.random())
      this.envelope *= decay
      const white = this.random() * 2 - 1
      this.low += f * this.band
      const high = white - this.low - damping * this.band
      this.band += f * high
      const x = this.band * this.envelope * grit.level * 1.5
      out[i] = x / (1 + Math.abs(x))
    }
  }
}
