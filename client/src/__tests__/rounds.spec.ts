import { describe, expect, it } from 'vitest'
import { Rounds } from '@/render/rounds'
import { BULLET_SPEED } from '@/sim/consts'

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

describe('Rounds', () => {
  it('flies a round at bullet speed and sparks where it stops on something', () => {
    const { rounds, placed, sparks } = harness()
    rounds.fire({ x: 0, y: 0, z: 0 }, { x: 0, y: 0, z: -BULLET_SPEED }, true)
    rounds.update(0.5)
    expect(placed.at(-1)![2]).toBeCloseTo(-BULLET_SPEED / 2)
    expect(sparks).toEqual([])
    rounds.update(0.6)
    expect(rounds.count).toBe(0)
    expect(sparks).toHaveLength(1)
    expect(sparks[0]!.z).toBeCloseTo(-BULLET_SPEED)
  })

  it('a round into the open does not spark', () => {
    const { rounds, sparks } = harness()
    rounds.fire({ x: 0, y: 0, z: 0 }, { x: 10, y: 0, z: 0 }, false)
    rounds.update(1)
    expect(rounds.count).toBe(0)
    expect(sparks).toEqual([])
  })

  it('a hit the server reports cuts the round short there', () => {
    const { rounds, sparks } = harness()
    rounds.fire({ x: 0, y: 0, z: 0 }, { x: 0, y: 0, z: -300 }, false)
    rounds.struck({ x: 0.3, y: 0, z: -40 })
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
