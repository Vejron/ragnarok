import { describe, expect, it } from 'vitest'
import init, { ClientSim } from '@/sim/wasm/sim'
import { boom, pointOfSail, wind } from '@/sim/wind'

// The client draws a ship's sail from its own copy of `crates/shared/src/wind.rs`; the sim (here built
// to WASM) sails her on the Rust one. They must be the same wind.
describe('wind', () => {
  it('matches the sim’s', async () => {
    await init()
    for (const [tick, fx, fz] of [[0, 0, -1], [12_345, 1, 0], [30_000, -0.6, 0.8], [61_439, 0.28, -0.96]] as const) {
      const [x, z, share, swing] = ClientSim.windAt(tick, fx, fz)
      const blowing = wind(tick)
      expect(blowing[0]).toBeCloseTo(x!, 4)
      expect(blowing[1]).toBeCloseTo(z!, 4)
      expect(pointOfSail([fx, fz], blowing)).toBeCloseTo(share!, 4)
      expect(boom([fx, fz], blowing)).toBeCloseTo(swing!, 4)
    }
  })

  it('drives a ship hardest on the beam', () => {
    const blowing: [number, number] = [1, 0]
    expect(pointOfSail([0, -1], blowing)).toBeCloseTo(1, 1)
    expect(pointOfSail([-1, 0], blowing)).toBeLessThan(0.1)
  })
})
