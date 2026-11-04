import { Mesh } from '@ragnarok/engine'
import { describe, expect, it } from 'vitest'
import { buildWater, disposeWater } from '@/render/water'
import type { TileGround } from '@/sim/terrain'
import { NO_WATER } from '@/world'

/** 5 × 5 samples 5 m apart, flat at 2 m, with a pond of 3 × 3 samples bedded 1 m down. */
function pond() {
  const side = 5
  const heights = new Float32Array(side * side).fill(2)
  const water = new Float32Array(side * side).fill(NO_WATER)
  for (let row = 1; row <= 3; row++) {
    for (let column = 1; column <= 3; column++) {
      heights[row * side + column] = 1
      water[row * side + column] = 2
    }
  }
  const ground: TileGround = { x0: 0, z0: 0, cell: 5, side, heights }
  return { ground, water }
}

describe('water', () => {
  it('draws only the cells that touch the pond, with depth at the vertices', () => {
    const { ground, water } = pond()
    const group = buildWater({ water, streams: [] }, ground)!
    const mesh = group.children[0] as Mesh
    // The geometry's custom attribute carries depth in x.
    const custom = mesh.geometry.custom!
    const positions = mesh.geometry.positions
    const depthAt = (x: number, z: number) => {
      for (let i = 0; i < positions.length / 3; i++) if (positions[i * 3] === x && positions[i * 3 + 2] === z) return custom[i * 4]!
      return undefined
    }
    expect(depthAt(10, 10)).toBeCloseTo(1)
    expect(depthAt(0, 0)).toBeLessThan(0)
    // 3 × 3 wet samples touch 4 × 4 = 16 cells, each 5 m cell cut into 4 × 4 for the waves, two triangles each.
    expect(mesh.geometry.indexCount).toBe(16 * 16 * 6)
    disposeWater(group)
  })

  it('keeps a fine grid as it is', () => {
    const { ground, water } = pond()
    const group = buildWater({ water, streams: [] }, { ...ground, cell: 1 })!
    expect((group.children[0] as Mesh).geometry.indexCount).toBe(16 * 6)
    disposeWater(group)
  })

  it('has no mesh for a tile without water', () => {
    const { ground } = pond()
    expect(buildWater({ water: new Float32Array(), streams: [] }, ground)).toBeNull()
  })

  it('lays a stream as a ribbon lifted over the ground, carrying its distance along', () => {
    const { ground } = pond()
    const group = buildWater({ water: new Float32Array(), streams: [{ width: 2, points: [[2, 2], [18, 2]] }] }, ground)!
    const mesh = group.children[0] as Mesh
    expect(mesh.geometry.positions[1]).toBeGreaterThan(2)
    // Flow is (along, across, is-stream) in custom's y, z and w.
    const custom = mesh.geometry.custom!
    expect(custom[3]).toBe(1)
    expect(custom[(mesh.geometry.vertexCount - 1) * 4 + 1]).toBeCloseTo(16)
  })
})
