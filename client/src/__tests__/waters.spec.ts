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

  it('gives the current between its samples, as the sim does, and still water where a tile has none', () => {
    const river = new Waters(50, 100)
    // East at 2 m/s along the west column, 4 m/s along the middle one (FLOW_STEP 0.05), still on the east.
    const flow = new Int8Array(SIDE * SIDE * 2)
    for (let row = 0; row < SIDE; row++) flow.set([40, 0, 80, -20], row * SIDE * 2)
    river.set({ col: 0, row: 0, hash: 0 }, ground(-1), new Float32Array(SIDE * SIDE).fill(1), undefined, flow)
    const [x, z] = river.flowAt(-25, 0)
    expect(x).toBeCloseTo(3)
    expect(z).toBeCloseTo(-0.5)
    expect(river.flowAt(25, 0)[0]).toBeCloseTo(2)
    const still = new Waters(50, 100)
    still.set({ col: 0, row: 0, hash: 0 }, ground(-1), new Float32Array(SIDE * SIDE).fill(1))
    expect(still.flowAt(0, 0)).toEqual([0, 0])
  })

  it('forgets a tile that streams out', () => {
    waters.delete({ col: 0, row: 0, hash: 0 })
    expect(waters.levelAt(0, 0)).toBe(NO_WATER)
  })
})

describe('the mirrored water', () => {
  // A river 400 m across the camera, stepping down a rapid at x = 0: 5 m upstream (west), 2 m below it, and
  // falling gently on each side.
  const SIDE = 81
  const river = new Waters(200, 400)
  const levels = new Float32Array(SIDE * SIDE)
  for (let j = 0; j < SIDE; j++) for (let i = 0; i < SIDE; i++) levels[j * SIDE + i] = (i < 40 ? 5 : 2) - i * 0.002
  river.set({ col: 0, row: 0, hash: 0 }, { x0: -200, z0: -200, cell: 5, side: SIDE, heights: new Float32Array(SIDE * SIDE).fill(-3) }, levels)

  it('mirrors the water the camera looks at, a gently falling stretch of it as one level', () => {
    expect(river.planeNear(0, 0, 1, 0)).toBeCloseTo(1.8, 0)
    expect(river.planeNear(0, 0, -1, 0)).toBeCloseTo(4.8, 0)
  })

  it('changes level once as the camera turns from one water to the other, later than it would afresh', () => {
    // From 5 m above the rapid, looking east (downstream) round through north to west (upstream), a degree at a time.
    const turn = (keep: boolean) => {
      let kept: number | null = null
      const levels: number[] = []
      for (let degree = 0; degree <= 180; degree++) {
        const a = (degree * Math.PI) / 180
        kept = river.planeNear(-5, 0, Math.cos(a), -Math.sin(a), keep ? kept : null)
        levels.push(kept!)
      }
      return levels
    }
    const flips = (levels: number[]) => levels.flatMap((level, k) => (k > 0 && Math.abs(level - levels[k - 1]!) > 1 ? [k] : []))
    const afresh = flips(turn(false))
    const kept = flips(turn(true))
    expect(afresh).toHaveLength(1)
    expect(kept).toHaveLength(1)
    expect(kept[0]).toBeGreaterThan(afresh[0]! + 10)
  })
})
