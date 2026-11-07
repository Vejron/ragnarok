import { describe, expect, it } from 'vitest'
import { arcTime, Rounds } from '@/render/rounds'
import { BULLET_GRAVITY_SCALE, BULLET_SPEED, GRAVITY } from '@/sim/consts'

type V = { x: number; y: number; z: number }

function harness() {
  const placed: number[][] = []
  const sparks: V[] = []
  const ended: number[] = []
  const models = { place: (_id: number, _kind: number, at: number[]) => placed.push([...at]), remove() {} }
  const trails = { follow() {}, end: (_g: string, id: number) => ended.push(id) }
  const rounds = new Rounds(models as never, trails as never, (at) => sparks.push(at))
  return { rounds, placed, sparks, ended }
}

const AHEAD = { x: 0, y: 0, z: -1 }
const FALL = GRAVITY * BULLET_GRAVITY_SCALE

describe('Rounds', () => {
  it('flies a round at bullet speed, falling, and sparks where it stops on something', () => {
    const { rounds, placed, sparks } = harness()
    rounds.fire({ x: 0, y: 0, z: 0 }, AHEAD, 1, true)
    rounds.update(0.5)
    const at = placed[placed.length - 1]!
    expect(at[2]).toBeCloseTo(-BULLET_SPEED / 2)
    expect(at[1]).toBeCloseTo(-0.5 * FALL * 0.25)
    expect(sparks).toEqual([])
    rounds.update(0.6)
    expect(rounds.count).toBe(0)
    expect(sparks).toHaveLength(1)
    expect(sparks[0]!.z).toBeCloseTo(-BULLET_SPEED)
    expect(sparks[0]!.y).toBeCloseTo(-0.5 * FALL)
  })

  it('a round into the open does not spark', () => {
    const { rounds, sparks } = harness()
    rounds.fire({ x: 0, y: 0, z: 0 }, { x: 1, y: 0, z: 0 }, 0.1, false)
    rounds.update(1)
    expect(rounds.count).toBe(0)
    expect(sparks).toEqual([])
  })

  it('a hit the server reports cuts the round short there', () => {
    const { rounds, sparks } = harness()
    rounds.fire({ x: 0, y: 0, z: 0 }, AHEAD, 300 / BULLET_SPEED, false)
    const t = 40 / BULLET_SPEED
    rounds.struck({ x: 0.3, y: -0.5 * FALL * t * t, z: -40 })
    expect(sparks).toEqual([])
    rounds.update(1)
    expect(sparks).toHaveLength(1)
    expect(sparks[0]!.z).toBeCloseTo(-40)
  })

  it('a hit with no round in flight near it sparks at once', () => {
    const { rounds, sparks } = harness()
    rounds.struck({ x: 5, y: 5, z: 5 })
    expect(sparks).toEqual([{ x: 5, y: 5, z: 5 }])
  })
})

describe('arcTime', () => {
  it('follows the fall down to the ground, which a straight ray level with it never meets', () => {
    // Flat ground two metres under a level barrel.
    const ground = (origin: [number, number, number], d: [number, number, number], max: number) => {
      if (d[1] >= 0) return null
      const t = (origin[1] + 2) / -d[1]
      return t <= max ? { t } : null
    }
    const { time, hit } = arcTime(ground, { x: 0, y: 0, z: 0 }, AHEAD, 2000)
    expect(hit).toBe(true)
    // Two metres down under the fall: t = sqrt(2 h / g).
    expect(time).toBeCloseTo(Math.sqrt(4 / FALL), 2)
    expect(arcTime(() => null, { x: 0, y: 0, z: 0 }, AHEAD, 280)).toEqual({ time: 280 / BULLET_SPEED, hit: false })
  })
})
