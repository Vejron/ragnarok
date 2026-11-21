import { describe, expect, it } from 'vitest'
import type { Mesh } from '@ragnarok/engine'
import { FLOOR_HEATH, FLOOR_LEAF, FLOOR_MIXED, FLOOR_MOSS, floorAt, forestOf, standAt, woodsOf } from '@/render/forest'
import { FLOOR_FULL, FLOOR_LITE, FloorBuilder, ForestFloor, addFern, addStone, addStump, driftsAt } from '@/render/forestfloor'
import { chunkBlades, GRASS_CHUNK, random } from '@/render/grass'
import { FLOOR_SHADES, grassMask } from '@/render/grassmask'
import { COVER_WOOD, TREE_BIRCH, TREE_BUSH, TREE_PINE, TREE_ROCK_BOULDER, TREE_SPRUCE, type Point2, type Tree } from '@/world'

const square = (x0: number, z0: number, x1: number, z1: number): Point2[] => [[x0, z0], [x1, z0], [x1, z1], [x0, z1]]

/** A tree of `kind` every 4 m over the square, with a 2 m crown. */
function stand(kind: number, x0: number, z0: number, x1: number, z1: number): Tree[] {
  const trees: Tree[] = []
  for (let z = z0 + 2; z < z1; z += 4) for (let x = x0 + 2; x < x1; x += 4) trees.push({ x, z, base: 0, height: 12, crown: 2, kind, solid: true, trunkRadius: 0.15, trunkHeight: 6 })
  return trees
}

/** Every vertex of a layer's chunks, as `[x, y, z]`. */
function vertices(root: { children: readonly unknown[] }): number[][] {
  const out: number[][] = []
  for (const child of root.children as Mesh[]) {
    const p = child.geometry.positions
    for (let i = 0; i < p.length; i += 3) out.push([p[i]!, p[i + 1]!, p[i + 2]!])
  }
  return out
}

describe('the forest floor', () => {
  // Pine over the west half of a 128 m tile, spruce over the north-east quarter, birch over the south-east.
  const trees = [...stand(TREE_PINE, 0, 0, 64, 128), ...stand(TREE_SPRUCE, 64, 0, 128, 64), ...stand(TREE_BIRCH, 64, 64, 128, 128)]
  const forest = forestOf(woodsOf(trees), 0, 0, 128)

  it('is a pine heath under pine, moss under spruce and a birch wood under birch', () => {
    for (const pick of [0, 0.5, 0.99]) {
      expect(floorAt(forest, 24, 60, pick)).toBe(FLOOR_HEATH)
      expect(floorAt(forest, 100, 24, pick)).toBe(FLOOR_MOSS)
      expect(floorAt(forest, 100, 100, pick)).toBe(FLOOR_LEAF)
    }
  })

  it('is a patchwork of both floors where two stands meet', () => {
    const [pine, spruce] = standAt(forest, 64, 30, [0, 0, 0])
    expect(pine).toBeGreaterThan(0.3)
    expect(spruce).toBeGreaterThan(0.3)
    expect(floorAt(forest, 64, 30, 0.1)).toBe(FLOOR_HEATH)
    expect(floorAt(forest, 64, 30, 0.9)).toBe(FLOOR_MOSS)
  })

  it('has no stand to follow where no trees are, and counts neither rocks nor bushes as trees', () => {
    const lone = [{ x: 10, z: 10, crown: 1.5, kind: TREE_ROCK_BOULDER }, { x: 12, z: 10, crown: 1, kind: TREE_BUSH }]
    expect(woodsOf(lone)).toHaveLength(0)
    const bare = forestOf(woodsOf(lone), 0, 0, 128)
    expect(floorAt(bare, 10, 10, 0)).toBe(FLOOR_MIXED)
    // One young pine alone says little: most of the ground about it is still any wood's.
    const sapling = forestOf(woodsOf([{ x: 10, z: 10, crown: 1, kind: TREE_PINE }]), 0, 0, 128)
    expect(floorAt(sapling, 10, 10, 0.5)).toBe(FLOOR_MIXED)
  })

  it('gives a wood the grass of its stands, and other ground its own', () => {
    const tile = { cover: [{ kind: COVER_WOOD, rings: [square(0, 0, 128, 100)] }], roads: [], water: new Float32Array(), buildings: [], streams: [] }
    const mask = grassMask(tile, 0, 0, 128, null, undefined, forest)
    const at = (x: number, z: number) => mask.cells[Math.floor(z / 2) * mask.side + Math.floor(x / 2)]
    expect(at(24, 60)).toBe(FLOOR_SHADES[FLOOR_HEATH])
    expect(at(100, 24)).toBe(FLOOR_SHADES[FLOOR_MOSS])
    expect(at(100, 90)).toBe(FLOOR_SHADES[FLOOR_LEAF])
    // South of the wood the birches stand in a meadow.
    expect(at(100, 120)).toBe(1)
    // Without the stands a wood is a wood.
    expect(grassMask(tile, 0, 0, 128).cells[0]).toBe(FLOOR_SHADES[FLOOR_MIXED])
  })

  it('grows little grass in the moss under spruce and the most under birch', () => {
    const blades = (floor: number) => chunkBlades(3, 4, 12, 1, () => FLOOR_SHADES[floor]!, () => 0).length
    expect(blades(FLOOR_MOSS)).toBeLessThan(blades(FLOOR_HEATH))
    expect(blades(FLOOR_HEATH)).toBeLessThan(blades(FLOOR_MIXED))
    expect(blades(FLOOR_MIXED)).toBeLessThan(blades(FLOOR_LEAF))
    expect(blades(FLOOR_MOSS) * 5).toBeLessThan(blades(FLOOR_LEAF))
    expect(GRASS_CHUNK).toBe(16)
  })

  it('builds each thing about a root under it, which it shrinks into far off', () => {
    const out = new FloorBuilder()
    addFern(out, 5, 2, 7, [0, 1, 0], 0.25, random(1))
    addStump(out, { bark: { r: 1, g: 1, b: 1 }, mark: { r: 0, g: 0, b: 0 }, moss: 0.5 } as never, 9, 2, 3, random(2))
    addStone(out, 0.8, 20, 2, 20, 0.3, 0.2, random(4))
    const geometry = out.geometry()
    expect(Math.max(...geometry.indices!)).toBe(geometry.vertexCount - 1)
    expect(geometry.indices!.length % 3).toBe(0)
    const { positions, custom } = geometry
    for (let i = 0; i < geometry.vertexCount; i++) {
      // The height over the root is what `custom` says, and no root is far from its vertex.
      const rootY = positions[i * 3 + 1]! - custom![i * 4 + 2]!
      expect(rootY).toBeLessThan(2.6)
      expect(rootY).toBeGreaterThan(1.6)
      expect(Math.hypot(positions[i * 3]! - custom![i * 4]!, positions[i * 3 + 2]! - custom![i * 4 + 1]!)).toBeLessThan(1.2)
    }
    // The fern is the first thing in, and carries its rank.
    expect(custom![3]).toBe(0.25)
  })

  it('grows shrubs and ferns in drifts of their own', () => {
    let thick = 0, thin = 0
    for (let x = 0; x < 300; x += 3) {
      for (let z = 0; z < 300; z += 3) {
        const [shrubs] = driftsAt(x, z)
        if (shrubs > 0.9) thick++
        if (shrubs < 0.1) thin++
      }
    }
    expect(thick).toBeGreaterThan(1500)
    expect(thin).toBeGreaterThan(300)
    expect(driftsAt(12.5, 40)).toEqual(driftsAt(12.5, 40))
  })

  it('stands only on a forest floor, the same things every time, and fewer plants on a lower setting', () => {
    // Moss west of x = 0 and a road's bare ground east of it; the ground is not loaded north of z = -40.
    const shade = (x: number) => (x < 0 ? FLOOR_SHADES[FLOOR_MOSS]! : 0)
    const height = (_x: number, z: number) => (z < -40 ? null : 3)
    const build = (density = FLOOR_FULL) => {
      const floor = new ForestFloor(500, 250, height, shade)
      floor.setDensity(density)
      // A chunk or two a frame: enough frames for all of them.
      for (let frame = 0; frame < 200; frame++) floor.update(0, 0)
      const [plants, timber] = floor.root.children
      return { floor, plants: vertices(plants!), timber: vertices(timber!) }
    }
    const { floor, plants, timber } = build()
    expect(plants.length).toBeGreaterThan(5000)
    expect(timber.length).toBeGreaterThan(200)
    for (const [x, y, z] of [...plants, ...timber]) {
      // A fern's frond may reach a little past where its foot stands.
      expect(x!).toBeLessThan(1.5)
      expect(z!).toBeGreaterThan(-45)
      expect(y!).toBeGreaterThan(2.5)
      expect(y!).toBeLessThan(4.5)
    }
    expect(build().plants).toEqual(plants)
    const lite = build(FLOOR_LITE).plants
    expect(lite.length).toBeLessThan(plants.length * 0.5)
    expect(lite.length).toBeGreaterThan(0)

    // Under a winter's snow the plants are gone and the timber stays.
    floor.setPlants(false)
    floor.update(0, 0)
    expect(vertices(floor.root.children[0]!)).toHaveLength(0)
    expect(vertices(floor.root.children[1]!)).toEqual(timber)
  })

  it('stands no timber in a tree, a rock or a crater', () => {
    const floor = new ForestFloor(500, 250, () => 0, () => FLOOR_SHADES[FLOOR_MOSS]!)
    for (let frame = 0; frame < 200; frame++) floor.update(0, 0)
    const before = vertices(floor.root.children[1]!)
    // A boulder five metres across where a piece of timber stood: it and its neighbours make way.
    const [x, , z] = before[0]!
    floor.addTile({ col: Math.floor((x! + 500) / 250), row: Math.floor((z! + 500) / 250) }, { trees: [{ x: x!, z: z!, base: 0, height: 2, crown: 5, kind: TREE_ROCK_BOULDER, solid: true, trunkRadius: 5, trunkHeight: 2 }] })
    for (let frame = 0; frame < 200; frame++) floor.update(0, 0)
    const after = vertices(floor.root.children[1]!)
    expect(after.length).toBeLessThan(before.length)
    expect(after.every(([ax, , az]) => Math.hypot(ax! - x!, az! - z!) > 4)).toBe(true)
    const [cx, , cz] = after[0]!
    floor.dent(cx!, cz!, 6)
    for (let frame = 0; frame < 200; frame++) floor.update(0, 0)
    expect(vertices(floor.root.children[1]!).every(([ax, , az]) => Math.hypot(ax! - cx!, az! - cz!) > 5)).toBe(true)
  })
})
