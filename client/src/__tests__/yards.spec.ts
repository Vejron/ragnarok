import { describe, expect, it } from 'vitest'
import { encode } from '@msgpack/msgpack'
import { serverOpenings, type Tri } from '@/sim/carve'
import { openingsOf } from '@/render/windows'
import { facadeOf } from '@/render/facade'
import { joinHoles, triangulate } from '@/render/geom'
import { grassMask } from '@/render/grassmask'
import { buildingAt } from '@/screenshot'
import { BUILDING_FLATS, BUILDING_HALL, decodeTile, yardsOf, type Building, type CollisionMesh, type TileRef } from '@/world'

/*
 * A building round a yard that is open to the sky (`Building.holes`): walls round the yard as round
 * the outline, no floor or roof over it, grass in it, and no door into it.
 */

type Q = [number, number]

/**
 * An eight-sided block with walls of 20 and 14 m, round a yard 24 × 6 m whose long walls are the
 * longest of all. Centimetres; the outline counter-clockwise from above and the yard clockwise, as
 * the world file has them. As `the_front_door_is_never_on_a_courtyard_wall` in `windows.rs`.
 */
const OUTER: Q[] = [[1000, 0], [3000, 0], [4000, 1000], [4000, 3000], [3000, 4000], [1000, 4000], [0, 3000], [0, 1000]]
const YARD: Q[] = [[800, 1700], [800, 2300], [3200, 2300], [3200, 1700]]
const HEIGHT = 600

/** The walls round `rings`, `HEIGHT` high, wound inwards as a look is. */
function wallsOf(rings: readonly Q[][]): Tri[] {
  const out: Tri[] = []
  for (const ring of rings) {
    ring.forEach((p, i) => {
      const q = ring[(i + 1) % ring.length]!
      out.push([p[0], 0, p[1], q[0], 0, q[1], q[0], HEIGHT, q[1]], [p[0], 0, p[1], q[0], HEIGHT, q[1], p[0], HEIGHT, p[1]])
    })
  }
  return out
}

function lookOf(tris: Tri[]): CollisionMesh {
  const vertices = new Float32Array(tris.length * 9)
  tris.forEach((t, i) => t.forEach((v, k) => (vertices[i * 9 + k] = v / 100)))
  return { vertices, triangles: Uint32Array.from(tris.flatMap((_, i) => [i * 3, i * 3 + 1, i * 3 + 2])) }
}

/** Whether every corner of an opening is on a wall of the yard, metres. */
const inYard = (corners: readonly (readonly number[])[]) => corners.every((p) => p[0]! > 7.9 && p[0]! < 32.1 && p[2]! > 16.9 && p[2]! < 23.1)

/** The area of triangles lying flat, seen from above, square metres. */
const areaOf = (tris: readonly (readonly (readonly number[])[])[]) => tris.reduce((sum, [a, b, c]) => sum + Math.abs((b![0]! - a![0]!) * (c![2]! - a![2]!) - (b![2]! - a![2]!) * (c![0]! - a![0]!)) / 2, 0)

describe('a building round a yard', () => {
  it('has no door on the walls of its yard, on the client as on the server, and windows on them', () => {
    const walls = wallsOf([OUTER, YARD])
    const yards = yardsOf([YARD])
    for (const [kind, front] of [[0, [0, 0]], [0, [0, 127]], [0, [0, -127]], [BUILDING_HALL, [0, 0]], [BUILDING_FLATS, [127, 0]]] as const) {
      const ours = openingsOf(lookOf(walls), { first: 0, walls: walls.length, roof: 0, kind, front, yards })
      const theirs = serverOpenings(walls, kind, front, yards)
      expect(theirs.length).toBe(ours.length)
      ours.forEach((o, i) => {
        const t = theirs[i]!
        expect(t.door).toBe(o.door)
        for (let k = 0; k < 4; k++) for (let a = 0; a < 3; a++) expect(t.corners[k]![a]).toBeCloseTo(o.corners[k]![a]!, 4)
      })
      const doors = ours.filter((o) => o.door)
      expect(doors.length).toBeGreaterThan(0)
      expect(doors.some((door) => inYard(door.corners))).toBe(false)
      expect(ours.some((o) => !o.door && inYard(o.corners))).toBe(true)
    }
    // Told nothing of the yard, the door goes on its wall, the longest.
    expect(openingsOf(lookOf(walls), { first: 0, walls: walls.length, roof: 0 }).some((o) => o.door && inYard(o.corners))).toBe(true)
  })

  it('has a floor under its roof and none in its yard', () => {
    const walls = wallsOf([OUTER, YARD])
    const facade = facadeOf(lookOf(walls), { first: 0, walls: walls.length, roof: 0 })
    // 40 × 40 m less four corners of 10 × 10 / 2, less the yard.
    expect(areaOf(facade.floor)).toBeCloseTo(1600 - 200 - 144, 3)
    for (const tri of facade.floor) {
      const [x, z] = [(tri[0]![0] + tri[1]![0] + tri[2]![0]) / 3, (tri[0]![2] + tri[1]![2] + tri[2]![2]) / 3]
      expect(x > 8.01 && x < 31.99 && z > 17.01 && z < 22.99, `a floor triangle at ${x}, ${z}`).toBe(false)
    }
  })

  it('lets two yards into an L, whichever way each is drawn', () => {
    const l: Q[] = [[0, 0], [30, 0], [30, 12], [12, 12], [12, 30], [0, 30]]
    const yards: Q[][] = [[[3, 3], [9, 3], [9, 9], [3, 9]], [[20, 4], [20, 8], [26, 8], [26, 4]]]
    const joined = joinHoles(l, yards)
    expect(joined.length).toBe(l.length + 8 + 4)
    const ids = triangulate(joined)
    let area = 0
    for (let i = 0; i < ids.length; i += 3) {
      const [a, b, c] = [joined[ids[i]!]!, joined[ids[i + 1]!]!, joined[ids[i + 2]!]!]
      area += Math.abs((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) / 2
    }
    expect(area).toBeCloseTo(30 * 12 + 12 * 18 - 36 - 24, 6)
  })

  it('is read from a tile with its yards, in metres, and packed for the WebAssembly build', () => {
    const building = [600, OUTER, [YARD], 0, 0, 0, 0, 0, BUILDING_FLATS, [0, 127], [[0, 0, 0], [0, 0, 0], new Uint8Array(), new Uint8Array()], [null, null], [0, 0]]
    const tile = decodeTile(
      encode([[building], [], [[], []], [2, new Uint8Array()], [], [], new Uint8Array(), new Uint8Array(), new Uint8Array(), new Uint8Array(), new Uint8Array(), new Uint8Array(), [], [], new Uint8Array(), [], [], [], new Uint8Array(), new Uint8Array(), [[], [], [[0, 4, 2]]], []]),
    )
    const read = tile.buildings[0]!
    expect(read.holes).toEqual([[[8, 17], [8, 23], [32, 23], [32, 17]]])
    expect(read.footprint[2]).toEqual([40, 10])
    expect([...read.look!.yards!]).toEqual([4, 800, 1700, 800, 2300, 3200, 2300, 3200, 1700])
  })

  it('has grass in its yard and none under its roof', () => {
    const metres = (ring: Q[]) => ring.map(([x, z]) => [x / 100, z / 100] as Q)
    const building = { footprint: metres(OUTER), holes: [metres(YARD)], kind: BUILDING_FLATS }
    const mask = grassMask({ cover: [], roads: [], water: null as never, streams: [], buildings: [building] }, 0, 0, 64)
    const at = (x: number, z: number) => mask.cells[Math.floor((z / 64) * mask.side) * mask.side + Math.floor((x / 64) * mask.side)]!
    expect(at(20, 20)).toBeGreaterThan(0)
    expect(at(20, 10)).toBe(0)
    expect(at(50, 50)).toBeGreaterThan(0)
  })

  it('is what a shot names for a point on a wall of its yard, and not for one out in the yard', () => {
    const metres = (ring: Q[]) => ring.map(([x, z]) => [x / 100, z / 100] as Q)
    const block = { ref: { col: 0, row: 0 } as TileRef, index: 0, building: { footprint: metres(OUTER), holes: [metres(YARD)] } as Building }
    expect(buildingAt([20, 10], [block])).toBe(block)
    expect(buildingAt([20, 17.05], [block])).toBe(block)
    expect(buildingAt([20, 20], [block])).toBeNull()
  })
})
