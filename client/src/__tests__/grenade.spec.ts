import { describe, expect, it } from 'vitest'
import { dropTrajectory, throwTrajectory, type RayCaster } from '@/sim/grenade'
import { GRAVITY } from '@/sim/consts'
import { GRENADE_FUSE_TICKS } from '@/sim/items'

/** A flat floor at y = 0 and nothing else. */
const floor: RayCaster = (origin, dir, max) => {
  if (dir[1] >= 0 || origin[1] <= 0) return null
  const t = origin[1] / -dir[1]
  return t <= max ? { t, normal: [0, 1, 0] } : null
}
const open: RayCaster = () => null

const distance = (a: readonly number[], b: readonly number[]) => Math.hypot(a[0]! - b[0]!, a[2]! - b[2]!)

describe('throwTrajectory', () => {
  it('goes the way the player faces, and starts at the hand', () => {
    const t = throwTrajectory(floor, [0, 1.5, 0], 0, 0)
    const end = t.points[t.points.length - 1]!
    // Yaw 0 faces -Z.
    expect(end[2]).toBeLessThan(-3)
    expect(Math.abs(end[0])).toBeLessThan(1e-6)
    expect(distance(t.points[0]!, [0, 0, 0])).toBeLessThan(1)
    const turned = throwTrajectory(floor, [0, 1.5, 0], Math.PI / 2, 0)
    expect(turned.points[turned.points.length - 1]![0]).toBeLessThan(-3)
  })

  it('lands first further away the higher it is aimed, up to a point', () => {
    const land = (pitch: number) => throwTrajectory(floor, [0, 1.5, 0], 0, pitch).firstImpact!
    const flat = distance(land(-0.2), [0, 0, 0])
    const level = distance(land(0), [0, 0, 0])
    const raised = distance(land(0.3), [0, 0, 0])
    expect(level).toBeGreaterThan(flat)
    expect(raised).toBeGreaterThan(level)
    // Straight up it comes back near the thrower.
    expect(distance(land(1.4), [0, 0, 0])).toBeLessThan(raised)
  })

  it('bounces and comes to rest, bursting where the fuse leaves it', () => {
    const t = throwTrajectory(floor, [0, 1.5, 0], 0, 0.2)
    expect(t.firstImpact).not.toBeNull()
    expect(t.burst[1]).toBeCloseTo(0.1, 1)
    expect(distance(t.burst, [0, 0, 0])).toBeGreaterThan(distance(t.firstImpact!, [0, 0, 0]))
    expect(t.points.length).toBeGreaterThan(10)
    expect(t.points.length).toBeLessThanOrEqual(GRENADE_FUSE_TICKS / 2 + 2)
  })

  it('with nothing to hit never lands and ends in the air', () => {
    const t = throwTrajectory(open, [0, 1.5, 0], 0, 0)
    expect(t.firstImpact).toBeNull()
    expect(t.burst[1]).toBeLessThan(-40)
  })

  it('starts at a wall rather than behind it', () => {
    const wall: RayCaster = (origin, dir, max) => {
      if (dir[2] >= 0) return null
      const t = (origin[2] - -0.2) / -dir[2]
      return t >= 0 && t <= max ? { t, normal: [0, 0, 1] } : null
    }
    const t = throwTrajectory(wall, [0, 1.5, 0], 0, 0)
    expect(t.points[0]![2]).toBeGreaterThan(-0.2)
  })
})

describe('dropTrajectory', () => {
  it('carries a bomb on with the plane and lands it where it falls to', () => {
    const t = dropTrajectory(floor, [0, 50, 0], [0, 0, -30])
    const impact = t.firstImpact!
    // It falls 50 m in √(2·50/g) seconds, all the while moving on at the plane's 30 m/s.
    const fall = Math.sqrt((2 * 50) / GRAVITY)
    expect(impact[1]).toBeCloseTo(0, 3)
    expect(impact[2]).toBeCloseTo(-30 * fall, 0)
    expect(Math.abs(impact[0])).toBeLessThan(1e-6)
    expect(t.burst).toEqual(impact)
    expect(t.points[t.points.length - 1]).toEqual(impact)
  })

  it('never lands in an open sky', () => {
    expect(dropTrajectory(open, [0, 50, 0], [10, 0, 0]).firstImpact).toBeNull()
  })
})
