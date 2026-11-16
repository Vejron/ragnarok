import { describe, expect, it } from 'vitest'
import { dropTrajectory, type RayCaster } from '@/sim/grenade'
import { AMV_LOB, MORTAR_LOB, layLob, layTube, lobPitch, lobPitches, lobRange, type Lob } from '@/sim/lob'
import type { Vec3 } from '@/sim/motor'

/** Level ground at `height` and nothing else. */
const ground = (height: number): RayCaster => (origin, dir, max) => {
  if (dir[1] >= 0 || origin[1] <= height) return null
  const t = (origin[1] - height) / -dir[1]
  return t <= max ? { t, normal: [0, 1, 0] } : null
}

/** Where a bomb from a tube at `pivot`, laid to `yaw` and `pitch`, first lands. */
function landing(lob: Lob, cast: RayCaster, pivot: Vec3, yaw: number, pitch: number): Vec3 {
  const dir: Vec3 = [-Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), -Math.cos(yaw) * Math.cos(pitch)]
  const start: Vec3 = [pivot[0] + dir[0] * lob.length, pivot[1] + dir[1] * lob.length, pivot[2] + dir[2] * lob.length]
  const velocity: Vec3 = [dir[0] * lob.speed, dir[1] * lob.speed, dir[2] * lob.speed]
  return dropTrajectory(cast, start, velocity, 1, lob.ttl).firstImpact!
}

describe.each([
  ['the field mortar', MORTAR_LOB],
  ['the AMV', AMV_LOB],
])('the lay of %s', (_, lob) => {
  it('agrees with the range dial on level ground, both ways', () => {
    for (const range of [lob.min, lob.start, lob.max - 1]) {
      expect(layLob(lob, range, 0)).toBeCloseTo(lobPitch(lob, range), 3)
      expect(lobRange(lob, lobPitch(lob, range))).toBeCloseTo(range, 1)
    }
  })

  it('lays steeper onto a hill and flatter into a valley than on the level', () => {
    const range = lob.start
    expect(layLob(lob, range, 20)).toBeLessThan(layLob(lob, range, 0))
    expect(layLob(lob, range, -20)).toBeGreaterThan(layLob(lob, range, 0))
  })

  it('keeps within the dial, and throws as far as it can at a mark out of reach', () => {
    const { low, high } = lobPitches(lob)
    expect(layLob(lob, 5, 0)).toBe(high)
    expect(layLob(lob, lob.max + 100, 0)).toBe(low)
    expect(layLob(lob, lob.max - 60, 80)).toBe(low)
  })

  it.each([
    ['level', 0],
    ['on a hill', 15],
    ['in a valley', -15],
  ])('lands a bomb on the mark %s', (_, height) => {
    const pivot: Vec3 = [3, 1, -4]
    const mark: Vec3 = [40, height, -0.6 * lob.max]
    const { yaw, pitch } = layTube(lob, pivot, mark)
    const at = landing(lob, ground(height), pivot, yaw, pitch)
    expect(Math.hypot(at[0] - mark[0], at[2] - mark[2])).toBeLessThan(2)
  })
})

describe('the AMV against the field mortar', () => {
  it('reaches two thirds as far, with a smaller burst', () => {
    expect(AMV_LOB.max).toBeLessThan(MORTAR_LOB.max * 0.7)
    expect(AMV_LOB.radius).toBeLessThan(MORTAR_LOB.radius)
  })
})
