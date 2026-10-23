import { describe, expect, it } from 'vitest'
import { Group, InstancedMesh, Mesh } from '@ragnarok/engine'
import * as THREE from 'three'
import { encode } from '@msgpack/msgpack'
import { decodeServer, OBJ_TREE } from '@/net/codec'
import { BuildingDamage } from '@/render/damage'
import { buildTrees } from '@/render/trees'
import { FallingTrees } from '@/render/falling'
import { TREE_PINE, type TileData, type Tree } from '@/world'
import fixture from './messages.fixture.json'

const tree = (x: number, solid: boolean): Tree => ({ x, z: 0, base: 0, height: 16, crown: 3, kind: TREE_PINE, solid, trunkRadius: 0.2, trunkHeight: 8 })

describe('destroyed message', () => {
  it('decodes the bytes the server sends', () => {
    // `messages.fixture.json` holds every message as the Rust tests encode it.
    expect(decodeServer(new Uint8Array(fixture.destroyed))).toEqual({
      type: 'destroyed',
      objects: [{ kind: OBJ_TREE, col: 2, row: 3, index: 70000, yaw: -1.571, force: 0.2 }],
    })
  })

  it('refuses entries of the wrong shape', () => {
    // [6, [[0, 1, 2]]]
    expect(decodeServer(new Uint8Array([146, 6, 145, 147, 0, 1, 2]))).toBeNull()
  })
})

describe('felling a tree', () => {
  const scene = () => {
    const group = new Group()
    group.add(...buildTrees([tree(0, true), tree(10, true)]))
    return group
  }

  it('hides its instance once, and only that one', () => {
    const group = scene()
    const falling = new FallingTrees()
    expect(falling.fell(group, tree(0, true), 0, 0, false)).toBe(true)
    expect(falling.fell(group, tree(0, true), 0, 0, false)).toBe(false)
    const mesh = group.children[0] as InstancedMesh
    // A felled instance is scaled to nothing: its x axis has no length.
    expect(mesh.matrices[0]).toBe(0)
    expect(mesh.matrices[16]).not.toBe(0)
  })

  it('tips the crown towards the way it falls', () => {
    // yaw -π/2 faces +X.
    const group = scene()
    const falling = new FallingTrees()
    const noises: string[] = []
    falling.onNoise = (noise) => noises.push(noise.phase)
    falling.fell(group, tree(0, true), 0, -Math.PI / 2, true)
    const fx = { smoke: { spawn: () => {} } }
    for (let i = 0; i < 600; i++) falling.update(0.01, fx)
    // It cracks as it starts and thuds as it lands, once each.
    expect(noises).toEqual(['hit', 'crack', 'thud'])
    const pivot = group.children[group.children.length - 1]!
    const top = new THREE.Vector3(0, 1, 0).applyQuaternion(pivot.quaternion)
    expect(top.x).toBeGreaterThan(0.95)
    expect(Math.abs(top.z)).toBeLessThan(0.05)
    expect(top.y).toBeLessThan(0.1)
  })

  it('comes down much faster the harder it was hit', () => {
    const landedAfter = (force: number) => {
      const group = scene()
      const falling = new FallingTrees()
      let landed = -1
      falling.onNoise = ({ phase }) => {
        if (phase === 'thud') landed = t
      }
      falling.fell(group, tree(0, true), 0, 0, true, force)
      const fx = { smoke: { spawn: () => {} } }
      let t = 0
      for (; t < 5 && landed < 0; t += 0.01) falling.update(0.01, fx)
      return landed
    }
    expect(landedAfter(1)).toBeLessThan(landedAfter(0) / 2)
  })

  it('lies down at once when told it already fell', () => {
    const group = scene()
    new FallingTrees().fell(group, tree(0, true), 0, 0, false)
    const top = new THREE.Vector3(0, 1, 0).applyQuaternion(group.children[group.children.length - 1]!.quaternion)
    // yaw 0 faces -Z.
    expect(top.z).toBeLessThan(-0.95)
  })
})

// ---- buildings

import { Effects } from '@/render/effects'
import { Collapses } from '@/render/rubble'
import { buildTile, rebuildBuildings } from '@/render/world'
import type { SimMotor } from '@/sim/motor'
import { flatMotor, tileBytes } from './worlds'
import { shellOf } from '@/sim/carve'
import { Fracture } from '@/render/fracture'
import { buildingTriangleRanges, ROOF_FLAT, type Building } from '@/world'

/** A closed box mesh from `x0` to `x0 + 10` metres, 6 m tall: 12 triangles, 8 of them walls. */
function box(x0: number, vertexBase: number) {
  const v = (x: number, y: number, z: number) => [x0 + x, y, z]
  const vertices = [v(0, 0, -5), v(10, 0, -5), v(10, 0, 5), v(0, 0, 5), v(0, 6, -5), v(10, 6, -5), v(10, 6, 5), v(0, 6, 5)]
  const triangles = [
    [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
    [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
  ].map((t) => t.map((i) => i! + vertexBase))
  return { vertices, triangles }
}

function twoHouses(): TileData {
  const [a, b] = [box(0, 0), box(30, 8)]
  const building = (x0: number, first: number): Building => ({
    height: 6, base: 0, first, walls: 8, roof: 2, roofKind: ROOF_FLAT, cells: null,
    footprint: [[x0, -5], [x0 + 10, -5], [x0 + 10, 5], [x0, 5]],
  })
  return {
    buildings: [building(0, 0), building(30, 12)],
    roads: [], cover: [], trees: [], streams: [], bridges: [],
    collision: { vertices: Float32Array.from([...a.vertices, ...b.vertices].flat()), triangles: Uint32Array.from([...a.triangles, ...b.triangles].flat()) },
    ground: new Float32Array(0), water: new Float32Array(0),
  } as unknown as TileData
}

describe('building triangle ranges', () => {
  it('runs each building up to the next one or bridge, floor included (as WorldFile::assemble)', () => {
    // The Rust test `assemble_names_each_building_and_its_merged_triangles`'s first tile:
    // two buildings at 0 and 4, a bridge at 6, 8 triangles.
    const tile = {
      buildings: [{ first: 0 }, { first: 4 }],
      bridges: [{ first: 6 }],
      collision: { triangles: new Uint32Array(8 * 3) },
    } as unknown as TileData
    expect(buildingTriangleRanges(tile)).toEqual([[0, 4], [4, 6]])
    expect(buildingTriangleRanges(twoHouses())).toEqual([[0, 12], [12, 24]])
  })
})

describe('redrawing a tile without a building', () => {
  const vertexCount = (group: Group) =>
    group.children.reduce((n, c) => n + (c instanceof Mesh && c.userData.merged ? c.geometry.vertexCount : 0), 0)

  it('drops that building\'s triangles from the merged meshes and no others', () => {
    const tile = twoHouses()
    const group = buildTile(tile, 0)
    // Each house is drawn as its own cubes.
    const both = vertexCount(group)
    rebuildBuildings(group, tile, 0, new Set([0]))
    const second = vertexCount(group)
    rebuildBuildings(group, tile, 0, new Set([1]))
    const first = vertexCount(group)
    expect(first).toBeGreaterThan(0)
    expect(second).toBeGreaterThan(0)
    expect(first + second).toBe(both)
    rebuildBuildings(group, tile, 0, new Set([0, 1]))
    expect(vertexCount(group)).toBe(0)
  })
})

describe('the building collider without a fallen building', () => {
  /** The tile in a world of its own, solid to the server's physics (in WASM). */
  async function solidTile(tile: TileData): Promise<SimMotor> {
    const { vertices, triangles } = tile.collision
    const motor = await flatMotor()
    motor.addTile(
      { col: 0, row: 0 },
      tileBytes({
        vertices: Array.from({ length: vertices.length / 3 }, (_, i) => [vertices[i * 3]!, vertices[i * 3 + 1]!, vertices[i * 3 + 2]!]),
        triangles: Array.from({ length: triangles.length / 3 }, (_, i) => [triangles[i * 3]!, triangles[i * 3 + 1]!, triangles[i * 3 + 2]!]),
        buildings: tile.buildings,
      }),
    )
    return motor
  }
  const along = (motor: SimMotor, x: number, y = 1, z = 0) => motor.castRay([x, y, z], [1, 0, 0], 5)

  it('keeps the other house and loses that one', async () => {
    const motor = await solidTile(twoHouses())
    motor.setBuildings(0, 0, [0], new Map())
    // Rays start 2 m before each house's west wall.
    expect(along(motor, -2)).toBeNull()
    expect(along(motor, 28)).not.toBeNull()
  })

  // The cells are the ones the client draws (`carve.ts`); the server's physics cuts them out of its own shell.
  it('opens the wall where cells were blasted out and leaves the rest of that wall', async () => {
    const tile = twoHouses()
    const shell = shellOf(tile, 0)!
    const hole = new Set<number>()
    for (let y = 0; y <= 2.5; y += 0.5) for (let z = -1.5; z <= 1.5; z += 0.5) hole.add(shell.cellAt(0, y, z))
    const motor = await solidTile(tile)
    motor.setBuildings(0, 0, [], new Map([[0, hole]]))
    expect(along(motor, -2, 1, 0)).toBeNull()
    expect(along(motor, -2, 1, 4)).not.toBeNull()
    // The other house and this one's floor are as they were.
    expect(along(motor, 28, 1, 0)).not.toBeNull()
  })

  it('is the cubes the house is drawn as, not its own walls and roof', async () => {
    // A house whose walls and roof fall between the cells: 5.4 m tall, its west wall at x = 0.3.
    const tile = twoHouses()
    const v = tile.collision.vertices
    for (let i = 0; i < 24; i += 3) {
      v[i] = v[i]! + 0.3
      if (v[i + 1] === 6) v[i + 1] = 5.4
    }
    const motor = await solidTile(tile)
    // Standing on the roof is standing on the top of its cubes, 5.76 m up, as drawn.
    const down = motor.castRay([5, 20, 0], [0, -1, 0], 30)!
    expect(20 - down.t).toBeCloseTo(5.76, 4)
    // And walking into the wall stops at the cubes' face, x = 0.
    expect(along(motor, -2)!.t - 2).toBeCloseTo(0, 4)
  })

  it('is nothing at all once every building is gone', async () => {
    const motor = await solidTile(twoHouses())
    motor.setBuildings(0, 0, [0, 1], new Map())
    expect(along(motor, -2)).toBeNull()
    expect(along(motor, 28)).toBeNull()
    // Down to the ground, with no floor left either.
    expect(20 - motor.castRay([5, 20, 0], [0, -1, 0], 30)!.t).toBeCloseTo(0, 4)
  })
})

describe('a building collapsing', () => {
  it('sinks its walls, leaves a pile of rubble in the tile, and clears away its pieces', () => {
    const tile = twoHouses()
    const group = new Group()
    // A tile's group hangs in the world; the collapse stops when it is taken out.
    new Group().add(group)
    const effects = new Effects()
    const collapses = new Collapses()
    collapses.start(group, tile, 0, 0, 0, () => 0, 7)
    // The building's walls, the pile and the flung pieces.
    expect(group.children.length).toBe(3)
    const body = group.children[0]!
    for (let i = 0; i < 20; i++) collapses.update(0.05, effects)
    // A second in of 1.8, easing in: past 0.5 of the way down, not yet flat.
    expect(body.scale.y).toBeLessThan(0.75)
    expect(body.scale.y).toBeGreaterThan(0.3)
    for (let i = 0; i < 300; i++) collapses.update(0.05, effects)
    // Only the pile is left.
    expect(group.children.length).toBe(1)
    const heap = group.children[0] as Mesh
    expect(heap.scale.y).toBeGreaterThanOrEqual(1)
    expect(heap.scale.y).toBeLessThanOrEqual(3)
    effects.dispose()
  })

  it('is just the pile when the tile came in after it fell', () => {
    const group = new Group()
    Collapses.settled(group, twoHouses(), 1)
    expect(group.children.length).toBe(1)
    expect(group.children[0]!.position.x).toBeCloseTo(35)
  })
})

describe('damaged message', () => {
  it('decodes the bytes the server sends, with a mark and without', () => {
    expect(decodeServer(new Uint8Array(fixture.damaged))).toEqual({
      type: 'damaged',
      buildings: [{ col: 2, row: 3, index: 70000, stage: 3, mark: [10, 0.2, -10] }, { col: 0, row: 1, index: 4, stage: 1, mark: null }],
    })
  })

  it('refuses a mark that is not a point', () => {
    expect(decodeServer(encode([7, [[2, 3, 4, 1, [0, 0]]]]))).toBeNull()
  })
})

describe('a hurt building', () => {
  const key = 5
  const tileWith = () => twoHouses()
  const bodyOf = (damage: BuildingDamage) =>
    damage.root.children[0]!.children.find((c) => c instanceof Mesh && c.castShadow && c.geometry.colors) as Mesh | undefined
  const cellAt = (index: number, x: number, y: number, z: number) => shellOf(tileWith(), index)!.cellAt(x, y, z)

  it('keeps the tile drawing a building that is only hurt', () => {
    const damage = new BuildingDamage()
    damage.attach(key, tileWith())
    damage.hurtBuilding(key, 0, 3)
    expect(damage.fractured(key)).toEqual([])
    expect(bodyOf(damage)).toBeUndefined()
  })

  it('burns the wall around a hole darker than the rest', () => {
    const damage = new BuildingDamage()
    damage.attach(key, tileWith())
    damage.carve(key, 0, [cellAt(0, 0, 2, 0)], [], [-1, 2, 0], false)
    const colors = Array.from(bodyOf(damage)!.geometry.colors!)
    const brightness = (i: number) => colors[i * 3]! + colors[i * 3 + 1]! + colors[i * 3 + 2]!
    const all = Array.from({ length: colors.length / 3 }, (_, i) => brightness(i))
    expect(Math.min(...all)).toBeLessThan(Math.max(...all) * 0.6)
  })

  it('burns a joiner’s walls the same, from the removed cells alone', () => {
    const colors = (live: boolean) => {
      const damage = new BuildingDamage()
      damage.attach(key, tileWith())
      damage.carve(key, 0, [cellAt(0, 0, 2, 0)], [], live ? [-1, 2, 0] : null, live)
      return Array.from(bodyOf(damage)!.geometry.colors!)
    }
    expect(colors(true)).toEqual(colors(false))
  })

  it('has the same walls in the same places for everyone', () => {
    const positions = () => {
      const damage = new BuildingDamage()
      damage.attach(key, tileWith())
      damage.carve(key, 1, [cellAt(1, 30, 2, 0), cellAt(1, 30, 3, 1)], [], [28, 2, 0], false)
      damage.dent(key, 1, cellAt(1, 40, 2, 0), 24)
      return Array.from(bodyOf(damage)!.geometry.positions)
    }
    expect(positions()).toEqual(positions())
  })

  it('lets a piece that lost its hold tip towards the gap, not straight down', () => {
    const tile = tileWith()
    const shell = shellOf(tile, 0)!
    // A solid cell, and the next one along the wall that is solid too: that one is cut out, this one hangs on.
    const hung = shell.cellAt(0, 2, 0)
    const [hx, hy, hz] = shell.coords(hung)
    const gap = [shell.index(hx + 1, hy, hz), shell.index(hx - 1, hy, hz), shell.index(hx, hy, hz + 1), shell.index(hx, hy, hz - 1)].find((id) => shell.isSolid(id))!
    const [gx, , gz] = shell.coords(gap)
    const damage = new BuildingDamage()
    damage.attach(key, tile)
    damage.carve(key, 0, [gap], [hung], [0, 2, 0], true)
    const pieces = damage.root.children[0]!.children.filter((c) => c instanceof Mesh && c.geometry.colors && c.position.lengthSq() > 0) as Mesh[]
    const start = pieces.map((m) => m.position.clone())
    for (let i = 0; i < 30; i++) damage.update(0.02, { dust() {} } as never, new THREE.Vector3(1e6, 0, 0))
    const moved = pieces.map((m, i) => m.position.clone().sub(start[i]!))
    const towards = moved.some((d) => d.x * Math.sign(gx - hx) + d.z * Math.sign(gz - hz) > 0.2)
    expect(towards).toBe(true)
  })

  it('is cleared when it collapses, and not drawn again when the tile returns', () => {
    const damage = new BuildingDamage()
    damage.attach(key, tileWith())
    damage.hurtBuilding(key, 0, 2)
    damage.carve(key, 0, [cellAt(0, 0, 2, 0)], [], [-2, 2, 0], true)
    damage.fell(key, 0)
    expect(damage.root.children.length).toBe(0)
    expect(damage.fractured(key)).toEqual([])
    damage.detach(key)
    damage.attach(key, tileWith())
    expect(damage.shown(key, 0)).toEqual({ cells: 0 })
  })

  it('is hurt as it was when its tile streams back in', () => {
    const damage = new BuildingDamage()
    damage.attach(key, tileWith())
    damage.hurtBuilding(key, 0, 2)
    damage.carve(key, 0, [cellAt(0, 0, 2, 0)], [], [-1, 2, 0], true)
    const before = damage.shown(key, 0)
    const body = Array.from(bodyOf(damage)!.geometry.positions)
    damage.detach(key)
    expect(damage.root.children.length).toBe(0)
    expect(damage.fractured(key)).toEqual([0])
    damage.attach(key, tileWith())
    expect(damage.shown(key, 0)).toEqual(before)
    expect(Array.from(bodyOf(damage)!.geometry.positions)).toEqual(body)
  })

  it('remembers damage that came before its tile did', () => {
    const damage = new BuildingDamage()
    expect(damage.carve(key, 1, [cellAt(1, 30, 2, 0)], [], null, false)).toBe(true)
    expect(damage.root.children.length).toBe(0)
    damage.attach(key, tileWith())
    expect(damage.shown(key, 1).cells).toBe(1)
    expect(bodyOf(damage)).toBeDefined()
  })
})

describe('carved message', () => {
  it('decodes the bytes the server sends', () => {
    expect(decodeServer(new Uint8Array(fixture.carved))).toEqual({
      type: 'carved',
      live: true,
      buildings: [{ col: 2, row: 3, index: 70000, point: [10, 0.2, -10], blasted: [5, 300], fell: [9] }],
    })
  })

  it('refuses a cell list that is not numbers', () => {
    expect(decodeServer(encode([10, true, [[1, 2, 3, [0, 0, 0], ['x'], []]]]))).toBeNull()
  })
})

describe('a building with cells blasted out of it', () => {
  const key = 5
  /** The building's standing cells are drawn as several bricks (they receive shadow, the thrown pieces do not); this is them as one mesh. */
  const bodyOf = (damage: BuildingDamage) => {
    const bricks = damage.root.children[0]!.children.filter((c) => c instanceof Mesh && c.receiveShadow && c.geometry.colors) as Mesh[]
    const join = (pick: (m: Mesh) => ArrayLike<number>) => Float32Array.from(bricks.flatMap((m) => Array.from(pick(m))))
    const positions = join((m) => m.geometry.positions)
    return { geometry: { positions, colors: join((m) => m.geometry.colors!), vertexCount: positions.length / 3 } }
  }
  const effects = () => new Effects()

  it('is drawn as pieces from the first bite, and says so only once', () => {
    const damage = new BuildingDamage()
    damage.attach(key, twoHouses())
    expect(damage.carve(key, 0, [1, 2, 3], [], [-1, 2, 0], true)).toBe(true)
    expect(damage.carve(key, 0, [4], [], [-1, 2, 0], true)).toBe(false)
    expect(damage.fractured(key)).toEqual([0])
    expect(Array.from(damage.carved(key).get(0)!).sort()).toEqual([1, 2, 3, 4])
  })

  it('leaves out the triangles of the cells that went', () => {
    const damage = new BuildingDamage()
    damage.attach(key, twoHouses())
    damage.dent(key, 0, 0, 1)
    const whole = bodyOf(damage).geometry.vertexCount
    // Every cell of the house: nothing of its walls is drawn.
    const all = Array.from({ length: 64_000 }, (_, i) => i)
    damage.carve(key, 0, all, [], [-1, 2, 0], true)
    // A hit on a building already drawn as shards is drawn at the next frame.
    const e = effects()
    damage.update(0, e, new THREE.Vector3(500, 0, 0))
    e.dispose()
    expect(bodyOf(damage).geometry.vertexCount).toBeLessThan(whole)
  })

  it('throws what was blasted out, and drops what was left hanging', () => {
    const damage = new BuildingDamage()
    damage.attach(key, twoHouses())
    damage.dent(key, 0, 0, 1)
    const group = damage.root.children[0]!
    const before = group.children.length
    const all = Array.from({ length: 4000 }, (_, i) => i)
    damage.carve(key, 0, all.slice(0, 20), all.slice(20, 60), [-1, 2, 0], true)
    expect(group.children.length).toBeGreaterThan(before)
    const e = effects()
    for (let i = 0; i < 400; i++) damage.update(0.05, e, new THREE.Vector3(500, 0, 0))
    expect(group.children.length).toBeLessThanOrEqual(before)
    e.dispose()
  })

  it('darkens a piece of wall as it takes rounds, without shrinking it, and drops the dent when it breaks out', () => {
    const damage = new BuildingDamage()
    damage.attach(key, twoHouses())
    const whole = Array.from(new Fracture(twoHouses(), 0, 0).build().positions)
    const shell = shellOf(twoHouses(), 0)!
    const cell = shell.cellAt(0, 1.5, 0)
    // The darkest a vertex is: a shot cell is the only dark one.
    const darkest = () => Math.min(...Array.from(bodyOf(damage).geometry.colors!))
    const fresh = Math.min(...Array.from(new Fracture(twoHouses(), 0, 0).build().colors))
    expect(damage.dent(key, 0, cell, 12)).toBe(true)
    expect(damage.fractured(key)).toEqual([0])
    const once = darkest()
    expect(once).toBeLessThan(fresh)
    expect(damage.dent(key, 0, cell, 12)).toBe(false)
    expect(darkest()).toBeLessThan(once)
    // Every cube keeps its size: the walls are where they were, only cut into more faces.
    const extent = (p: number[]) => [0, 1, 2].map((a) => [Math.min(...p.filter((_, i) => i % 3 === a)), Math.max(...p.filter((_, i) => i % 3 === a))])
    expect(extent(Array.from(bodyOf(damage).geometry.positions))).toEqual(extent(whole))
    // The wall breaks out: the dent is gone with the cell, and nothing is left to darken.
    damage.carve(key, 0, [cell], [], [-1, 1.5, 0], true)
    const broken = Array.from(bodyOf(damage).geometry.positions)
    expect(damage.dent(key, 0, cell, 12)).toBe(false)
    expect(Array.from(bodyOf(damage).geometry.positions)).toEqual(broken)
  })

  it('is as it was when its tile streams back in, and throws nothing for a catch-up', () => {
    const damage = new BuildingDamage()
    damage.attach(key, twoHouses())
    damage.carve(key, 0, [1, 2, 3, 4, 5, 6], [], null, false)
    const group = damage.root.children[0]!
    const pieces = group.children.filter((c) => c instanceof Mesh && c.castShadow && !c.receiveShadow)
    expect(pieces.length).toBe(0)
    const before = bodyOf(damage).geometry.vertexCount
    damage.detach(key)
    damage.attach(key, twoHouses())
    expect(bodyOf(damage).geometry.vertexCount).toBe(before)
  })
})
