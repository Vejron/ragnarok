import { describe, expect, it } from 'vitest'
import { Group, Mesh, Vector3 } from '@ragnarok/engine'
import { GLASS_OUT, GLASS_REACH, PANE_VERTICES, glassBuffers, paneBlasted, paneCentre, paneSheet, paneStruck, teethOf, writePane } from '@/render/glass'
import { Glazing } from '@/render/glazing'
import { openingStands } from '@/render/lookfracture'
import { openingsFor, openingsOf } from '@/render/windows'
import { buildTile, glassOf } from '@/render/world'
import { shellOf } from '@/sim/carve'
import type { Debris } from '@/render/debris'
import { ROOF_FLAT, type Building, type TileData } from '@/world'

/** A closed box 10 × 10 m, 6 m tall, from `x0`, wound inwards as the importer winds it. */
function box(x0: number, vertexBase: number) {
  const v = (x: number, y: number, z: number) => [x0 + x, y, z]
  const vertices = [v(0, 0, -5), v(10, 0, -5), v(10, 0, 5), v(0, 0, 5), v(0, 6, -5), v(10, 6, -5), v(10, 6, 5), v(0, 6, 5)]
  const triangles = [
    [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
    [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
  ].map((t) => t.map((i) => i! + vertexBase))
  return { vertices, triangles }
}

/** A tile with one looked house. */
function house(): TileData {
  const a = box(0, 0)
  const building: Building = {
    height: 6, base: 0, first: 0, walls: 8, roof: 2, roofKind: ROOF_FLAT, cells: null, paint: null,
    footprint: [[0, -5], [10, -5], [10, 5], [0, 5]], look: { first: 0, walls: 8, roof: 2 },
  }
  const collision = { vertices: Float32Array.from(a.vertices.flat()), triangles: Uint32Array.from(a.triangles.flat()) }
  return { buildings: [building], roads: [], cover: [], trees: [], streams: [], bridges: [], collision, look: collision, ground: new Float32Array(0), water: new Float32Array(0) } as unknown as TileData
}

/** A Debris that only counts what it is thrown. */
function pile(): Debris & { thrown: number } {
  const debris = { thrown: 0, throw: () => debris.thrown++ }
  return debris as unknown as Debris & { thrown: number }
}

const KEY = 7

describe('the glass of a window', () => {
  const tile = house()
  const range = tile.buildings[0]!.look!
  const openings = openingsOf(tile.look!, range)
  const windows = openings.filter((o) => !o.door)

  it('is a sheet a hair in front of the pane, inside the frame, for each window and no door', () => {
    const glass = glassBuffers([openings])!
    expect(glass.panes.length / 2).toBe(windows.length)
    expect(glass.positions.length).toBe(windows.length * PANE_VERTICES * 3)
    for (const o of windows) {
      const sheet = paneSheet(o)
      for (const p of sheet) {
        // Out from the wall by GLASS_OUT along its normal.
        const out = (p[0] - o.corners[0][0]) * o.normal[0] + (p[2] - o.corners[0][2]) * o.normal[2]
        expect(out).toBeCloseTo(GLASS_OUT, 6)
        expect(p[1]).toBeGreaterThan(o.corners[0][1])
        expect(p[1]).toBeLessThan(o.corners[3][1])
      }
    }
  })

  it('collapses the sheet and leaves two to four teeth in the frame when it breaks', () => {
    const o = windows[0]!
    const positions = new Float32Array(PANE_VERTICES * 3)
    const normals = new Float32Array(PANE_VERTICES * 3)
    writePane(positions, normals, 0, o, true)
    for (let v = 0; v < 6; v++) for (let k = 0; k < 3; k++) expect(positions[v * 3 + k]).toBeCloseTo(o.corners[0][k]!, 5)
    const teeth = teethOf(o)
    expect(teeth.length).toBeGreaterThanOrEqual(2)
    expect(teeth.length).toBeLessThanOrEqual(4)
    // Each tooth's base is on the frame's inner edge: two corners on the pane's rim.
    const [a, b, , d] = o.corners
    const width = Math.hypot(b[0] - a[0], b[2] - a[2])
    const height = d[1] - a[1]
    const along = [(b[0] - a[0]) / width, (b[2] - a[2]) / width]
    for (const tooth of teeth) {
      const onRim = tooth.filter((p) => {
        const u = (p[0] - a[0]) * along[0]! + (p[2] - a[2]) * along[1]!
        const v = p[1] - a[1]
        return Math.min(Math.abs(u - 0.01), Math.abs(u - (width - 0.01)), Math.abs(v - 0.01), Math.abs(v - (height - 0.01))) < 1e-6
      })
      expect(onRim.length).toBeGreaterThanOrEqual(2)
    }
    // And a whole pane has a sheet of six distinct corners in the first slots.
    writePane(positions, normals, 0, o, false)
    expect(Array.from(positions.slice(0, 3))).not.toEqual(Array.from(positions.slice(3, 6)))
  })

  it('is struck by a round ending on it, or on the cell box in front of it, not beside it or far out', () => {
    const o = windows[0]!
    const c = paneCentre(o)
    const n = o.normal
    expect(paneStruck(o, c)).toBe(true)
    expect(paneStruck(o, [c[0] - n[0] * 0.05, c[1], c[2] - n[2] * 0.05])).toBe(true)
    expect(paneStruck(o, [c[0] + n[0] * 0.7, c[1], c[2] + n[2] * 0.7])).toBe(true)
    expect(paneStruck(o, [c[0] + n[0] * 1.0, c[1], c[2] + n[2] * 1.0])).toBe(false)
    expect(paneStruck(o, [c[0], c[1] + 1.2, c[2]])).toBe(false)
    const along = [-n[2], 0, n[0]]
    expect(paneStruck(o, [c[0] + along[0]! * 0.9, c[1], c[2] + along[2]! * 0.9])).toBe(false)
  })

  it('breaks within a blast\'s radius, never past its reach or on the far side of the house, and the same every time', () => {
    const o = windows[0]!
    const c = paneCentre(o)
    const n = o.normal
    const outside = (d: number): [number, number, number] => [c[0] + n[0] * d, c[1], c[2] + n[2] * d]
    expect(paneBlasted(o, outside(2), 3)).toBe(true)
    expect(paneBlasted(o, outside(3 * GLASS_REACH + 0.5), 3)).toBe(false)
    expect(paneBlasted(o, outside(-4), 3)).toBe(false)
    const mid = outside(3 * (1 + GLASS_REACH) / 2)
    expect(paneBlasted(o, mid, 3)).toBe(paneBlasted(o, mid, 3))
    // In the falloff some break and some do not, over many panes.
    const broken = windows.filter((w) => paneBlasted(w, [5, 1, -5 - 3.6], 3)).length
    expect(broken).toBeGreaterThan(0)
  })
})

describe('glazing', () => {
  const sheetOf = (group: Group, pane: number) => glassOf(group)!.geometry.positions.slice(pane * PANE_VERTICES * 3, pane * PANE_VERTICES * 3 + 18)
  const collapsed = (sheet: Float32Array) => Array.from(sheet.slice(0, 3)).every((v, i) => v === sheet[3 + i] && v === sheet[6 + i])

  it('breaks the pane a round ends on, in place, and throws its glass once', () => {
    const tile = house()
    const group = buildTile(tile, 0)
    const debris = pile()
    const glazing = new Glazing(debris)
    glazing.attach(KEY, tile, group, new Map())
    const windows = openingsFor(tile.look!, tile.buildings[0]!.look!).map((o, i) => [o, i] as const).filter(([o]) => !o.door)
    const [o, opening] = windows[0]!
    const c = paneCentre(o)
    expect(collapsed(sheetOf(group, 0))).toBe(false)
    expect(glazing.struck([c[0] + o.normal[0] * 0.3, c[1], c[2] + o.normal[2] * 0.3])).toBe(true)
    expect(glazing.brokenPanes(KEY, 0).has(opening)).toBe(true)
    expect(collapsed(sheetOf(group, 0))).toBe(true)
    expect(debris.thrown).toBe(7)
    // The same pane again is still a round into the window, not the wall, but breaks nothing more; a miss is a miss.
    expect(glazing.struck(c)).toBe(true)
    expect(glazing.struck([c[0] + o.normal[0] * 2, c[1], c[2] + o.normal[2] * 2])).toBe(false)
    expect(debris.thrown).toBe(7)
    expect(glazing.untouched).toBe(false)
  })

  it('breaks the panes near a blast, throwing glass from the nearest few only', () => {
    const tile = house()
    const group = buildTile(tile, 0)
    const debris = pile()
    const glazing = new Glazing(debris)
    glazing.attach(KEY, tile, group, new Map())
    glazing.blast([5, 1, -7], 3)
    const broken = glazing.brokenPanes(KEY, 0)
    const openings = openingsFor(tile.look!, tile.buildings[0]!.look!)
    expect(broken.size).toBeGreaterThan(0)
    // Only the front wall (z = -5) loses glass; the far wall keeps its panes.
    for (const i of broken) expect(openings[i]!.normal[2]).toBeLessThan(-0.9)
    expect(debris.thrown).toBeLessThanOrEqual(40)
    expect(debris.thrown).toBeLessThanOrEqual(Math.min(6, broken.size) * 7)
  })

  it('takes a pane away whole, teeth and all, when the wall round it is gone, and keeps it away', () => {
    const tile = house()
    const shell = shellOf(tile, 0)!
    const openings = openingsFor(tile.look!, tile.buildings[0]!.look!)
    const window = openings.findIndex((o) => !o.door && o.normal[2] < -0.9)
    const o = openings[window]!
    const c = paneCentre(o)
    const under = shell.cellAt(c[0], o.corners[0]![1] - 0.1, -5 + 0.01)
    expect(openingStands(shell, new Set([under]), o)).toBe(false)
    const glazing = new Glazing(pile())
    let group = buildTile(tile, 0)
    glazing.attach(KEY, tile, group, new Map())
    const paneOf = (g: Group) => {
      const ids = glassOf(g)!.userData.panes as Uint32Array
      for (let i = 0; i < ids.length / 2; i++) if (ids[i * 2 + 1] === window) return i
      return -1
    }
    const nothing = (g: Group) => {
      const p = glassOf(g)!.geometry.positions.slice(paneOf(g) * PANE_VERTICES * 3, (paneOf(g) + 1) * PANE_VERTICES * 3)
      return Array.from(p).every((v, i) => v === p[i % 3])
    }
    expect(nothing(group)).toBe(false)
    glazing.holed(KEY, 0, new Set([under]), true)
    expect(glazing.brokenPanes(KEY, 0).has(window)).toBe(true)
    expect(nothing(group)).toBe(true)
    // Back after streaming out: still nothing, not teeth.
    glazing.detach(KEY)
    group = buildTile(tile, 0)
    glazing.attach(KEY, tile, group, new Map([[0, new Set([under])]]))
    expect(nothing(group)).toBe(true)
  })

  it('takes the glass of a window beside a hole without throwing it, and remembers what broke while the tile is away', () => {
    const tile = house()
    const shell = shellOf(tile, 0)!
    const openings = openingsFor(tile.look!, tile.buildings[0]!.look!)
    const window = openings.findIndex((o) => !o.door && o.normal[2] < -0.9)
    const c = paneCentre(openings[window]!)
    const hole = shell.cellAt(c[0], c[1], -5 + 0.01)
    const debris = pile()
    const glazing = new Glazing(debris)
    let group = buildTile(tile, 0)
    glazing.attach(KEY, tile, group, new Map())
    glazing.holed(KEY, 0, new Set([hole]), true)
    expect(glazing.brokenPanes(KEY, 0).has(window)).toBe(true)
    expect(debris.thrown).toBe(0)
    // Away and back: the pane is drawn broken in the new mesh.
    glazing.detach(KEY)
    group = buildTile(tile, 0)
    const ids = glassOf(group)!.userData.panes as Uint32Array
    let pane = -1
    for (let i = 0; i < ids.length / 2; i++) if (ids[i * 2 + 1] === window) pane = i
    expect(collapsed(sheetOf(group, pane))).toBe(false)
    glazing.attach(KEY, tile, group, new Map())
    expect(collapsed(sheetOf(group, pane))).toBe(true)
    // A tile attached with the hole already in it breaks the pane too.
    const other = new Glazing(pile())
    const fresh = buildTile(tile, 0)
    other.attach(KEY, tile, fresh, new Map([[0, new Set([hole])]]))
    expect(collapsed(sheetOf(fresh, pane))).toBe(true)
    void Mesh
    void Vector3
  })
})
