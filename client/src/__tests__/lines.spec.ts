import { describe, expect, it } from 'vitest'
import { along, buildLines } from '@/render/lines'
import { LINE_FENCE, LINE_FENCE_HIGH, LINE_POWER, type Line } from '@/world'

describe('fences and power lines', () => {
  it('sets posts evenly, with one at each end and none just short of it', () => {
    expect(along([[0, 0], [10, 0]], 3)).toEqual([[0, 0], [3, 0], [6, 0], [10, 0]])
    // Round a corner, the step carries on from where it was.
    expect(along([[0, 0], [2, 0], [2, 4]], 3)).toEqual([[0, 0], [2, 1], [2, 4]])
  })

  it('draws a pole at the last point only where the line ends there', () => {
    const power = (end: boolean): Line => ({ kind: LINE_POWER, points: [[0, 0], [50, 0], [100, 10]], base: [1, 2, 3], end })
    const [open, wires] = buildLines([power(false)], null)
    const [closed] = buildLines([power(true)], null)
    expect(closed!.geometry.vertexCount).toBe(open!.geometry.vertexCount * 1.5)
    // Three wires over two spans, two vertices across at each of eleven points.
    expect(wires!.geometry.vertexCount).toBe(3 * 2 * 11 * 2)
    // Hung from the crossarm, nine metres up and more.
    expect(Math.min(...Array.from(wires!.geometry.positions.filter((_, i) => i % 3 === 1)))).toBeGreaterThan(9)
  })

  it('strings wire on a fence and netting on a game fence', () => {
    const fence = buildLines([{ kind: LINE_FENCE, points: [[0, 0], [9, 0]], base: [0, 0], end: true }], null)
    expect(fence.length).toBe(2)
    const game = buildLines([{ kind: LINE_FENCE_HIGH, points: [[0, 0], [8, 0]], base: [0, 0], end: true }], null)
    expect(game.length).toBe(3)
    expect(buildLines(undefined, null)).toEqual([])
  })
})
