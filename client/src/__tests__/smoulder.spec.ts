import { describe, expect, it } from 'vitest'
import { SMOULDER_MAX, SMOULDER_MIN_RADIUS, SMOULDER_SECONDS, Smoulders } from '@/render/smoulder'

/** Counts the wisps let out. */
function sink() {
  const spawned: unknown[] = []
  return { spawned, smoke: { spawn: (spec: unknown) => void spawned.push(spec) } }
}

describe('Smoulders', () => {
  it('smoke thickly at first, then seldom, then not at all', () => {
    const smoulders = new Smoulders()
    const { spawned, smoke } = sink()
    smoulders.add(0, 0, 0, 6, 0)
    const wisps = (from: number, to: number) => {
      const before = spawned.length
      for (let t = from; t < to; t += 1 / 60) smoulders.update(t, smoke)
      return spawned.length - before
    }
    const early = wisps(0, 5)
    const late = wisps(SMOULDER_SECONDS - 5, SMOULDER_SECONDS - 0.01)
    expect(early).toBeGreaterThan(late * 2)
    expect(late).toBeGreaterThan(0)
    expect(wisps(SMOULDER_SECONDS, SMOULDER_SECONDS + 5)).toBe(0)
    expect(smoulders.count).toBe(0)
  })

  it('leave small craters cold', () => {
    const smoulders = new Smoulders()
    smoulders.add(0, 0, 0, SMOULDER_MIN_RADIUS - 0.5, 0)
    expect(smoulders.count).toBe(0)
  })

  it('keep only the newest craters smoking in a barrage', () => {
    const smoulders = new Smoulders()
    for (let i = 0; i < SMOULDER_MAX + 10; i++) smoulders.add(i, 0, 0, 6, 0)
    expect(smoulders.count).toBe(SMOULDER_MAX)
  })
})
