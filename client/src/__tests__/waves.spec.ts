import { describe, expect, it } from 'vitest'
import init, { ClientSim } from '@/sim/wasm/sim'
import { LOOP_TICKS, shore, wave, waveTime } from '@/sim/waves'

// The client draws the waves from its own copy of `crates/shared/src/waves.rs`; the sim (here built
// to WASM) rides vehicles on the Rust one. They must be the same waves.
describe('waves', () => {
  it('match the sim’s', async () => {
    await init()
    for (const [x, z, time] of [[0, 0, 0], [3.7, -12.2, 21.4], [-140, 88, 63.9], [512, -301, 7.25]] as const) {
      const [height, slopeX, slopeZ, rise] = ClientSim.waveAt(x, z, time)
      const ours = wave(x, z, time)
      expect(ours.height).toBeCloseTo(height!, 4)
      expect(ours.slopeX).toBeCloseTo(slopeX!, 4)
      expect(ours.slopeZ).toBeCloseTo(slopeZ!, 4)
      expect(ours.rise).toBeCloseTo(rise!, 4)
    }
  })

  it('run on a looping clock', () => {
    expect(waveTime(LOOP_TICKS + 30)).toBeCloseTo(waveTime(30), 9)
    expect(waveTime(-30)).toBeCloseTo(waveTime(LOOP_TICKS - 30), 9)
    expect(wave(5, 9, waveTime(LOOP_TICKS)).height).toBeCloseTo(wave(5, 9, 0).height, 6)
  })

  it('die away towards the shore', () => {
    expect(shore(0.1)).toBe(0)
    expect(shore(3)).toBe(1)
  })
})
