import { describe, expect, it } from 'vitest'
import { Interpolator } from '@/net/interp'
import type { EntityState } from '@/net/codec'

const at = (x: number, yaw = 0): EntityState => ({ id: 7, kind: 0, x, y: 1, z: 0, yaw, vy: 0, grounded: true })

describe('Interpolator', () => {
  it('blends between the bracketing snapshots', () => {
    const interp = new Interpolator()
    interp.push(10, [at(0)])
    interp.push(12, [at(2)])
    expect(interp.sample(7, 11)!.x).toBeCloseTo(1)
    expect(interp.sample(7, 11)!.speed).toBeCloseTo(60)
  })

  it('accepts out-of-order snapshots', () => {
    const interp = new Interpolator()
    interp.push(10, [at(0)])
    interp.push(14, [at(4)])
    interp.push(12, [at(1)])
    expect(interp.sample(7, 13)!.x).toBeCloseTo(2.5)
  })

  it('holds the newest state past the end', () => {
    const interp = new Interpolator()
    interp.push(10, [at(0)])
    interp.push(12, [at(2)])
    expect(interp.sample(7, 20)!.x).toBe(2)
  })

  it('takes the short way round for yaw', () => {
    const interp = new Interpolator()
    interp.push(10, [at(0, 3.0)])
    interp.push(12, [at(0, -3.0)])
    const yaw = interp.sample(7, 11)!.yaw
    expect(Math.abs(Math.abs(yaw) - Math.PI)).toBeLessThan(0.01)
  })

  it('drops entities that stop appearing', () => {
    const interp = new Interpolator()
    interp.push(10, [at(0)])
    interp.push(500, [])
    expect(interp.ids()).toEqual([])
  })
})
