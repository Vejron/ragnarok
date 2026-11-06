import { describe, expect, it } from 'vitest'
import { AdaptiveScale } from '@/render/adaptive'

const HZ60 = 1000 / 60

/** Feeds `seconds` of frames `interval` ms apart from `from`; the time after, and every scale it stepped to. */
function run(scale: AdaptiveScale, from: number, seconds: number, interval: number, gpuMs?: number) {
  const steps: number[] = []
  let now = from
  while (now < from + seconds * 1000) {
    now += interval
    if (scale.frame(now, interval, gpuMs)) steps.push(scale.scale)
  }
  return { now, steps }
}

describe('adaptive render scale', () => {
  it('stays put at full speed', () => {
    const scale = new AdaptiveScale(0.6)
    expect(run(scale, 0, 10, HZ60).steps).toEqual([])
    expect(scale.scale).toBe(1)
  })

  it('steps down while frames are slow, a second apart, to no lower than the floor', () => {
    const scale = new AdaptiveScale(0.7)
    const { now } = run(scale, 0, 2, HZ60)
    const slow = run(scale, now, 1.2, 30)
    expect(slow.steps).toEqual([0.9])
    expect(run(scale, slow.now, 20, 30).steps).toEqual([0.8, 0.7])
    expect(scale.scale).toBe(0.7)
  })

  it('takes a long spell of slow frames for slow frames, not for a slow display', () => {
    const scale = new AdaptiveScale(0.3)
    const { now } = run(scale, 0, 2, HZ60)
    run(scale, now, 6, 30)
    expect(scale.scale).toBeLessThanOrEqual(0.5)
  })

  it('climbs back once frames are quick for a while', () => {
    const scale = new AdaptiveScale(0.5)
    let { now } = run(scale, 0, 2, HZ60)
    now = run(scale, now, 1.2, 30).now
    expect(scale.scale).toBe(0.9)
    const fast = run(scale, now, 2, HZ60)
    expect(fast.steps, 'not straight away').toEqual([])
    expect(run(scale, fast.now, 5, HZ60).steps).toEqual([0.95, 1])
  })

  it('leaves the scale alone where the GPU is not what is slow', () => {
    const scale = new AdaptiveScale(0.5)
    const { now } = run(scale, 0, 2, HZ60, 4)
    expect(run(scale, now, 5, 30, 4).steps).toEqual([])
    expect(run(scale, now + 5000, 2, 30, 28).steps.length).toBeGreaterThan(0)
  })

  it('aims at a fast display’s own rate', () => {
    const scale = new AdaptiveScale(0.5)
    const { now } = run(scale, 0, 2, 1000 / 120)
    // 80 fps would be fine on a 60 Hz screen; on this one it is slow.
    expect(run(scale, now, 1.2, 12.5).steps).toEqual([0.9])
  })

  it('ignores frames while paused and hitches', () => {
    const scale = new AdaptiveScale(0.5)
    let { now } = run(scale, 0, 2, HZ60)
    scale.pause(now, 3000)
    expect(run(scale, now, 2.5, 30).steps).toEqual([])
    now += 2500
    expect(scale.frame(now + 400, 400)).toBe(false)
    expect(scale.scale).toBe(1)
  })

  it('starts again at full scale on reset', () => {
    const scale = new AdaptiveScale(0.5)
    const { now } = run(scale, 0, 2, HZ60)
    run(scale, now, 1.2, 30)
    expect(scale.scale).toBeLessThan(1)
    scale.reset(now + 1200, 0.7)
    expect(scale.scale).toBe(1)
    expect(scale.floor).toBe(0.7)
  })
})
