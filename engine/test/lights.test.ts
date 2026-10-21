import { describe, expect, it } from 'vitest'
import { PointLights } from '../src/scene/lights'

describe('PointLights', () => {
  it('fades a flash out over its life', () => {
    const lights = new PointLights()
    lights.flash(0, 0, 0, { color: 0xffffff, intensity: 2, radius: 10, life: 0.5 })
    lights.update(0.25)
    expect(PointLights.decay(lights.live[0]!)).toBeCloseTo(0.25)
    lights.update(0.25)
    expect(lights.live).toHaveLength(0)
  })

  it('keeps a held light at full strength for the frame after it was held, then drops it', () => {
    const lights = new PointLights()
    lights.hold(0, 0, 0, { color: 0xffffff, intensity: 2, radius: 10 })
    lights.update(1 / 60)
    expect(lights.live).toHaveLength(1)
    expect(PointLights.decay(lights.live[0]!)).toBe(1)
    lights.update(1 / 60)
    expect(lights.live).toHaveLength(0)
  })

  it('holds a light through a frame that took no time', () => {
    const lights = new PointLights()
    lights.hold(0, 0, 0, { color: 0xffffff, intensity: 2, radius: 10 })
    lights.update(0)
    expect(lights.live).toHaveLength(1)
    lights.update(0)
    expect(lights.live).toHaveLength(0)
  })
})
