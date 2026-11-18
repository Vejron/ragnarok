import { describe, expect, it } from 'vitest'
import init, { ClientSim } from '@/sim/wasm/sim'
import { CHURN_HEIGHT, churn, LOOP_TICKS, OPEN_SEA, shore, wave, waveTime } from '@/sim/waves'

// The client draws the waves from its own copy of `crates/shared/src/waves.rs`; the sim (here built
// to WASM) rides vehicles on the Rust one. They must be the same waves.
describe('waves', () => {
  it('match the sim’s', async () => {
    await init()
    for (const [x, z, time, reach] of [[0, 0, 0, OPEN_SEA], [3.7, -12.2, 21.4, 30], [-140, 88, 63.9, 180], [512, -301, 7.25, 260]] as const) {
      const [height, slopeX, slopeZ, rise] = ClientSim.waveAt(x, z, time, reach)
      const ours = wave(x, z, time, reach)
      expect(ours.height).toBeCloseTo(height!, 4)
      expect(ours.slopeX).toBeCloseTo(slopeX!, 4)
      expect(ours.slopeZ).toBeCloseTo(slopeZ!, 4)
      expect(ours.rise).toBeCloseTo(rise!, 4)
    }
  })

  it('stand in a rapid as the sim’s do, and lie flat on a calm river', async () => {
    await init()
    for (const [x, z, time, speed] of [[0, 0, 0, 4], [3.7, -12.2, 21.4, 2.6], [-140, 88, 63.9, 3.1], [512, -301, 7.25, 1.2]] as const) {
      const [height, slopeX, slopeZ, rise] = ClientSim.churnAt(x, z, time, speed)
      const ours = churn(x, z, time, speed)
      expect(ours.height).toBeCloseTo(height!, 4)
      expect(ours.slopeX).toBeCloseTo(slopeX!, 4)
      expect(ours.slopeZ).toBeCloseTo(slopeZ!, 4)
      expect(ours.rise).toBeCloseTo(rise!, 4)
    }
    expect(churn(3, 4, 10, 1).height).toBe(0)
    expect(Math.abs(churn(3.7, -12.2, 0, 4).height)).toBeLessThanOrEqual(CHURN_HEIGHT)
  })

  it('run on a looping clock', () => {
    expect(waveTime(LOOP_TICKS + 30)).toBeCloseTo(waveTime(30), 9)
    expect(waveTime(-30)).toBeCloseTo(waveTime(LOOP_TICKS - 30), 9)
    expect(wave(5, 9, waveTime(LOOP_TICKS), OPEN_SEA).height).toBeCloseTo(wave(5, 9, 0, OPEN_SEA).height, 6)
  })

  it('grow with the open water under them', () => {
    const crest = (reach: number) => Math.max(...Array.from({ length: 400 }, (_, k) => wave(5, 9, k * 0.16, reach).height))
    expect(crest(20)).toBeLessThan(0.05)
    expect(crest(200)).toBeGreaterThan(crest(20))
    expect(crest(OPEN_SEA)).toBeGreaterThan(crest(200))
    expect(wave(5, 9, 3, 2).height).toBe(0)
  })

  it('die away towards the shore', () => {
    expect(shore(0.1)).toBe(0)
    expect(shore(3)).toBe(1)
  })
})
