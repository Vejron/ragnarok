import { beforeAll, describe, expect, it } from 'vitest'
import * as THREE from 'three'
import { decodeServer, OBJ_TREE } from '@/net/codec'
import { buildTrees } from '@/render/trees'
import { FallingTrees } from '@/render/falling'
import { trunkColliderIndex } from '@/sim/physics'
import { TREE_PINE, type TileData, type Tree } from '@/world'

// A copy of `DESTROYED_FIXTURE` in `crates/shared/src/protocol.rs`.
const DESTROYED_FIXTURE = [146, 6, 145, 149, 0, 2, 3, 206, 0, 1, 17, 112, 209, 249, 221]

const tree = (x: number, solid: boolean): Tree => ({ x, z: 0, base: 0, height: 16, crown: 3, kind: TREE_PINE, solid, trunkRadius: 0.2, trunkHeight: 8 })

describe('destroyed message', () => {
  it('decodes the bytes the server sends', () => {
    expect(decodeServer(new Uint8Array(DESTROYED_FIXTURE))).toEqual({
      type: 'destroyed',
      objects: [{ kind: OBJ_TREE, col: 2, row: 3, index: 70000, yaw: -1.571 }],
    })
  })

  it('refuses entries of the wrong shape', () => {
    // [6, [[0, 1, 2]]]
    expect(decodeServer(new Uint8Array([146, 6, 145, 147, 0, 1, 2]))).toBeNull()
  })
})

describe('trunkColliderIndex', () => {
  const tile = (triangles: number, trees: Tree[]) => ({ trees, collision: { vertices: new Float32Array(0), triangles: new Uint32Array(triangles * 3) } }) as unknown as TileData

  it('counts the building mesh, then solid trees only', () => {
    const t = tile(1, [tree(0, true), tree(5, false), tree(10, true)])
    expect(trunkColliderIndex(t, 0)).toBe(1)
    expect(trunkColliderIndex(t, 1)).toBe(-1)
    expect(trunkColliderIndex(t, 2)).toBe(2)
  })

  it('starts at zero when the tile has no buildings', () => {
    expect(trunkColliderIndex(tile(0, [tree(0, true)]), 0)).toBe(0)
  })
})

describe('felling a tree', () => {
  const scene = () => {
    const group = new THREE.Group()
    group.add(...buildTrees([tree(0, true), tree(10, true)]))
    return group
  }

  it('hides its instance once, and only that one', () => {
    const group = scene()
    const falling = new FallingTrees()
    expect(falling.fell(group, tree(0, true), 0, 0, false)).toBe(true)
    expect(falling.fell(group, tree(0, true), 0, 0, false)).toBe(false)
    const mesh = group.children[0] as THREE.InstancedMesh
    const m = new THREE.Matrix4()
    mesh.getMatrixAt(0, m)
    expect(m.elements[0]).toBe(0)
    mesh.getMatrixAt(1, m)
    expect(m.elements[0]).not.toBe(0)
  })

  it('tips the crown towards the way it falls', () => {
    // yaw -π/2 faces +X.
    const group = scene()
    const falling = new FallingTrees()
    falling.fell(group, tree(0, true), 0, -Math.PI / 2, true)
    for (let i = 0; i < 400; i++) falling.update(0.01)
    const pivot = group.children[group.children.length - 1]!
    const top = new THREE.Vector3(0, 1, 0).applyQuaternion(pivot.quaternion)
    expect(top.x).toBeGreaterThan(0.95)
    expect(Math.abs(top.z)).toBeLessThan(0.05)
    expect(top.y).toBeLessThan(0.1)
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

import RAPIER from '@dimforge/rapier3d-compat'
import { Effects } from '@/render/effects'
import { Collapses } from '@/render/rubble'
import { buildTile, rebuildBuildings } from '@/render/world'
import { rebuildBuildingCollider } from '@/sim/physics'
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
    height: 6, base: 0, first, walls: 8, roof: 2, roofKind: ROOF_FLAT,
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
  const vertexCount = (group: THREE.Group) =>
    group.children.reduce((n, c) => n + (c instanceof THREE.Mesh && c.userData.merged ? c.geometry.getAttribute('position').count : 0), 0)

  it('drops that building\'s triangles from the merged meshes and no others', () => {
    const tile = twoHouses()
    const group = buildTile(tile, 0)
    // 10 drawn triangles a house (the floor is not drawn), 3 vertices each.
    expect(vertexCount(group)).toBe(2 * 10 * 3)
    rebuildBuildings(group, tile, 0, new Set([0]))
    expect(vertexCount(group)).toBe(10 * 3)
    rebuildBuildings(group, tile, 0, new Set([0, 1]))
    expect(vertexCount(group)).toBe(0)
  })
})

describe('the building collider without a fallen building', () => {
  beforeAll(async () => {
    await RAPIER.init()
  })

  const solid = (world: RAPIER.World, x: number) => {
    world.step()
    return world.castRay(new RAPIER.Ray({ x, y: 1, z: 0 }, { x: 1, y: 0, z: 0 }), 5, true) !== null
  }

  it('keeps the other house and loses that one', () => {
    const tile = twoHouses()
    const world = new RAPIER.World({ x: 0, y: -9.81, z: 0 })
    const collider = rebuildBuildingCollider(world, tile, new Set([0]))
    expect(collider).not.toBeNull()
    // Rays start 2 m before each house's west wall.
    expect(solid(world, -2)).toBe(false)
    expect(solid(world, 28)).toBe(true)
  })

  it('is nothing at all once every building is gone', () => {
    const world = new RAPIER.World({ x: 0, y: 0, z: 0 })
    expect(rebuildBuildingCollider(world, twoHouses(), new Set([0, 1]))).toBeNull()
  })
})

describe('a building collapsing', () => {
  it('sinks its walls, leaves a pile of rubble in the tile, and clears away its pieces', () => {
    const tile = twoHouses()
    const group = new THREE.Group()
    new THREE.Scene().add(group)
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
    const heap = group.children[0] as THREE.Mesh
    expect(heap.scale.y).toBeGreaterThanOrEqual(1)
    expect(heap.scale.y).toBeLessThanOrEqual(3)
    effects.dispose()
  })

  it('is just the pile when the tile came in after it fell', () => {
    const group = new THREE.Group()
    Collapses.settled(group, twoHouses(), 1)
    expect(group.children.length).toBe(1)
    expect(group.children[0]!.position.x).toBeCloseTo(35)
  })
})
