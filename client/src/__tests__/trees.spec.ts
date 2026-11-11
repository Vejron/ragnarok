import { describe, expect, it } from 'vitest'
import { InstancedMesh } from '@ragnarok/engine'
import { TREE_LEVELS, TREE_LEVELS_FROM, buildTrees, treeGeometries } from '@/render/trees'
import { TREE_BIRCH, TREE_BROADLEAF, TREE_BUSH, TREE_COLUMNAR, TREE_FLAG, TREE_LOWWIDE, TREE_ORNAMENTAL, TREE_PINE, TREE_SPRUCE, type Tree } from '@/world'

const KINDS = [TREE_BIRCH, TREE_PINE, TREE_SPRUCE, TREE_ORNAMENTAL, TREE_BROADLEAF, TREE_COLUMNAR, TREE_LOWWIDE, TREE_BUSH, TREE_FLAG]

describe('tree levels of detail', () => {
  it('gives every kind a shape a level, each with fewer triangles than the one before', () => {
    for (const kind of KINDS) {
      const shapes = treeGeometries(kind)
      expect(shapes).toHaveLength(TREE_LEVELS)
      for (let level = 1; level < shapes.length; level++) {
        expect(shapes[level]!.indexCount, `kind ${kind} level ${level}`).toBeLessThan(shapes[level - 1]!.indexCount)
      }
    }
  })

  it('draws a far tree with a sixth of a near one\'s triangles or fewer', () => {
    for (const kind of [TREE_BIRCH, TREE_PINE, TREE_BROADLEAF, TREE_BUSH]) {
      const shapes = treeGeometries(kind)
      expect(shapes[TREE_LEVELS - 1]!.indexCount * 6).toBeLessThanOrEqual(shapes[0]!.indexCount)
    }
  })

  it('keeps a coarser shape about where and as big as the full one, so it is culled by the same bounds', () => {
    for (const kind of KINDS) {
      const [full, ...coarser] = treeGeometries(kind)
      for (const shape of coarser) {
        const apart = Math.hypot(shape.bounds.x - full!.bounds.x, shape.bounds.y - full!.bounds.y, shape.bounds.z - full!.bounds.z)
        expect(apart + shape.bounds.radius, `kind ${kind}`).toBeLessThanOrEqual(full!.bounds.radius + 1e-6)
        expect(shape.bounds.radius, `kind ${kind}`).toBeGreaterThan(full!.bounds.radius * 0.6)
      }
    }
  })

  it('hands a tile\'s trees their coarser shapes, nearest first, and casts shadows from a coarser one', () => {
    const tree: Tree = { x: 0, z: 0, base: 0, height: 10, crown: 3, kind: TREE_PINE, solid: true, trunkRadius: 0.2, trunkHeight: 8 }
    const [mesh] = buildTrees([tree])
    expect(mesh).toBeInstanceOf(InstancedMesh)
    expect(mesh!.levels!.map((level) => level.from)).toEqual(TREE_LEVELS_FROM)
    expect(mesh!.shadowLevel).toBeGreaterThan(0)
  })
})
