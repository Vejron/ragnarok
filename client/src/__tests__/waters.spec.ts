import { describe, expect, it } from 'vitest'
import { easeMirror, Waters, type ShownMirror, type WaterPlane } from '@/sim/waters'
import type { TileGround } from '@/sim/terrain'
import { NO_WATER } from '@/world'
import { CHURN_HEIGHT } from '@/sim/waves'

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

  it('reads a falling river’s level between its samples, and adds the surge of a rapid to the surface as the sim does', () => {
    const river = new Waters(50, 100)
    // Falling 1 m from the west column to the east one, 2 m deep, running east at 4 m/s.
    const levels = new Float32Array(SIDE * SIDE)
    const heights = new Float32Array(SIDE * SIDE)
    const flow = new Int8Array(SIDE * SIDE * 2)
    for (let row = 0; row < SIDE; row++) {
      for (let column = 0; column < SIDE; column++) {
        levels[row * SIDE + column] = 3 - column * 0.5
        heights[row * SIDE + column] = 1 - column * 0.5
        flow[(row * SIDE + column) * 2] = 80
      }
    }
    river.set({ col: 0, row: 0, hash: 0 }, { x0: -50, z0: -50, cell: 50, side: SIDE, heights }, levels, undefined, flow)
    expect(river.levelAt(0, 0)).toBeCloseTo(2.5)
    expect(river.levelAt(-25, 10)).toBeCloseTo(2.75)
    const heaves = Array.from({ length: 40 }, (_, k) => river.surfaceAt(-25 + k * 0.7, 10, k * 0.31) - river.levelAt(-25 + k * 0.7, 10))
    expect(Math.max(...heaves.map(Math.abs))).toBeGreaterThan(0.02)
    expect(Math.max(...heaves.map(Math.abs))).toBeLessThanOrEqual(CHURN_HEIGHT + 1e-6)
  })

  it('finds the boulders just under a fast river’s surface, and none in slow water or on ice', () => {
    const SIDE = 9
    const river = new Waters(50, 100)
    const heights = new Float32Array(SIDE * SIDE).fill(-2)
    const levels = new Float32Array(SIDE * SIDE).fill(0)
    const flow = new Int8Array(SIDE * SIDE * 2)
    for (let at = 0; at < SIDE * SIDE; at++) flow[at * 2] = at % SIDE < 6 ? 60 : 10
    // One boulder with its top 0.3 m down in the fast water, and one as shallow in the slow water.
    heights[4 * SIDE + 3] = -0.3
    heights[4 * SIDE + 7] = -0.3
    river.set({ col: 0, row: 0, hash: 0 }, { x0: -40, z0: -40, cell: 10, side: SIDE, heights }, levels, undefined, flow)
    const rocks = river.rocksNear(0, 0, 60, 0.45, 1)
    expect(rocks).toHaveLength(1)
    expect(rocks[0]).toMatchObject({ x: -10, z: 0 })
    expect(rocks[0]!.depth).toBeCloseTo(0.3)
    expect(rocks[0]!.flow[0]).toBeCloseTo(3)
    expect(river.rocksNear(0, 0, 5, 0.45, 1)).toHaveLength(0)
    river.calm = true
    expect(river.rocksNear(0, 0, 60, 0.45, 1)).toHaveLength(0)
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
    expect(river.planeNear(0, 0, 1, 0)!.y).toBeCloseTo(1.8, 0)
    expect(river.planeNear(0, 0, -1, 0)!.y).toBeCloseTo(4.8, 0)
  })

  it('changes level once as the camera turns from one water to the other, later than it would afresh', () => {
    // From 5 m above the rapid, looking east (downstream) round through north to west (upstream), a degree at a time.
    const turn = (keep: boolean) => {
      let kept: WaterPlane | null = null
      const levels: number[] = []
      for (let degree = 0; degree <= 180; degree++) {
        const a = (degree * Math.PI) / 180
        kept = river.planeNear(-5, 0, Math.cos(a), -Math.sin(a), keep ? kept : null)
        levels.push(kept!.y)
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

describe('the mirrored plane on a falling river', () => {
  // A river 400 m wide falling 4 cm a metre towards +x, as a rapid's run does, and a lake beside it.
  const SIDE = 81
  const make = (fall: number) => {
    const waters = new Waters(200, 400)
    const levels = new Float32Array(SIDE * SIDE)
    for (let j = 0; j < SIDE; j++) for (let i = 0; i < SIDE; i++) levels[j * SIDE + i] = 20 - (i * 5 - 200) * fall
    waters.set({ col: 0, row: 0, hash: 0 }, { x0: -200, z0: -200, cell: 5, side: SIDE, heights: new Float32Array(SIDE * SIDE).fill(-3) }, levels)
    return waters
  }

  it('leans with the river, so the water far up and down it is mirrored too', () => {
    const plane = make(0.04).planeNear(0, 0, -1, 0)!
    expect(plane.slopeX).toBeCloseTo(-0.04, 2)
    expect(plane.slopeZ).toBeCloseTo(0, 3)
    expect(plane.y).toBeCloseTo(20, 1)
  })

  it('lies level on a lake', () => {
    const plane = make(0).planeNear(0, 0, 1, 0)!
    expect(plane.slopeX).toBeCloseTo(0, 6)
    expect(plane.slopeZ).toBeCloseTo(0, 6)
    expect(plane.y).toBeCloseTo(20, 3)
  })

  it('moves smoothly as the camera goes up the river', () => {
    const waters = make(0.04)
    let kept: WaterPlane | null = null
    let last: number | null = null
    for (let x = 60; x >= -60; x -= 0.5) {
      kept = waters.planeNear(x, 0, -1, 0, kept)
      // The mirrored plane under the camera stays on the water there, well within the shader's 20 cm,
      // and never jumps: it moves no more than the river falls in the step and a little.
      expect(Math.abs(kept!.y - (20 - x * 0.04))).toBeLessThan(0.05)
      if (last !== null) expect(Math.abs(kept!.y - last - 0.02)).toBeLessThan(0.02)
      last = kept!.y
    }
  })
})

describe('the mirror as shown', () => {
  const at = (y: number): WaterPlane => ({ x: 0, y, z: 0, slopeX: 0, slopeZ: 0 })
  const DT = 1 / 60

  it('slides from one water to another and never jumps', () => {
    let shown: ShownMirror | null = { plane: at(5), strength: 1 }
    let last = 5
    for (let frame = 0; frame < 60; frame++) {
      shown = easeMirror(shown, at(2), DT)!
      // 3 m apart: no frame moves it more than 30 cm.
      expect(Math.abs(shown.plane.y - last)).toBeLessThan(0.3)
      last = shown.plane.y
    }
    expect(last).toBeCloseTo(2, 1)
  })

  it('fades out where no water is about, and back in', () => {
    let shown: ShownMirror | null = { plane: at(5), strength: 1 }
    const out: number[] = []
    while (shown) {
      shown = easeMirror(shown, null, DT)
      out.push(shown?.strength ?? 0)
    }
    // Over about half a second, a little a frame.
    expect(out.length).toBeGreaterThan(25)
    out.forEach((strength, k) => k > 0 && expect(out[k - 1]! - strength).toBeLessThan(0.05))
    shown = easeMirror(null, at(5), DT)
    expect(shown!.strength).toBeLessThan(0.05)
  })

  it('finds water far below a camera high over it', () => {
    // Water only 400 m and more from the camera, to the west.
    const SIDE = 81
    const waters = new Waters(400, 800)
    const levels = new Float32Array(SIDE * SIDE).fill(NO_WATER)
    for (let j = 0; j < SIDE; j++) for (let i = 0; i < 20; i++) levels[j * SIDE + i] = 2
    waters.set({ col: 0, row: 0, hash: 0 }, { x0: -400, z0: -400, cell: 10, side: SIDE, heights: new Float32Array(SIDE * SIDE).fill(-3) }, levels)
    expect(waters.planeNear(0, 0, -1, 0)?.y).toBeCloseTo(2, 3)
  })
})
