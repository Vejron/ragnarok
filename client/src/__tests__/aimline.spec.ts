import { describe, expect, it } from 'vitest'
import { blockedAt, clearArm, lookDir, shoulderPivot, spreadPixels } from '@/aimline'
import { ON_FOOT_HEIGHT, ON_FOOT_SHOULDER } from '@/sim/consts'

type Vec3 = [number, number, number]

/** A wall across -Z at distance `at`, up to height `top`; nothing beyond it. */
const wall = (at: number, top: number) => (origin: Vec3, direction: Vec3, max: number) => {
  if (direction[2] >= 0) return null
  const t = (at + origin[2]) / -direction[2]
  return t >= 0 && t <= max && origin[1] + direction[1] * t <= top ? { t } : null
}

describe('clearArm', () => {
  const flat = () => 0
  // Looking along -Z from a pivot 2 m up, the arm runs back along +Z.
  const back = [0, 0, 1]

  it('takes the whole distance with room', () => {
    expect(clearArm([0, 2, 0], back, 3.2, flat, 0.3)).toBe(3.2)
  })

  it('stops short of a slope rising behind', () => {
    const arm = clearArm([0, 2, 0], back, 3.2, (_x, z) => z, 0.3)
    expect(arm).toBeGreaterThan(1.6)
    expect(arm).toBeLessThan(1.71)
  })

  it('is smooth as the ground rises, not stepped', () => {
    const arms = [0, 0.02, 0.04].map((rise) => clearArm([0, 2, 0], back, 3.2, (_x, z) => z + rise, 0.3))
    expect(Math.abs(arms[0]! - arms[1]!)).toBeLessThan(0.06)
    expect(Math.abs(arms[1]! - arms[2]!)).toBeLessThan(0.06)
  })

  it('is nothing when the pivot is itself in the ground', () => {
    expect(clearArm([0, 0.1, 0], back, 3.2, flat, 0.3)).toBe(0)
  })
})

describe('spreadPixels', () => {
  it('is half the picture at the edge of the lens', () => {
    expect(spreadPixels((75 * Math.PI) / 360, 75, 1080)).toBeCloseTo(540)
  })

  it('grows as the lens narrows', () => {
    expect(spreadPixels(0.012, 50, 1080)).toBeGreaterThan(spreadPixels(0.012, 75, 1080))
  })
})

describe('blockedAt', () => {
  const position = [0, 1, 0]
  const eye: Vec3 = [0, 1.6, 0]

  it('is nothing with a clear line', () => {
    expect(blockedAt(() => null, position, eye, 0, 0, 200)).toBeNull()
  })

  it('is nothing for a wall ahead: the round leaves on the crosshair\'s line', () => {
    expect(blockedAt(wall(10, 2.2), position, eye, 0, 0, 200)).toBeNull()
  })

  it('finds what stops the round from the eye when a wall at the shoulder keeps it off that line', () => {
    // Along the soldier's right, between the eye and the pivot, which sits beyond it.
    const side = (origin: Vec3, direction: Vec3, max: number) => {
      if (direction[0] <= 0) return null
      const t = (0.3 - origin[0]) / direction[0]
      return t >= 0 && t <= max ? { t } : null
    }
    const hit = blockedAt(side, position, eye, 0, 0, 200)
    expect(hit?.[0]).toBeCloseTo(0.3)
  })
})

describe('the shoulder', () => {
  it('turns with the look, on the body\'s right', () => {
    expect(shoulderPivot([0, 0, 0], 0)).toEqual([ON_FOOT_SHOULDER, ON_FOOT_HEIGHT, 0])
    expect(lookDir(0, 0)[2]).toBe(-1)
  })
})
