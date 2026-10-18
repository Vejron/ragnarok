import { describe, expect, it } from 'vitest'
import { Waters } from '@/sim/waters'
import type { TileGround } from '@/sim/terrain'
import { NO_WATER } from '@/world'

const SIDE = 3
const ground = (height: number): TileGround => ({ x0: -50, z0: -50, cell: 50, side: SIDE, heights: new Float32Array(SIDE * SIDE).fill(height) })

describe('Waters', () => {
  const waters = new Waters(50, 100)

  it('knows nothing of a tile that is not loaded', () => {
    expect(waters.levelAt(0, 0)).toBe(NO_WATER)
  })

  it('gives the surface height where the bed is under it', () => {
    const levels = new Float32Array(SIDE * SIDE).fill(NO_WATER)
    levels[4] = 2
    waters.set({ col: 0, row: 0, hash: 0 }, ground(-1), levels)
    expect(waters.levelAt(0, 0)).toBe(2)
    expect(waters.levelAt(-50, -50)).toBe(NO_WATER)
  })

  it('ignores water at or under the ground', () => {
    const dry = new Waters(50, 100)
    dry.set({ col: 0, row: 0, hash: 0 }, ground(5), new Float32Array(SIDE * SIDE).fill(2))
    expect(dry.levelAt(0, 0)).toBe(NO_WATER)
  })

  it('forgets a tile that streams out', () => {
    waters.delete({ col: 0, row: 0, hash: 0 })
    expect(waters.levelAt(0, 0)).toBe(NO_WATER)
  })
})
