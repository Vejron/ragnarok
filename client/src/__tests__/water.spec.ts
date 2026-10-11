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
    const mesh = group.children[0] as import('three').Mesh
    const depth = mesh.geometry.getAttribute('depth')
    expect(depth.getX(2 * 5 + 2)).toBeCloseTo(1)
    expect(depth.getX(0)).toBeLessThan(0)
    // 3 × 3 wet samples touch 4 × 4 = 16 cells, two triangles each.
    expect(mesh.geometry.getIndex()!.count).toBe(16 * 6)
    disposeWater(group)
  })

  it('has no mesh for a tile without water', () => {
    const { ground } = pond()
    expect(buildWater({ water: new Float32Array(), streams: [] }, ground)).toBeNull()
  })

  it('lays a stream as a ribbon lifted over the ground, carrying its distance along', () => {
    const { ground } = pond()
    const group = buildWater({ water: new Float32Array(), streams: [{ width: 2, points: [[2, 2], [18, 2]] }] }, ground)!
    const mesh = group.children[0] as import('three').Mesh
    const position = mesh.geometry.getAttribute('position')
    expect(position.getY(0)).toBeGreaterThan(2)
    const flow = mesh.geometry.getAttribute('flow')
    expect(flow.getZ(0)).toBe(1)
    expect(flow.getX(flow.count - 1)).toBeCloseTo(16)
  })
})
