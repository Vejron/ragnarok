import { describe, expect, it } from 'vitest'
import init, { ClientSim } from '@/sim/wasm/sim'
import { CHURN_HEIGHT, churn, LOOP_TICKS, OPEN_SEA, particle, shore, wave, waveTime } from '@/sim/waves'

// The client draws the waves from its own copy of `crates/shared/src/waves.rs`; the sim (here built
// to WASM) rides vehicles on the Rust one. They must be the same waves.
describe('waves', () => {
  it('match the sim’s', async () => {
    await init()
    for (const [x, z, time, reach, share] of [[0, 0, 0, OPEN_SEA, 1], [3.7, -12.2, 21.4, 60, 1], [-140, 88, 63.9, 400, 0.6], [512, -301, 7.25, OPEN_SEA, 0.3]] as const) {
      const [height, slopeX, slopeZ, rise, orbitX, orbitZ] = ClientSim.waveAt(x, z, time, reach, share)
      const ours = wave(x, z, time, reach, share)
      expect(ours.height).toBeCloseTo(height!, 3)
      expect(ours.slopeX).toBeCloseTo(slopeX!, 3)
      expect(ours.slopeZ).toBeCloseTo(slopeZ!, 3)
      expect(ours.rise).toBeCloseTo(rise!, 3)
      expect(ours.orbitX).toBeCloseTo(orbitX!, 3)
      expect(ours.orbitZ).toBeCloseTo(orbitZ!, 3)
      const [shiftX, shiftZ, lifted, squeeze] = ClientSim.particleAt(x, z, time, reach, share)
      const moved = particle(x, z, time, reach, share)
      expect(moved.shiftX).toBeCloseTo(shiftX!, 3)
      expect(moved.shiftZ).toBeCloseTo(shiftZ!, 3)
      expect(moved.height).toBeCloseTo(lifted!, 3)
      expect(moved.squeeze).toBeCloseTo(squeeze!, 3)
    }
  })

  it('carry the water at rest under a point there, at the height the sim rides', () => {
    // The water shader moves each vertex as `particle` does; the sim asks what stands over a point.
    const [x, z, time] = [37.5, -12.2, 21.4]
    const over = wave(x, z, time, OPEN_SEA, 1)
    let [x0, z0] = [x, z]
    for (let i = 0; i < 12; i++) {
      const moved = particle(x0, z0, time, OPEN_SEA, 1)
      ;[x0, z0] = [x - moved.shiftX, z - moved.shiftZ]
    }
    const moved = particle(x0, z0, time, OPEN_SEA, 1)
    expect(x0 + moved.shiftX).toBeCloseTo(x, 3)
    expect(moved.height).toBeCloseTo(over.height, 3)
  })

  it('surge in a rapid as the sim’s do, across the current, and lie flat on a calm river', async () => {
    await init()
    for (const [x, z, time, flowX, flowZ] of [[0, 0, 0, 4, 0], [3.7, -12.2, 21.4, 1.6, -2.1], [-140, 88, 63.9, -2.2, 2.2], [512, -301, 7.25, 0.3, 1.5]] as const) {
      const [height, slopeX, slopeZ, rise] = ClientSim.churnAt(x, z, time, flowX, flowZ)
      const ours = churn(x, z, time, flowX, flowZ)
      expect(ours.height).toBeCloseTo(height!, 4)
      expect(ours.slopeX).toBeCloseTo(slopeX!, 4)
      expect(ours.slopeZ).toBeCloseTo(slopeZ!, 4)
      expect(ours.rise).toBeCloseTo(rise!, 4)
    }
    expect(churn(3, 4, 10, 1, 0).height).toBe(0)
    expect(Math.abs(churn(3.7, -12.2, 0, 4, 0).height)).toBeLessThanOrEqual(CHURN_HEIGHT)
  })

  it('run on a looping clock', () => {
    expect(waveTime(LOOP_TICKS + 30)).toBeCloseTo(waveTime(30), 9)
    expect(waveTime(-30)).toBeCloseTo(waveTime(LOOP_TICKS - 30), 9)
    expect(wave(5, 9, waveTime(LOOP_TICKS), OPEN_SEA, 1).height).toBeCloseTo(wave(5, 9, 0, OPEN_SEA, 1).height, 5)
  })

  it('grow with the open water under them', () => {
    const crest = (reach: number) => Math.max(...Array.from({ length: 400 }, (_, k) => wave(5, 9, k * 0.16, reach, 1).height))
    expect(crest(20)).toBeLessThan(0.05)
    expect(crest(200)).toBeGreaterThan(crest(20))
    expect(crest(OPEN_SEA)).toBeGreaterThan(crest(200))
    expect(wave(5, 9, 3, 2, 1).height).toBe(0)
    expect(wave(5, 9, 3, OPEN_SEA, 0).height).toBe(0)
  })

  it('die away towards the shore', () => {
    expect(shore(0.1)).toBe(0)
    expect(shore(3)).toBe(1)
  })
})
