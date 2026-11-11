import { describe, expect, it } from 'vitest'
import { InstancedMesh } from '@ragnarok/engine'
import { TREE_LEVELS, TREE_LEVELS_FROM, buildTrees, treeGeometries } from '@/render/trees'
import { TREE_BIRCH, TREE_BROADLEAF, TREE_BUSH, TREE_COLUMNAR, TREE_FLAG, TREE_LOWWIDE, TREE_ORNAMENTAL, TREE_PINE, TREE_SPRUCE, type Tree } from '@/world'

const KINDS = [TREE_BIRCH, TREE_PINE, TREE_SPRUCE, TREE_ORNAMENTAL, TREE_BROADLEAF, TREE_COLUMNAR, TREE_LOWWIDE, TREE_BUSH, TREE_FLAG]

describe('tree levels of detail', () => {
  it('gives every kind a shape a level, the round ones each with fewer triangles than the one before', () => {
    for (const kind of KINDS) {
      const { levels } = treeGeometries(kind)
      expect(levels).toHaveLength(TREE_LEVELS)
      for (let level = 1; level < levels.length - 1; level++) {
        expect(levels[level]!.indexCount, `kind ${kind} level ${level}`).toBeLessThan(levels[level - 1]!.indexCount)
      }
    }
  })

  it('draws a far tree as a quad a part, turned and cut out by the shader', () => {
    for (const kind of KINDS) {
      const { levels } = treeGeometries(kind)
      const far = levels[TREE_LEVELS - 1]!
      expect(far.indexCount % 6, `kind ${kind}`).toBe(0)
      expect(far.indexCount, `kind ${kind}`).toBeLessThanOrEqual(24)
      expect(far.uvs, `kind ${kind}`).toBeDefined()
      // Every corner says what it is a part of, so none is drawn as plain geometry.
      for (let i = 0; i < far.vertexCount; i++) expect(far.custom![i * 4 + 2], `kind ${kind}`).toBeGreaterThan(0)
    }
  })

  it('casts a far tree\'s shadow from a few triangles, and keeps every shape inside the bounds it is culled by', () => {
    for (const kind of KINDS) {
      const { levels: [full, coarse], shadow } = treeGeometries(kind)
      expect(shadow.indexCount * 4, `kind ${kind}`).toBeLessThanOrEqual(full!.indexCount)
      for (const shape of [coarse!, shadow]) {
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
    expect(mesh!.levels![mesh!.levels!.length - 1]!.shadow).toBeDefined()
  })
})
