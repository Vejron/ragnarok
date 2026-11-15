import { describe, expect, it } from 'vitest'
import { buildGraves, graveGeometry } from '@/render/graves'
import { GRAVE_CROSS, GRAVE_HEADSTONE, GRAVE_KINDS, GRAVE_LOW, type Grave } from '@/world'

/** The least and greatest of a geometry's vertices along each axis. */
function reach(positions: ArrayLike<number>): { min: number[]; max: number[] } {
  const [min, max] = [[Infinity, Infinity, Infinity], [-Infinity, -Infinity, -Infinity]]
  for (let i = 0; i < positions.length; i++) {
    min[i % 3] = Math.min(min[i % 3]!, positions[i]!)
    max[i % 3] = Math.max(max[i % 3]!, positions[i]!)
  }
  return { min, max }
}

describe('tombstones', () => {
  it('are each drawn to the size of their collider, sunk a little into the ground', () => {
    for (const def of GRAVE_KINDS) {
      const { min, max } = reach(graveGeometry(def).positions)
      expect(max[0], def.name).toBeCloseTo(def.halfWidth, 2)
      expect(min[0], def.name).toBeCloseTo(-def.halfWidth, 2)
      expect(max[2], def.name).toBeLessThanOrEqual(def.halfDepth + 0.01)
      expect(min[2], def.name).toBeGreaterThanOrEqual(-def.halfDepth - 0.01)
      expect(max[1], def.name).toBeCloseTo(def.height, 1)
      expect(max[1], def.name).toBeLessThanOrEqual(def.height + 0.01)
      expect(min[1], def.name).toBeLessThan(-0.2)
    }
  })

  it('stand a stone of its kind at each grave, turned to its heading', () => {
    const graves: Grave[] = [
      { x: 10, z: 20, base: 3, yaw: 0, kind: GRAVE_HEADSTONE },
      { x: 12, z: 20, base: 3.1, yaw: Math.PI / 2, kind: GRAVE_HEADSTONE },
      { x: 14, z: 20, base: 3.2, yaw: 0, kind: GRAVE_CROSS },
      { x: 16, z: 20, base: 3.3, yaw: 0, kind: GRAVE_LOW },
    ]
    const meshes = buildGraves(graves)
    expect(meshes.map((mesh) => [mesh.userData.graves, mesh.count])).toEqual([[GRAVE_HEADSTONE, 2], [GRAVE_CROSS, 1], [GRAVE_LOW, 1]])
    const stones = meshes[0]!
    // Each stands where the world put it, on its ground.
    expect(Array.from(stones.matrices.slice(12, 15))).toEqual([10, 3, 20])
    expect(Array.from(stones.matrices.slice(28, 31)).map((v) => Math.fround(v))).toEqual([12, 3.1, 20].map(Math.fround))
    // At a quarter turn its face, which looked along +Z, looks along +X, and its width lies along Z.
    const turned = stones.matrices.slice(16, 32)
    expect(turned[8]).toBeCloseTo(1)
    expect(turned[10]).toBeCloseTo(0)
    expect(turned[2]).toBeCloseTo(-1)
    // Each is culled on its own, casts a shadow, and shares its shape with every other tile.
    for (const mesh of meshes) {
      expect(mesh.levels).toEqual([])
      expect(mesh.castShadow).toBe(true)
      expect(mesh.userData.sharedGeometry).toBe(true)
    }
    expect(buildGraves(graves)[0]!.geometry).toBe(stones.geometry)
  })

  it('are of more than one stone, the same every time a tile is built', () => {
    const graves: Grave[] = Array.from({ length: 40 }, (_, i) => ({ x: i * 2.2, z: 5, base: 0, yaw: 0, kind: GRAVE_HEADSTONE }))
    const colors = (mesh: { colors: Float32Array }) => Array.from({ length: 40 }, (_, i) => Array.from(mesh.colors.slice(i * 4, i * 4 + 3)).map((v) => v.toFixed(2)).join())
    const first = colors(buildGraves(graves)[0]!)
    expect(new Set(first).size).toBeGreaterThan(10)
    expect(colors(buildGraves(graves)[0]!)).toEqual(first)
  })

  it('are none where a tile has no graveyard', () => {
    expect(buildGraves([])).toEqual([])
    expect(buildGraves(undefined)).toEqual([])
  })
})
