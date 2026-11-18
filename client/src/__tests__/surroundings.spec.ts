import { describe, expect, it } from 'vitest'
import { Surroundings } from '@/surroundings'
import { Waters } from '@/sim/waters'
import type { FxWorld } from '@/render/tyres'

describe('a round striking the water', () => {
  it('splashes on the still level the server ends it on, even with a trough under it', () => {
    // Still water at 2 m; the waves have it 8 cm lower here just now.
    const waters = new Waters(50, 100)
    const side = 3
    waters.set({ col: 0, row: 0, hash: 0 }, { x0: -50, z0: -50, cell: 50, side, heights: new Float32Array(side * side).fill(-1) }, new Float32Array(side * side).fill(2))
    const heard: number[][] = []
    // The camera far off, so the splash is only heard.
    const fx = { waterAt: () => 1.92, camera: { x: 1e4, y: 0, z: 0 }, heard: { splash: (at: number[]) => heard.push(at) } } as unknown as FxWorld
    const surroundings = new Surroundings({} as never, () => fx)
    surroundings.waters = waters
    expect(surroundings.watery({ x: 0, y: 2, z: 0 }, 1)).toBe(true)
    expect(heard).toHaveLength(1)
    expect(surroundings.watery({ x: 0, y: 2.3, z: 0 }, 1)).toBe(false)
  })
})
