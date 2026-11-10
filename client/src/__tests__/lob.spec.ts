import { describe, expect, it } from 'vitest'
import { MORTAR_BARREL_LENGTH, MORTAR_RANGE_MAX, MORTAR_RANGE_MIN, MORTAR_SHELL_SPEED, MORTAR_SHELL_TTL_TICKS } from '@/sim/consts'
import { dropTrajectory, type RayCaster } from '@/sim/grenade'
import { LOB_PITCH_HIGH, LOB_PITCH_LOW, layLob, layTube, lobPitch, lobRange } from '@/sim/lob'
import type { Vec3 } from '@/sim/motor'

/** Level ground at `height` and nothing else. */
const ground = (height: number): RayCaster => (origin, dir, max) => {
  if (dir[1] >= 0 || origin[1] <= height) return null
  const t = (origin[1] - height) / -dir[1]
  return t <= max ? { t, normal: [0, 1, 0] } : null
}

/** Where a bomb from a tube at `pivot`, laid to `yaw` and `pitch`, first lands. */
function landing(cast: RayCaster, pivot: Vec3, yaw: number, pitch: number): Vec3 {
  const dir: Vec3 = [-Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), -Math.cos(yaw) * Math.cos(pitch)]
  const start: Vec3 = [pivot[0] + dir[0] * MORTAR_BARREL_LENGTH, pivot[1] + dir[1] * MORTAR_BARREL_LENGTH, pivot[2] + dir[2] * MORTAR_BARREL_LENGTH]
  const velocity: Vec3 = [dir[0] * MORTAR_SHELL_SPEED, dir[1] * MORTAR_SHELL_SPEED, dir[2] * MORTAR_SHELL_SPEED]
  return dropTrajectory(cast, start, velocity, 1, MORTAR_SHELL_TTL_TICKS).firstImpact!
}

describe('the mortar lay', () => {
  it('agrees with the range dial on level ground, both ways', () => {
    for (const range of [MORTAR_RANGE_MIN, 120, MORTAR_RANGE_MAX - 1]) {
      expect(layLob(range, 0)).toBeCloseTo(lobPitch(range), 3)
      expect(lobRange(lobPitch(range))).toBeCloseTo(range, 1)
    }
  })

  it('lays steeper onto a hill and flatter into a valley than on the level', () => {
    expect(layLob(150, 30)).toBeLessThan(layLob(150, 0))
    expect(layLob(150, -30)).toBeGreaterThan(layLob(150, 0))
  })

  it('keeps within the dial, and throws as far as it can at a mark out of reach', () => {
    expect(layLob(5, 0)).toBe(LOB_PITCH_HIGH)
    expect(layLob(400, 0)).toBe(LOB_PITCH_LOW)
    expect(layLob(240, 80)).toBe(LOB_PITCH_LOW)
  })

  it.each([
    ['level', 0],
    ['on a hill', 25],
    ['in a valley', -25],
  ])('lands a bomb on the mark %s', (_, height) => {
    const pivot: Vec3 = [3, 1, -4]
    const mark: Vec3 = [70, height, -120]
    const { yaw, pitch } = layTube(pivot, mark)
    const at = landing(ground(height), pivot, yaw, pitch)
    expect(Math.hypot(at[0] - mark[0], at[2] - mark[2])).toBeLessThan(2)
  })
})
