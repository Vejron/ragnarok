import { describe, expect, it } from 'vitest'
import { Grass } from '@/render/grass'
import { buildRocks, carveRock, packRockAlpha } from '@/render/rocks'
import { shoreLevelAt, shoreOf } from '@/render/shore'
import { NO_WATER, TREE_ROCK_BOULDER, TREE_ROCK_SLAB, TREE_SPRUCE, type TileData, type TileRef, type Tree } from '@/world'

const tree = (x: number, z: number, kind: number, crown = 1, height = 1.2): Tree => ({ x, z, base: 0, height, crown, kind, solid: true, trunkRadius: crown * 0.8, trunkHeight: height * 0.8 })

describe('rocks', () => {
  it('carves a shape that fills the unit space trees use, with a flat face a triangle', () => {
    const rock = carveRock({ cuts: 5, depth: [0.5, 0.8], top: 0.5, squash: 0.7, bump: 0.05 }, 3)
    const p = rock.positions
    let low = Infinity, high = -Infinity, wide = 0
    for (let i = 0; i < p.length; i += 3) {
      low = Math.min(low, p[i + 1]!)
      high = Math.max(high, p[i + 1]!)
      wide = Math.max(wide, Math.hypot(p[i]!, p[i + 2]!))
    }
    expect(low).toBeCloseTo(-0.25, 5)
    expect(high).toBeCloseTo(1, 5)
    expect(wide).toBeCloseTo(1, 5)
    expect(rock.indices).toBeUndefined()
    const n = rock.normals!
    expect([n[0], n[1], n[2]]).toEqual([n[3], n[4], n[5]])
    // The same seed carves the same rock.
    expect(carveRock({ cuts: 5, depth: [0.5, 0.8], top: 0.5, squash: 0.7, bump: 0.05 }, 3).positions).toEqual(p)
  })

  it('moss rocks in the forest and leaves those in the open bare', () => {
    const forest = Array.from({ length: 30 }, (_, i) => tree((i % 6) * 3 - 8, Math.floor(i / 6) * 3 - 8, TREE_SPRUCE, 2, 15))
    const rocks = [tree(0, 0, TREE_ROCK_BOULDER), tree(200, 200, TREE_ROCK_BOULDER), tree(0, 1, TREE_ROCK_SLAB), tree(201, 200, TREE_ROCK_SLAB)]
    const trees = [...forest, ...rocks]
    const meshes = buildRocks(
      trees,
      rocks.map((t) => ({ tree: t, index: trees.indexOf(t) })),
    )
    const moss = new Map<number, number>()
    for (const mesh of meshes) {
      const slots = mesh.userData.treeSlots as Map<number, number>
      // No water here: the alpha is the moss, a quarter up (`packRockAlpha`).
      for (const [index, slot] of slots) moss.set(index, mesh.colors[slot * 4 + 3]! - 0.25)
    }
    expect(moss.size).toBe(4)
    expect(meshes.reduce((n, m) => n + m.count, 0)).toBe(4)
    for (const [inForest, inOpen] of [[30, 31], [32, 33]] as const) {
      expect(moss.get(inForest)!).toBeGreaterThan(0.55)
      expect(moss.get(inOpen)!).toBeLessThan(0.2)
    }
  })

  it('keep the grass from growing through them', () => {
    const grass = new Grass(50, 100, () => 0)
    const tile = { cover: [], roads: [], water: new Float32Array(), buildings: [], streams: [], trees: [tree(10, 10, TREE_ROCK_BOULDER, 1.5)] } as unknown as TileData
    grass.addTile({ col: 0, row: 0 } as TileRef, tile, null)
    expect(grass.growsAt(10, 10)).toBe(0)
    expect(grass.growsAt(10.9, 10)).toBe(0)
    expect(grass.growsAt(13, 10)).not.toBe(0)
    grass.removeTile({ col: 0, row: 0 })
    grass.addTile({ col: 0, row: 0 } as TileRef, { ...tile, trees: [] }, null)
    expect(grass.growsAt(10, 10)).not.toBe(0)
  })

  it('know the level of the water near them, out to a reach, and nothing past it', () => {
    // 41 samples a metre apart; a sea 0.5 m deep along the west edge, its surface at 0.2 m.
    const side = 41
    const heights = new Float32Array(side * side)
    const water = new Float32Array(side * side).fill(NO_WATER)
    for (let j = 0; j < side; j++) {
      heights[j * side] = -0.3
      water[j * side] = 0.2
    }
    const ground = { x0: 100, z0: 200, cell: 1, side, heights }
    const shore = shoreOf(ground, water, 20)!
    expect(shoreLevelAt(shore, 100.5, 210)).toBeCloseTo(0.2)
    expect(shoreLevelAt(shore, 110, 210)).toBeCloseTo(0.2)
    expect(shoreLevelAt(shore, 135, 210)).toBeNaN()
    expect(shoreOf(ground, new Float32Array(side * side).fill(NO_WATER))).toBeNull()
  })

  it('pack their moss and the water by them into one number the shader takes apart', () => {
    const unpack = (a: number) => {
      const packed = Math.floor(a * 0.5)
      return [Math.min(1, Math.max(0, a - 2 * packed - 0.25)), packed > 0.5 ? packed / 20 - 1000 : null]
    }
    expect(unpack(packRockAlpha(0.6, 3.15))).toEqual([expect.closeTo(0.6, 3), expect.closeTo(3.15, 3)])
    expect(unpack(packRockAlpha(0, -0.4))).toEqual([expect.closeTo(0, 3), expect.closeTo(-0.4, 3)])
    expect(unpack(packRockAlpha(1, NaN))).toEqual([expect.closeTo(1, 3), null])
    // As a float32 on the GPU, a little off as it is interpolated, it still comes apart the same.
    const a = Math.fround(packRockAlpha(0, 3.15)) * (1 - 1e-6)
    expect(unpack(a)[1]).toBeCloseTo(3.15, 3)
  })
})
