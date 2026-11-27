import { describe, expect, it } from 'vitest'
import { Group, Mesh } from '@ragnarok/engine'
import { GateView } from '@/render/gates'
import { isGate, openingsFor } from '@/render/windows'
import { shellOf } from '@/sim/carve'
import { BUILDING_BARN, BUILDING_HALL, ROOF_FLAT, type Building, type TileData } from '@/world'

/** A tile with one looked building of `kind`: a closed box 10 × 10 m, 6 m tall, wound inwards as the importer winds it. */
function tileOf(kind: number): TileData {
  const vertices = [[0, 0, -5], [10, 0, -5], [10, 0, 5], [0, 0, 5], [0, 6, -5], [10, 6, -5], [10, 6, 5], [0, 6, 5]]
  const triangles = [[0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2]]
  const footprint = Int32Array.from([0, -500, 1000, -500, 1000, 500, 0, 500])
  const building = {
    height: 6, base: 0, first: 0, walls: 8, roof: 2, roofKind: ROOF_FLAT, kind, front: [0, 0], cells: null, paint: [null, null], clad: [0, 0],
    footprint: [[0, -5], [10, -5], [10, 5], [0, 5]], look: { first: 0, walls: 8, roof: 2, kind, footprint },
  } as unknown as Building
  const collision = { vertices: Float32Array.from(vertices.flat()), triangles: Uint32Array.from(triangles.flat()) }
  return { buildings: [building], roads: [], cover: [], trees: [], streams: [], bridges: [], collision, look: collision, ground: new Float32Array(0), water: new Float32Array(0) } as unknown as TileData
}

const KEY = 7
const leavesOf = (group: Group) => group.children.filter((child): child is Mesh => child instanceof Mesh && child.userData.gate === true)

describe('gates', () => {
  it('hangs a leaf in every gate, shut, and none in a house door', () => {
    for (const [kind, gates] of [[BUILDING_BARN, 1], [BUILDING_HALL, 2], [0, 0]] as const) {
      const tile = tileOf(kind)
      const group = new Group()
      const view = new GateView()
      view.attach(KEY, tile, group, new Map())
      expect(leavesOf(group).length).toBe(gates)
      const openings = openingsFor(tile.look!, tile.buildings[0]!.look!)
      expect(view.shut(KEY, 0)).toEqual(openings.flatMap((o, i) => (isGate(o) ? [i] : [])))
      expect(view.untouched).toBe(true)
    }
  })

  it('rolls a hall\'s door up under its lintel when the server opens it, and down again', () => {
    const tile = tileOf(BUILDING_HALL)
    const group = new Group()
    const view = new GateView()
    view.attach(KEY, tile, group, new Map())
    const [opening] = view.shut(KEY, 0)
    const o = openingsFor(tile.look!, tile.buildings[0]!.look!)[opening!]!
    const leaf = leavesOf(group)[0]!
    const top = leaf.position.y
    expect(top).toBeGreaterThan(o.corners[3][1])
    expect(leaf.scale.y).toBe(1)
    view.set(KEY, 0, opening!, true, true)
    expect(view.shut(KEY, 0)).not.toContain(opening)
    // On its way after half a second, up after two.
    view.update(0.5)
    expect(leaf.scale.y).toBeGreaterThan(0.1)
    expect(leaf.scale.y).toBeLessThan(1)
    view.update(1.5)
    expect(leaf.scale.y).toBeLessThan(0.1)
    expect(leaf.position.y).toBe(top)
    view.set(KEY, 0, opening!, false, true)
    view.update(2)
    expect(leaf.scale.y).toBe(1)
    expect(view.untouched).toBe(true)
  })

  it('slides a barn\'s door aside along its wall, out from it', () => {
    const tile = tileOf(BUILDING_BARN)
    const group = new Group()
    const view = new GateView()
    view.attach(KEY, tile, group, new Map())
    const [opening] = view.shut(KEY, 0)
    const o = openingsFor(tile.look!, tile.buildings[0]!.look!)[opening!]!
    const leaf = leavesOf(group)[0]!
    const from = [leaf.position.x, leaf.position.z]
    view.set(KEY, 0, opening!, true, true)
    view.update(2)
    const moved = [leaf.position.x - from[0]!, leaf.position.z - from[1]!]
    const width = Math.hypot(o.corners[1][0] - o.corners[0][0], o.corners[1][2] - o.corners[0][2])
    expect(Math.hypot(moved[0]!, moved[1]!)).toBeGreaterThan(width)
    // Along the wall: nothing of the move is out from it.
    expect(Math.abs(moved[0]! * o.normal[0] + moved[1]! * o.normal[2])).toBeLessThan(1e-6)
    expect(leaf.scale.y).toBe(1)
  })

  it('remembers an open gate while its tile is away, and is told of one at once on joining', () => {
    const tile = tileOf(BUILDING_HALL)
    const view = new GateView()
    const first = new Group()
    view.attach(KEY, tile, first, new Map())
    const [opening] = view.shut(KEY, 0)
    view.set(KEY, 0, opening!, true, false)
    expect(leavesOf(first).some((leaf) => leaf.scale.y < 0.1)).toBe(true)
    view.detach(KEY)
    const again = new Group()
    view.attach(KEY, tile, again, new Map())
    expect(view.opened(KEY)).toEqual([0])
    expect(leavesOf(again).filter((leaf) => leaf.scale.y < 0.1).length).toBe(1)
  })

  it('takes a gate away with the wall round it', () => {
    const tile = tileOf(BUILDING_BARN)
    const group = new Group()
    const view = new GateView()
    view.attach(KEY, tile, group, new Map())
    const [opening] = view.shut(KEY, 0)
    const o = openingsFor(tile.look!, tile.buildings[0]!.look!)[opening!]!
    const shell = shellOf(tile, 0)!
    const mid = [0, 1, 2].map((k) => (o.corners[0][k]! + o.corners[2][k]!) / 2 - o.normal[k]! * 0.01) as [number, number, number]
    const cell = shell.cellAt(mid[0], mid[1], mid[2])
    view.holed(KEY, 0, new Set([cell]))
    expect(leavesOf(group)[0]!.visible).toBe(false)
  })
})
