import { describe, expect, it } from 'vitest'
import { buildLines } from '@/render/lines'
import { LINE_FENCE, LINE_FENCE_HIGH, LINE_GUARDRAIL, LINE_POWER, LINE_WALL, LINE_WALL_HIGH, lineKindDef, linePieceIndex, type Line } from '@/world'

describe('fences, walls and power lines', () => {
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

  it('stands a guard rail on a deck over a cutting, and a fence on the ground under it', () => {
    // Flat ground at 2 m, with the rail's foot at 6 m in its middle: a deck over a cutting.
    const ground = { x0: -10, z0: -10, cell: 1, side: 21, heights: new Float32Array(21 * 21).fill(2) }
    const lowest = (line: Line) => {
      const [posts] = buildLines([line], ground)
      return Math.min(...Array.from(posts!.geometry.positions.filter((_, i) => i % 3 === 1 && Math.abs(posts!.geometry.positions[i - 1]!) < 0.5)))
    }
    const rail: Line = { kind: LINE_GUARDRAIL, points: [[-4, 0], [-2, 0], [0, 0], [2, 0], [4, 0]], base: [2, 4, 6, 4, 2], end: true }
    expect(lowest(rail)).toBeGreaterThan(5)
    // A foot within a few centimetres of the ground (rounding, a dent) is on the ground.
    expect(lowest({ ...rail, base: [2, 2, 2.2, 2, 2] })).toBeLessThan(2.1)
  })

  it('strings wire on a fence and netting on a game fence', () => {
    const fence = buildLines([{ kind: LINE_FENCE, points: [[0, 0], [9, 0]], base: [0, 0], end: true }], null)
    expect(fence.length).toBe(2)
    const game = buildLines([{ kind: LINE_FENCE_HIGH, points: [[0, 0], [8, 0]], base: [0, 0], end: true }], null)
    expect(game.length).toBe(3)
    expect(buildLines(undefined, null)).toEqual([])
  })

  it('stands a post at each point of a fence and leaves a broken piece out', () => {
    const fence: Line = { kind: LINE_FENCE, points: [[0, 0], [3, 0], [6, 0], [9, 0]], base: [0, 0, 0, 0], end: true }
    const [posts, wires] = buildLines([fence], null)
    // Three strands a piece, two vertices across at each end.
    expect(wires!.geometry.vertexCount).toBe(3 * 3 * 4)
    const perPost = posts!.geometry.vertexCount / 4
    // The middle piece gone: its wire goes, and every post still has a piece beside it.
    const [, , ...none] = buildLines([fence], null, new Set([linePieceIndex(0, 1)]))
    const [mid, midWires] = buildLines([fence], null, new Set([linePieceIndex(0, 1)]))
    expect(none).toEqual([])
    expect(midWires!.geometry.vertexCount).toBe(3 * 2 * 4)
    expect(mid!.geometry.vertexCount).toBe(perPost * 4)
    // The first two gone: the first post has nothing to hold up.
    const [end] = buildLines([fence], null, new Set([linePieceIndex(0, 0), linePieceIndex(0, 1)]))
    expect(end!.geometry.vertexCount).toBe(perPost * 2)
    // The piece is named by the line's place among the tile's lines.
    const second = buildLines([fence, fence], null, new Set([linePieceIndex(1, 0), linePieceIndex(1, 1), linePieceIndex(1, 2)]))
    expect(second[0]!.geometry.vertexCount).toBe(perPost * 4)
  })

  it('builds a wall of blocks as tall as its kind, up the slope, and leaves a broken one out', () => {
    const wall = (kind: number): Line => ({ kind, points: [[0, 0], [3, 0], [6, 0]], base: [0, 0, 2], end: true })
    const ys = (meshes: ReturnType<typeof buildLines>) => Array.from(meshes[0]!.geometry.positions.filter((_, i) => i % 3 === 1))
    const xs = (meshes: ReturnType<typeof buildLines>) => Array.from(meshes[0]!.geometry.positions.filter((_, i) => i % 3 === 0))
    for (const kind of [LINE_WALL, LINE_WALL_HIGH]) {
      const meshes = buildLines([wall(kind)], null)
      // One solid mesh, no wire or netting. Its top is the kind's height over the higher end, about.
      expect(meshes.length).toBe(1)
      expect(Math.max(...ys(meshes))).toBeGreaterThan(2 + lineKindDef(kind)!.height * 0.8)
      expect(Math.max(...ys(meshes))).toBeLessThan(2 + lineKindDef(kind)!.height + 0.3)
      expect(Math.min(...ys(meshes))).toBeLessThan(0)
    }
    const whole = buildLines([wall(LINE_WALL)], null)
    const half = buildLines([wall(LINE_WALL)], null, new Set([linePieceIndex(0, 1)]))
    expect(half[0]!.geometry.vertexCount).toBe(whole[0]!.geometry.vertexCount / 2)
    expect(Math.max(...xs(half))).toBeLessThan(3.2)
    expect(buildLines([wall(LINE_WALL)], null, new Set([linePieceIndex(0, 0), linePieceIndex(0, 1)]))).toEqual([])
  })
})
