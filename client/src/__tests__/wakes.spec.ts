import { describe, expect, it } from 'vitest'
import { WAKE_GRID, WAKE_TEXEL, WakeTrail, Wakes } from '@/render/wakes'

const camera = { x: 0, y: 0, z: 0 }

describe('wakes', () => {
  it('dig a trench where the hull was, then send a ring out from it', () => {
    const wakes = new Wakes()
    wakes.drop(2, 3, 0.25)
    wakes.update(1 / 60, camera)
    const [dug, , , foam] = wakes.sample(2, 3)
    // The ring just leaving it fills some of it back in.
    expect(dug).toBeLessThan(-0.04)
    expect(foam).toBeGreaterThan(0.5)
    // A second on, the ring has run out about 3.8 m and stands there, the trench all but filled in.
    for (let i = 0; i < 60; i++) wakes.update(1 / 60, camera)
    expect(Math.abs(wakes.sample(2, 3)[0])).toBeLessThan(0.02)
    expect(wakes.sample(2 + 3.8, 3)[0]).toBeGreaterThan(0.04)
    expect(Math.abs(wakes.sample(2 + 12, 3)[0])).toBeLessThan(0.01)
  })

  it('give the slope away from a ring’s crest', () => {
    const wakes = new Wakes()
    wakes.drop(0, 0, 0.25)
    for (let i = 0; i < 60; i++) wakes.update(1 / 60, camera)
    const crest = 3.8
    // Outside the crest the ground falls away along +X; inside it rises towards it.
    expect(wakes.sample(crest + 0.4, 0)[1]).toBeLessThan(0)
    expect(wakes.sample(crest - 0.4, 0)[1]).toBeGreaterThan(0)
  })

  it('let a source go once it has died away, and leave a still map alone', () => {
    const wakes = new Wakes()
    wakes.drop(0, 0, 0.2)
    expect(wakes.update(0.1, camera)).toBe(true)
    expect(wakes.update(10, camera)).toBe(true)
    expect(wakes.count).toBe(0)
    expect(wakes.sample(0, 0)[0]).toBeCloseTo(0, 2)
    expect(wakes.update(0.1, camera)).toBe(false)
  })

  it('follow the camera on whole texels', () => {
    const wakes = new Wakes()
    wakes.drop(100, 0, 0.2)
    wakes.update(0.1, { x: 100.3, y: 0, z: 0 })
    expect(wakes.area[0] % WAKE_TEXEL).toBeCloseTo(0, 6)
    expect(wakes.area[0]).toBeCloseTo(100 - (WAKE_GRID * WAKE_TEXEL) / 2, 6)
    expect(wakes.sample(100, 0)[0]).toBeLessThan(-0.04)
  })

  it('carve a trail a step apart, however fast the mover goes', () => {
    const dropped: number[] = []
    const trail = new WakeTrail(1)
    for (let x = 0; x <= 5; x += 0.25) trail.move((at) => dropped.push(at), x, 0, 0.1)
    expect(dropped).toEqual([0, 1, 2, 3, 4, 5])
    trail.lift()
    trail.move((at) => dropped.push(at), 5.1, 0, 0.1)
    expect(dropped.at(-1)).toBe(5.1)
  })
})
