import { describe, expect, it } from 'vitest'
import { InstancedMesh } from '@ragnarok/engine'
import { TREE_FADE, TREE_LEVELS, TREE_LEVELS_FROM, buildTrees, treeGeometries } from '@/render/trees'
import { TREE_BIRCH, TREE_BROADLEAF, TREE_BUSH, TREE_COLUMNAR, TREE_FLAG, TREE_LOWWIDE, TREE_ORNAMENTAL, TREE_PINE, TREE_SPRUCE, type Tree } from '@/world'

const KINDS = [TREE_BIRCH, TREE_PINE, TREE_SPRUCE, TREE_ORNAMENTAL, TREE_BROADLEAF, TREE_COLUMNAR, TREE_LOWWIDE, TREE_BUSH, TREE_FLAG]

describe('tree levels of detail', () => {
  it('draws a far tree as one quad, marked as an impostor and turned by the shader', () => {
    const tiles = new Set<number>()
    for (const kind of KINDS) {
      const { impostor } = treeGeometries(kind)
      expect(impostor.indexCount, `kind ${kind}`).toBe(6)
      expect(impostor.uvs, `kind ${kind}`).toBeDefined()
      const { full } = treeGeometries(kind)
      for (let i = 0; i < impostor.vertexCount; i++) {
        // Its pictures' half width, and the upright cylinder it is fitted to, which says it is an impostor.
        const [extent, , radius, half] = Array.from(impostor.custom!.subarray(i * 4, i * 4 + 4))
        expect(half, `kind ${kind}`).toBeGreaterThan(0)
        expect(radius, `kind ${kind}`).toBeGreaterThan(0)
        // The cylinder holds the solid, and fits in the sphere the pictures frame.
        for (let v = 0; v < full.vertexCount; v++) {
          const p = full.positions
          expect(Math.hypot(p[v * 3]! - full.bounds.x, p[v * 3 + 2]! - full.bounds.z), `kind ${kind}`).toBeLessThanOrEqual(radius! + 1e-6)
          expect(Math.abs(p[v * 3 + 1]! - full.bounds.y), `kind ${kind}`).toBeLessThanOrEqual(half! + 1e-6)
        }
        expect(Math.max(radius!, half!), `kind ${kind}`).toBeLessThanOrEqual(extent!)
      }
      tiles.add(impostor.custom![1]!)
    }
    // Each kind has its own pictures in the atlas.
    expect(tiles.size).toBe(KINDS.length)
  })

  it('tells the shader where the middle of a solid is, the same middle it is culled and faded by', () => {
    for (const kind of KINDS) {
      const { full } = treeGeometries(kind)
      for (let i = 0; i < full.vertexCount; i++) {
        expect(full.custom![i * 4 + 3], `kind ${kind}`).toBe(0)
        expect(full.custom![i * 4], `kind ${kind}`).toBeCloseTo(full.bounds.x)
        expect(full.custom![i * 4 + 1], `kind ${kind}`).toBeCloseTo(full.bounds.y)
        expect(full.custom![i * 4 + 2], `kind ${kind}`).toBeCloseTo(full.bounds.z)
      }
    }
  })

  it('casts shadows from ever fewer triangles, and keeps every shape inside the bounds it is culled by', () => {
    for (const kind of KINDS) {
      const { full, coarse, shadow } = treeGeometries(kind)
      expect(coarse.indexCount, `kind ${kind}`).toBeLessThan(full.indexCount)
      expect(shadow.indexCount, `kind ${kind}`).toBeLessThan(coarse.indexCount)
      for (const shape of [coarse, shadow]) {
        const apart = Math.hypot(shape.bounds.x - full.bounds.x, shape.bounds.y - full.bounds.y, shape.bounds.z - full.bounds.z)
        expect(apart + shape.bounds.radius, `kind ${kind}`).toBeLessThanOrEqual(full.bounds.radius + 1e-6)
        expect(shape.bounds.radius, `kind ${kind}`).toBeGreaterThan(full.bounds.radius * 0.6)
      }
    }
  })

  it('hands a tile\'s trees their impostor with a fade into it, and casts shadows from solids', () => {
    const tree: Tree = { x: 0, z: 0, base: 0, height: 10, crown: 3, kind: TREE_PINE, solid: true, trunkRadius: 0.2, trunkHeight: 8 }
    const [mesh] = buildTrees([tree])
    expect(mesh).toBeInstanceOf(InstancedMesh)
    const levels = mesh!.levels!
    expect(levels).toHaveLength(TREE_LEVELS - 1)
    expect(levels.map((level) => level.from)).toEqual(TREE_LEVELS_FROM)
    expect(levels[0]!.fade).toBe(TREE_FADE)
    // The view does not change at the second level: only the shadow does.
    expect(levels[1]!.geometry).toBe(levels[0]!.geometry)
    expect(levels[1]!.fade).toBeUndefined()
    expect(mesh!.shadowLevel).toBeGreaterThan(0)
    for (const level of levels) expect(level.shadow).toBeDefined()
  })
})
