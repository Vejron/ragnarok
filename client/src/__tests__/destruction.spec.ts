import { describe, expect, it } from 'vitest'
import { Geometry, Group, InstancedMesh, Mesh, Vector3 } from '@ragnarok/engine'
import { encode } from '@msgpack/msgpack'
import { decodeServer, OBJ_TREE } from '@/net/codec'
import { BuildingDamage, span } from '@/render/damage'
import { buildTrees, showTreeInstance } from '@/render/trees'
import { GROWN_YOUNG, TREE_SAPLING_SCALE, TREE_YOUNG_SCALE } from '@/sim/consts'
import { FallingTrees } from '@/render/falling'
import { TREE_PINE, type TileData, type Tree } from '@/world'
import fixture from './messages.fixture.json'

const tree = (x: number, solid: boolean): Tree => ({ x, z: 0, base: 0, height: 16, crown: 3, kind: TREE_PINE, solid, trunkRadius: 0.2, trunkHeight: 8 })

describe('destroyed message', () => {
  it('decodes the bytes the server sends', () => {
    // `messages.fixture.json` holds every message as the Rust tests encode it.
    expect(decodeServer(new Uint8Array(fixture.destroyed))).toEqual({
      type: 'destroyed',
      objects: [{ kind: OBJ_TREE, col: 2, row: 3, index: 70000, yaw: -1.571, force: 0.2, stage: GROWN_YOUNG }],
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
    const top = new Vector3(0, 1, 0).applyQuaternion(pivot.quaternion)
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

  it('grows back from where it stood: the log goes, a sapling, a young tree, then whole', () => {
    const group = scene()
    const falling = new FallingTrees()
    const mesh = group.children[0] as InstancedMesh
    const whole = mesh.matrices.slice(0, 16)
    const children = group.children.length
    falling.fell(group, tree(0, true), 0, 0, false)
    expect(group.children.length).toBe(children + 1)
    expect(falling.lift(group, 0)).toBe(true)
    expect(falling.lift(group, 0)).toBe(false)
    expect(group.children.length).toBe(children)
    for (const scale of [TREE_SAPLING_SCALE, TREE_YOUNG_SCALE, 1]) {
      expect(showTreeInstance(group, 0, scale)).toBe(true)
      // Its size (the x axis's length) grows; its foot stays where it stood.
      expect(mesh.matrices[0]).toBeCloseTo(whole[0]! * scale)
      expect([...mesh.matrices.slice(12, 15)]).toEqual([...whole.slice(12, 15)])
    }
    expect([...mesh.matrices.slice(0, 16)]).toEqual([...whole])
  })

  it('cannot fell a sapling, but can fell a young tree again', () => {
    const group = scene()
    const falling = new FallingTrees()
    falling.fell(group, tree(0, true), 0, 0, false)
    falling.lift(group, 0)
    showTreeInstance(group, 0, TREE_SAPLING_SCALE)
    expect(falling.fell(group, tree(0, true), 0, 0, false)).toBe(false)
    showTreeInstance(group, 0, TREE_YOUNG_SCALE)
    expect(falling.fell(group, tree(0, true), 0, 0, false)).toBe(true)
    expect((group.children[0] as InstancedMesh).matrices[0]).toBe(0)
  })

  it('lies down at once when told it already fell', () => {
    const group = scene()
    new FallingTrees().fell(group, tree(0, true), 0, 0, false)
    const top = new Vector3(0, 1, 0).applyQuaternion(group.children[group.children.length - 1]!.quaternion)
    // yaw 0 faces -Z.
    expect(top.z).toBeLessThan(-0.95)
  })
})

// ---- buildings

import { Effects } from '@/render/effects'
import { Particles } from '@/render/particles'
import { buildTile, rebuildBuildings } from '@/render/world'
import type { SimMotor } from '@/sim/motor'
import { flatMotor, tileBytes } from './worlds'
import { Shell, shellOf } from '@/sim/carve'
import { Fracture } from '@/render/fracture'
import { drawLook, heapRubble } from '@/render/lookjobs'
import type { LookSource, LookWorker } from '@/render/lookworker'
import { lookChunk } from '@/render/lookfracture'
import type { RubbleMemo } from '@/render/shards'
import { toonMaterial } from '@/render/toon'
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
    height: 6, base: 0, first, walls: 8, roof: 2, roofKind: ROOF_FLAT, kind: 0, front: [0, 0], cells: null, paint: [null, null], clad: [0, 0],
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
  /** Corners of the merged meshes' triangles that are drawn: a building taken out in place leaves its own collapsed to a point. */
  const vertexCount = (group: Group) =>
    group.children.reduce((n, c) => {
      if (!(c instanceof Mesh) || !c.userData.merged) return n
      const p = c.geometry.positions
      const indices = c.geometry.indices!
      for (let i = 0; i < indices.length; i += 3) {
        const [a, b, d] = [indices[i]! * 3, indices[i + 1]! * 3, indices[i + 2]! * 3]
        const [ax, ay, az, bx, by, bz, cx, cy, cz] = [p[a], p[a + 1], p[a + 2], p[b], p[b + 1], p[b + 2], p[d], p[d + 1], p[d + 2]]
        const [ux, uy, uz, vx, vy, vz] = [bx! - ax!, by! - ay!, bz! - az!, cx! - ax!, cy! - ay!, cz! - az!]
        if (Math.hypot(uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx) > 0) n += 3
      }
      return n
    }, 0)

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

  it('takes a building out in place when it only leaves out more of them', () => {
    const tile = twoHouses()
    const group = buildTile(tile, 0)
    const mesh = group.children.find((c): c is Mesh => c instanceof Mesh && c.userData.merged === true)!
    const geometry = mesh.geometry
    rebuildBuildings(group, tile, 0, new Set([0]))
    // The same geometry, with only the first house's vertices collapsed.
    expect(group.children.find((c) => c instanceof Mesh && c.userData.merged)).toBe(mesh)
    expect(geometry.collapsedRanges).toHaveLength(2)
    const fresh = buildTile(tile, 0, null, undefined, new Set([0]))
    expect(vertexCount(group)).toBe(vertexCount(fresh))
    // Bringing a house back takes a new mesh.
    rebuildBuildings(group, tile, 0, new Set())
    expect(group.children.find((c) => c instanceof Mesh && c.userData.merged)).not.toBe(mesh)
  })
})

describe('the building collider', () => {
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
    motor.buildBuildings(0, 0, Infinity, 0)
    return motor
  }
  const along = (motor: SimMotor, x: number, y = 1, z = 0) => motor.castRay([x, y, z], [1, 0, 0], 5)

  // The cells are the ones the client draws (`carve.ts`); the server's physics cuts them out of its own shell.
  it('opens the wall where cells were blasted out and leaves the rest of that wall', async () => {
    const tile = twoHouses()
    const shell = shellOf(tile, 0)!
    const hole = new Set<number>()
    for (let y = 0; y <= 2.5; y += 0.5) for (let z = -1.5; z <= 1.5; z += 0.5) hole.add(shell.cellAt(0, y, z))
    const motor = await solidTile(tile)
    motor.setBuildings(0, 0, new Map([[0, hole]]))
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
  // The building as it stands: it receives shadows, which the pieces thrown from it do not.
  const bodyOf = (damage: BuildingDamage) =>
    damage.root.children[0]!.children.find((c) => c instanceof Mesh && c.receiveShadow && c.geometry.colors) as Mesh | undefined
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
    const inside = ([x, z]: number[]) => x! >= 0 && z! >= 0 && x! < shell.dims[0] && z! < shell.dims[2]
    const next = [[hx + 1, hz], [hx - 1, hz], [hx, hz + 1], [hx, hz - 1]].filter(inside).map(([x, z]) => shell.index(x!, hy, z!))
    const gap = next.find((id) => shell.isSolid(id))!
    const [gx, , gz] = shell.coords(gap)
    const damage = new BuildingDamage()
    damage.attach(key, tile)
    // The gap was cut earlier (no animation); now the piece beside it loses its hold.
    damage.carve(key, 0, [gap], [], null, false)
    damage.carve(key, 0, [], [hung], [0, 2, 0], true)
    const pieces = damage.root.children[0]!.children.filter((c) => c instanceof Mesh && c.geometry.colors && c.position.lengthSq() > 0) as Mesh[]
    const start = pieces.map((m) => m.position.clone())
    for (let i = 0; i < 30; i++) damage.update(0.02, { dust() {} } as never, new Vector3(1e6, 0, 0))
    const moved = pieces.map((m, i) => m.position.clone().sub(start[i]!))
    const towards = moved.some((d) => d.x * Math.sign(gx - hx) + d.z * Math.sign(gz - hz) > 0.2)
    expect(towards).toBe(true)
  })

  it('lets what it throws come to rest on the ground where it lands, not at its floor', () => {
    const damage = new BuildingDamage()
    // Ground falling away from the house: 2 m lower for every 10 m out along x.
    const ground = (x: number) => -0.2 * Math.abs(x - 5)
    damage.groundAt = (x) => ground(x)
    const landed: number[] = []
    damage.landed = (_x, y) => landed.push(y)
    damage.attach(key, tileWith())
    damage.carve(key, 0, [cellAt(0, 0, 2, 0), cellAt(0, 0, 3, 0), cellAt(0, 0, 4, 0)], [], [-1, 2, 0], true)
    const pieces = damage.root.children[0]!.children.filter((c) => c instanceof Mesh && c.geometry.colors && c.position.lengthSq() > 0) as Mesh[]
    expect(pieces.length).toBeGreaterThan(0)
    for (let i = 0; i < 300; i++) damage.update(0.02, { dust() {} } as never, new Vector3(1e6, 0, 0))
    // Each is heard coming down, once.
    expect(landed.length).toBe(pieces.length)
    // Each lies with its lowest corner on the ground under it.
    for (const piece of pieces) expect(span(piece).low).toBeCloseTo(ground(piece.position.x), 1)
  })

  it('brings every section of a looked house to lie still on the ground, none left in the air', () => {
    const tile = tileWith()
    // Drawn from its look: what loses its hold falls as whole sections that tip over.
    tile.look = tile.collision
    tile.buildings[0]!.look = { first: 0, walls: 8, roof: 2 }
    const damage = new BuildingDamage()
    damage.groundAt = () => 0
    damage.attach(key, tile)
    const shell = shellOf(tile, 0)!
    const fell: number[] = []
    for (let id = 0; id < shell.dims[0] * shell.dims[1] * shell.dims[2]; id++) if (shell.isSolid(id) && shell.coords(id)[1] >= 3) fell.push(id)
    damage.carve(key, 0, [cellAt(0, 0, 1, 0), cellAt(0, 0, 2, 0)], [], [-1, 2, 0], false)
    damage.carve(key, 0, [], fell, [5, 4, 0], true)
    const pieces = () => damage.root.children[0]!.children.filter((c) => c instanceof Mesh && c.geometry.colors && c.position.lengthSq() > 0) as Mesh[]
    expect(pieces().length).toBeGreaterThan(0)
    for (let i = 0; i < 450; i++) damage.update(0.02, { dust() {} } as never, new Vector3(1e6, 0, 0))
    const before = pieces().map((m) => [m.position.clone(), m.quaternion.clone()] as const)
    for (let i = 0; i < 25; i++) damage.update(0.02, { dust() {} } as never, new Vector3(1e6, 0, 0))
    pieces().forEach((m, i) => {
      expect(span(m).low).toBeCloseTo(0, 1)
      expect(m.position.distanceTo(before[i]![0])).toBeLessThan(1e-6)
      expect(Math.abs(m.quaternion.dot(before[i]![1]))).toBeCloseTo(1, 6)
    })
  })

  it('lays a fallen section of wall down flat on its broad side, not spinning on the spot', () => {
    const tile = tileWith()
    tile.look = tile.collision
    tile.buildings[0]!.look = { first: 0, walls: 8, roof: 2 }
    const damage = new BuildingDamage()
    damage.groundAt = () => 0
    damage.attach(key, tile)
    // The front wall's top half, 4 m along, loses its hold: a slab standing on its edge.
    const shell = shellOf(tile, 0)!
    const slab: number[] = []
    for (let x = 1; x < 5; x++) for (let y = 3; y < 6; y++) {
      const id = cellAt(0, x + 0.3, y + 0.3, -4.9)
      if (shell.isSolid(id) && !slab.includes(id)) slab.push(id)
    }
    damage.carve(key, 0, [], slab, [3, 4, -5], true)
    const sections = () => (damage as unknown as { drawn: Map<number, Map<number, { falling: { mesh: Mesh; section?: boolean; resting: boolean }[] }>> }).drawn.get(key)!.get(0)!.falling.filter((f) => f.section)
    expect(sections().length).toBeGreaterThan(0)
    for (let i = 0; i < 400; i++) damage.update(0.02, { dust() {} } as never, new Vector3(1e6, 0, 0))
    for (const { mesh, resting } of sections()) {
      expect(resting).toBe(true)
      // Lying down: far wider than it is tall.
      const { height, wide } = span(mesh)
      expect(height).toBeLessThan(wide * 0.5)
      expect(span(mesh).low).toBeCloseTo(0, 1)
    }
  })

  /** A house with a look whose front wall's top half, 4 m along, is about to lose its hold: the tile, and that slab's cells. */
  const slabHouse = () => {
    const tile = tileWith()
    tile.look = tile.collision
    tile.buildings[0]!.look = { first: 0, walls: 8, roof: 2 }
    const shell = shellOf(tile, 0)!
    const slab: number[] = []
    for (let x = 1; x < 5; x++) for (let y = 3; y < 6; y++) {
      const id = cellAt(0, x + 0.3, y + 0.3, -4.9)
      if (shell.isSolid(id) && !slab.includes(id)) slab.push(id)
    }
    return { tile, slab }
  }

  /**
   * A look worker that does what the real one does (`drawLook`, `heapRubble`), but only when the test
   * lets each request through (`answer`), as the real one answers a frame or two later.
   */
  const heldWorker = () => {
    const waiting: (() => void)[] = []
    const known = new Map<string, { source: LookSource; shell: Shell; mounds: RubbleMemo }>()
    const told = (name: string, source: () => LookSource) => {
      if (!known.has(name)) {
        const s = source()
        known.set(name, { source: s, shell: Shell.ofFlags(s.shell), mounds: new Map() })
      }
      return known.get(name)!
    }
    const worker = {
      draw: (name: string, source: () => LookSource, ask: Parameters<LookWorker['draw']>[2]) => new Promise((resolve) => waiting.push(() => resolve(drawLook(told(name, source).source, told(name, source).shell, ask)))),
      heap: (name: string, source: () => LookSource, ask: Parameters<LookWorker['heap']>[2]) => new Promise((resolve) => waiting.push(() => resolve(heapRubble(told(name, source).shell, ask, told(name, source).mounds)))),
      dispose: () => {},
    }
    const answer = async () => {
      for (const reply of waiting.splice(0)) reply()
      await new Promise((r) => setTimeout(r, 0))
    }
    return { worker: worker as unknown as LookWorker, waiting, answer }
  }

  type Inside = { drawn: Map<number, Map<number, { falling: { mesh: Mesh; section?: boolean }[]; rubble: Mesh | null }>> }

  it('throws a falling section with the redraw that shows its hole', async () => {
    const { tile, slab } = slabHouse()
    const damage = new BuildingDamage()
    damage.groundAt = () => 0
    const { worker, waiting, answer } = heldWorker()
    damage.looks = worker
    damage.attach(key, tile)
    damage.carve(key, 0, [], slab, [3, 4, -5], true)
    const drawn = () => (damage as unknown as Inside).drawn.get(key)!.get(0)!
    // Asked for, with its hole: nothing of the wall falls before the hole it leaves shows.
    expect(waiting.length).toBe(1)
    expect(drawn().falling.filter((f) => f.section)).toHaveLength(0)
    expect(bodyOf(damage)).toBeUndefined()
    await answer()
    expect(bodyOf(damage)).toBeDefined()
    expect(drawn().falling.filter((f) => f.section).length).toBeGreaterThan(0)
  })

  it('cuts out the same falling section in the worker as on the main thread', async () => {
    const sections = async (worker: boolean) => {
      const { tile, slab } = slabHouse()
      const damage = new BuildingDamage()
      damage.groundAt = () => 0
      const held = heldWorker()
      if (worker) damage.looks = held.worker
      damage.attach(key, tile)
      damage.carve(key, 0, [], slab, [3, 4, -5], true)
      await held.answer()
      return (damage as unknown as Inside).drawn.get(key)!.get(0)!.falling.filter((f) => f.section).map((f) => [f.mesh.position.toArray(), Array.from(f.mesh.geometry.positions)])
    }
    expect(await sections(true)).toEqual(await sections(false))
  })

  it('heaps the rubble the worker made once what fell has landed, as the main thread heaps it', async () => {
    const heap = async (worker: boolean) => {
      const { tile, slab } = slabHouse()
      const damage = new BuildingDamage()
      damage.groundAt = () => 0
      const held = heldWorker()
      if (worker) damage.looks = held.worker
      damage.attach(key, tile)
      damage.carve(key, 0, [], slab, [3, 4, -5], true)
      await held.answer()
      const rubble = () => (damage as unknown as Inside).drawn.get(key)!.get(0)!.rubble
      expect(rubble()).toBeNull()
      // About a second later what fell has landed, and the heap is asked for.
      for (let i = 0; i < 60; i++) damage.update(0.02, { dust() {} } as never, new Vector3(1e6, 0, 0))
      if (worker) {
        expect(rubble()).toBeNull()
        expect(held.waiting.length).toBe(1)
        await held.answer()
      }
      return Array.from(rubble()!.geometry.positions)
    }
    const made = await heap(true)
    expect(made.length).toBeGreaterThan(0)
    expect(made).toEqual(await heap(false))
  })

  it('measures a falling piece’s reach from its corners as from every vertex', () => {
    const { tile, slab } = slabHouse()
    const shell = shellOf(tile, 0)!
    const soup = lookChunk(tile.look!, tile.buildings[0]!.look!, shell, new Set(slab), [3, 4, -5], [1, 1, 1], [1, 1, 1])
    const mesh = new Mesh(new Geometry({ positions: soup.positions }), toonMaterial({}))
    for (const [x, y, z] of [[0, 0, 0], [0.7, 0.2, -1.1], [1.9, -0.4, 0.3], [-2.2, 1.3, 2.8]] as const) {
      mesh.rotation.set(x, y, z)
      // Every vertex, turned as the piece is.
      const p = soup.positions
      let [low, high, x0, x1, z0, z1] = [Infinity, -Infinity, Infinity, -Infinity, Infinity, -Infinity]
      for (let i = 0; i < p.length; i += 3) {
        const v = new Vector3(p[i]!, p[i + 1]!, p[i + 2]!).applyQuaternion(mesh.quaternion)
        ;[low, high] = [Math.min(low, v.y), Math.max(high, v.y)]
        ;[x0, x1, z0, z1] = [Math.min(x0, v.x), Math.max(x1, v.x), Math.min(z0, v.z), Math.max(z1, v.z)]
      }
      const reach = span(mesh)
      expect(reach.low).toBeCloseTo(low, 1)
      expect(reach.height).toBeCloseTo(high - low, 1)
      expect(reach.wide).toBeCloseTo(Math.max(x1 - x0, z1 - z0), 1)
    }
  })

  it('lets every section fall in a long cascade, however much already lies about', () => {
    const tile = tileWith()
    tile.look = tile.collision
    tile.buildings[0]!.look = { first: 0, walls: 8, roof: 2 }
    const damage = new BuildingDamage()
    damage.groundAt = () => 0
    damage.attach(key, tile)
    const shell = shellOf(tile, 0)!
    const solid: number[] = []
    for (let id = 0; id < shell.dims[0] * shell.dims[1] * shell.dims[2]; id++) if (shell.isSolid(id)) solid.push(id)
    const drawn = () => (damage as unknown as { drawn: Map<number, Map<number, { falling: { section?: boolean }[] }>> }).drawn.get(key)?.get(0)?.falling ?? []
    // Top down, a few cells at a time, as the server lets a building go.
    solid.sort((a, b) => shell.coords(b)[1] - shell.coords(a)[1] || a - b)
    let falls = 0
    for (let at = 0; at < solid.length; at += 6) {
      const group = solid.slice(at, at + 6)
      const before = drawn().filter((f) => f.section).length
      damage.carve(key, 0, [], group, null, true)
      const after = drawn().filter((f) => f.section).length
      // A section for each fall (or, past the limit, the oldest sinking away while it does).
      if (after <= before) expect(drawn().length).toBeGreaterThan(60)
      else falls++
      for (let i = 0; i < 20; i++) damage.update(0.02, { dust() {} } as never, new Vector3(1e6, 0, 0))
    }
    expect(falls).toBeGreaterThan(solid.length / 6 / 2)
    expect(drawn().length).toBeGreaterThan(96)
  })

  it('keeps what it threw lying about, rather than taking it away after a while', () => {
    const damage = new BuildingDamage()
    damage.attach(key, tileWith())
    damage.carve(key, 0, [cellAt(0, 0, 2, 0), cellAt(0, 0, 3, 0)], [], [-1, 2, 0], true)
    const count = () => damage.root.children[0]!.children.filter((c) => c instanceof Mesh && c.geometry.colors && c.position.lengthSq() > 0).length
    const thrown = count()
    expect(thrown).toBeGreaterThan(0)
    for (let i = 0; i < 1500; i++) damage.update(0.02, { dust() {} } as never, new Vector3(1e6, 0, 0))
    expect(count()).toBe(thrown)
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
  const effects = () => new Effects({ smoke: new Particles(64), glow: new Particles(64, true), halo: new Particles(64, true, true), blood: new Particles(64) }, 64)

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
    damage.update(0, e, new Vector3(500, 0, 0))
    e.dispose()
    expect(bodyOf(damage).geometry.vertexCount).toBeLessThan(whole)
  })

  it('stands whole again where cells drawn as gone on a prediction are mended, and keeps the others out', () => {
    const damage = new BuildingDamage()
    damage.attach(key, twoHouses())
    const hole = Array.from({ length: 4000 }, (_, i) => i)
    damage.carve(key, 0, hole, [], [-1, 2, 0], true)
    const e = effects()
    damage.update(0, e, new Vector3(500, 0, 0))
    const holed = bodyOf(damage).geometry.vertexCount
    // The server bore out the first hundred cells only.
    damage.mend(key, 0, hole.slice(100))
    damage.update(0, e, new Vector3(500, 0, 0))
    expect(damage.shown(key, 0).cells).toBe(100)
    expect(bodyOf(damage).geometry.vertexCount).not.toBe(holed)
    damage.mend(key, 0, hole)
    damage.update(0, e, new Vector3(500, 0, 0))
    e.dispose()
    expect(damage.shown(key, 0).cells).toBe(0)
    // The wall is back, though not as the few big faces it was: where the hole was is still marked.
    expect(bodyOf(damage).geometry.vertexCount).toBeGreaterThan(holed)
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
    for (let i = 0; i < 160; i++) damage.update(0.05, e, new Vector3(500, 0, 0))
    // Eight seconds on, all of it lies still on the ground.
    const lying = group.children.length
    expect(lying).toBeGreaterThan(before)
    const where = () => group.children.map((c) => [c.position.x, c.position.y, c.position.z]).flat()
    const still = where()
    damage.update(0.05, e, new Vector3(500, 0, 0))
    expect(where()).toEqual(still)
    e.dispose()
  })

  it('lets what was thrown lie a while, then sinks it slowly into the ground, all gone 30 s after it lands', () => {
    const damage = new BuildingDamage()
    damage.attach(key, twoHouses())
    damage.dent(key, 0, 0, 1)
    const group = damage.root.children[0]!
    const before = group.children.length
    const all = Array.from({ length: 4000 }, (_, i) => i)
    damage.carve(key, 0, all.slice(0, 20), all.slice(20, 60), [-1, 2, 0], true)
    const e = effects()
    const thrown = () => group.children.slice(before) as Mesh[]
    // Landed and lying by eight seconds; a little after the tenth second on the ground, sinking: lower, not yet gone.
    for (let i = 0; i < 160; i++) damage.update(0.05, e, new Vector3(500, 0, 0))
    const count = thrown().length
    expect(count).toBeGreaterThan(0)
    for (let i = 0; i < 160; i++) damage.update(0.05, e, new Vector3(500, 0, 0))
    expect(thrown().length).toBeGreaterThan(0)
    expect(thrown().some((m) => span(m).low < -0.05)).toBe(true)
    // Past thirty seconds on the ground, the last of it is gone.
    for (let i = 0; i < 600; i++) damage.update(0.05, e, new Vector3(500, 0, 0))
    expect(thrown().filter((m) => m.geometry.colors)).toEqual([])
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

  it('smoulders for a minute after being hit, thinning out, and then stops', () => {
    const smoulder = () => {
      const damage = new BuildingDamage()
      damage.attach(key, twoHouses())
      damage.dent(key, 0, 0, 1)
      damage.carve(key, 0, [1, 2, 3], [], [-1, 2, 0], true)
      const puffs: { alpha: number; color: number; at: number }[] = []
      const smoke = { spawn: (spec: { alpha: number; color: number }) => puffs.push({ alpha: spec.alpha, color: spec.color, at: seconds }) }
      let seconds = 0
      for (; seconds < 90; seconds += 0.1) damage.update(0.1, { dust() {} } as never, new Vector3(0, 0, 0), smoke)
      return puffs
    }
    const day = smoulder()
    expect(day.length).toBeGreaterThan(10)
    // Thick at first, thin towards the end, and nothing once the minute is up.
    expect(day[0]!.alpha).toBeGreaterThan(day[day.length - 1]!.alpha)
    expect(Math.max(...day.map((p) => p.at))).toBeLessThanOrEqual(60.1)
    expect(day.filter((p) => p.at < 30).length).toBeGreaterThan(day.filter((p) => p.at >= 30).length)
  })
})
