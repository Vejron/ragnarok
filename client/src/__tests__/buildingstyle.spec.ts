import { describe, expect, it } from 'vitest'
import { WALL_TRIMS } from '@ragnarok/engine'
import { SEED_MOST, Surface, buildingBuffers, buildingColors, buildingSeed, buildingSurfaces, cornered, dressingOf, surfaceOf } from '@/render/buildingmesh'
import { CURTAIN_RGBA, GLASS_RGBA, PANE_VERTICES, glassBuffers, paneDressing } from '@/render/glass'
import { FRAME_RGB, openingsOf } from '@/render/windows'
import { BUILDING_BARN, BUILDING_PLAIN, CLAD_BRICK, CLAD_WOOD, ROOF_GABLE } from '@/world'

/** A closed box 10 × 10 m, 6 m tall, from `x0` with its foot at `y0`, wound inwards as the importer winds it. */
function box(x0: number, y0: number, vertexBase: number) {
  const v = (x: number, y: number, z: number) => [x0 + x, y0 + y, z]
  const vertices = [v(0, 0, -5), v(10, 0, -5), v(10, 0, 5), v(0, 0, 5), v(0, 6, -5), v(10, 6, -5), v(10, 6, 5), v(0, 6, 5)]
  const triangles = [
    [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
    [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
  ].map((t) => t.map((i) => i! + vertexBase))
  return { vertices, triangles }
}

/** A tile of two looked houses side by side, the second with its foot 2 m up. */
function street() {
  const [a, b] = [box(0, 0, 0), box(20, 2, 8)]
  const look = { vertices: Float32Array.from([...a.vertices, ...b.vertices].flat()), triangles: Uint32Array.from([...a.triangles, ...b.triangles].flat()) }
  const house = (first: number) => ({ first: 0, walls: 0, roof: 0, roofKind: ROOF_GABLE, kind: BUILDING_PLAIN, cells: null, paint: [null, null] as [number | null, number | null], clad: [0, 0] as [number, number], look: { first, walls: 8, roof: 2, kind: BUILDING_PLAIN } })
  return { buildings: [house(0), house(12)], bridges: [], collision: { vertices: new Float32Array(), triangles: new Uint32Array() }, look }
}

/** A row of `count` houses with nothing mapped of them and no look. */
function row(count: number, kind = BUILDING_PLAIN) {
  return { buildings: Array.from({ length: count }, (_, i) => ({ first: 0, walls: 0, roof: 0, roofKind: ROOF_GABLE, kind, cells: null, paint: [null, null] as [number | null, number | null], clad: [0, 0] as [number, number], look: null, footprint: [[i * 100, 0], [i * 100 + 8, 0], [i * 100 + 8, 8], [i * 100, 8]] as [number, number][] })) }
}

describe('a building\'s own style', () => {
  it('gives each building of each tile a seed of its own, the same every time', () => {
    const seeds = new Set<number>()
    for (let salt = 0; salt < 20; salt++) for (let index = 0; index < 50; index++) {
      const seed = buildingSeed(salt, index)
      expect(seed).toBe(buildingSeed(salt, index))
      expect(Number.isInteger(seed) && seed >= 1 && seed <= SEED_MOST).toBe(true)
      seeds.add(seed)
    }
    // A thousand buildings, and hardly two alike.
    expect(seeds.size).toBeGreaterThan(980)
  })

  it('does not paint neighbouring houses in the order of the palette', () => {
    const tile = row(60)
    const walls = tile.buildings.map((_, index) => buildingColors(tile, index, 5).wall)
    expect(new Set(walls).size).toBeGreaterThan(5)
    // In the palette's order no house would be painted as the one before it, and each colour would come back every tenth house.
    expect(walls.some((wall, i) => i > 0 && wall === walls[i - 1])).toBe(true)
    expect(walls.every((wall, i) => i < 10 || wall === walls[i - 10])).toBe(false)
    // The roof is picked apart from the wall: houses with the same walls have different roofs.
    const roofsOf = new Map<number, Set<number>>()
    tile.buildings.forEach((_, index) => {
      const { wall, roof } = buildingColors(tile, index, 5)
      roofsOf.set(wall, (roofsOf.get(wall) ?? new Set()).add(roof))
    })
    expect([...roofsOf.values()].some((roofs) => roofs.size > 1)).toBe(true)
  })

  it('builds houses of one colour in more than one way, and trims them in more than one colour', () => {
    const tile = row(200)
    const styles = tile.buildings.map((_, index) => buildingSurfaces(tile, index, 1))
    const white = styles.filter((_, index) => buildingColors(tile, index, 1).wall === 0xeee9dc)
    expect(white.length).toBeGreaterThan(5)
    expect(new Set(white.map((s) => s.wall)).size).toBeGreaterThan(1)
    expect(new Set(styles.map((s) => s.frame.join())).size).toBeGreaterThan(2)
    expect(new Set(styles.map((s) => s.door.join())).size).toBeGreaterThan(2)
    // A barn keeps its tarred door.
    const barns = row(30, BUILDING_BARN)
    expect(new Set(barns.buildings.map((_, index) => buildingSurfaces(barns, index, 1).door.join())).size).toBe(1)
  })

  it('writes a house\'s seed, the foot of its walls and its plinth on its patterned faces, and adds no vertex for them', () => {
    const tile = street()
    const buffers = buildingBuffers(tile, 3, new Set())!
    const styles = [0, 1].map((index) => buildingSurfaces(tile, index, 3))
    expect(styles[0]!.foot).toBe(0)
    expect(styles[1]!.foot).toBe(2)
    for (const style of styles) {
      expect(style.plinth).toBeGreaterThanOrEqual(0.3)
      expect(style.plinth).toBeLessThanOrEqual(0.5)
      expect(style.seed).toBeGreaterThan(0)
    }
    expect(styles[0]!.seed).not.toBe(styles[1]!.seed)
    const { custom, ranges } = buffers
    // Bit for bit, as vertices are shared (`weld`).
    const bits = (floats: Float32Array) => new Uint32Array(floats.buffer, floats.byteOffset, floats.length)
    const [positions, normals, colors] = [bits(buffers.positions), bits(buffers.normals), bits(buffers.colors)]
    const alike = new Set<string>()
    for (let v = 0; v < custom.length / 4; v++) {
      const building = v < ranges![2]! ? 0 : 1
      const [surface, seed, foot, plinth] = [custom[v * 4]!, custom[v * 4 + 1]!, custom[v * 4 + 2]!, custom[v * 4 + 3]!]
      if (surface === 0) expect([seed, foot, plinth]).toEqual([0, 0, 0])
      else {
        expect(seed).toBeCloseTo(styles[building]!.seed, 6)
        expect(foot).toBe(styles[building]!.foot)
        expect(plinth).toBeCloseTo(styles[building]!.plinth, 6)
      }
      // Without the seed, foot and plinth no two vertices of a building are alike: they split none.
      alike.add([building, positions[v * 3], positions[v * 3 + 1], positions[v * 3 + 2], normals[v * 3], normals[v * 3 + 1], normals[v * 3 + 2], colors[v * 3], colors[v * 3 + 1], colors[v * 3 + 2], surface].join())
    }
    expect(alike.size).toBe(custom.length / 4)
  })

  it('puts a board over each corner of a boarded house in the colour of its frames, none on a brick one, and adds no vertex for them', () => {
    const tile = street()
    tile.buildings[0]!.clad = [CLAD_WOOD, 0]
    tile.buildings[1]!.clad = [CLAD_BRICK, 0]
    const [wooden, brick] = [0, 1].map((index) => buildingSurfaces(tile, index, 3))
    expect(wooden!.corners).toBeGreaterThan(0)
    expect(brick!.corners).toBe(0)
    // The shader paints the board from the engine's list: the colour there is the frames'.
    expect([...WALL_TRIMS[0]!]).toEqual([...FRAME_RGB])
    expect([...WALL_TRIMS[wooden!.corners - 1]!]).toEqual([...wooden!.frame])
    const buffers = buildingBuffers(tile, 3, new Set())!
    const { custom, uvs, positions, ranges } = buffers
    expect(uvs.length).toBe((positions.length / 3) * 2)
    let [boarded, atCorner] = [0, 0]
    for (let v = 0; v < custom.length / 4; v++) {
      const ends = [uvs[v * 2]!, uvs[v * 2 + 1]!]
      const wall = surfaceOf(custom[v * 4]!) === Surface.Upright || surfaceOf(custom[v * 4]!) === Surface.Lap
      // Ignore the faces round the windows' reveals and the like: only the outer walls, which stand on the box's sides.
      if (v >= ranges![2]! || !wall) {
        expect(ends).toEqual([0, 0])
        continue
      }
      expect(custom[v * 4]).toBe(cornered(wooden!.wall, wooden!.corners))
      // Each wall of the box is 10 m long: a vertex is as far from one end as it is short of the other.
      expect(ends[0]! + ends[1]!).toBeCloseTo(10, 4)
      boarded++
      const corner = Math.abs(Math.abs(positions[v * 3 + 2]!) - 5) < 1e-4 && (Math.abs(positions[v * 3]!) < 1e-4 || Math.abs(positions[v * 3]! - 10) < 1e-4)
      if (corner) {
        expect(Math.min(...ends)).toBeCloseTo(0, 4)
        atCorner++
      }
    }
    expect(boarded).toBeGreaterThan(0)
    expect(atCorner).toBeGreaterThan(0)
    // The same house with no boards over its corners has as many vertices: a wall's ends split none.
    tile.buildings[0]!.clad = [CLAD_BRICK, 0]
    const plain = buildingBuffers(tile, 3, new Set())!
    expect(plain.positions.length).toBe(positions.length)
    expect(plain.uvs.every((u) => u === 0)).toBe(true)
  })
})

describe('the panes of a building', () => {
  const tile = street()
  const openings = openingsOf(tile.look, tile.buildings[0]!.look)
  const windows = openings.filter((o) => !o.door)

  it('have curtains and lamps by the building\'s seed, the same every time, and more lamps in a home than in a barn', () => {
    const count = (kind: number, what: 'curtain' | 'lamp') => {
      let n = 0
      for (let seed = 1; seed <= 200; seed++) for (let opening = 0; opening < 10; opening++) {
        const pane = paneDressing(dressingOf(kind, seed), opening)
        expect(pane).toEqual(paneDressing(dressingOf(kind, seed), opening))
        if (what === 'curtain' ? pane.curtain !== null : pane.lamp > 0) n++
      }
      return n / 2000
    }
    expect(count(BUILDING_PLAIN, 'lamp')).toBeGreaterThan(0.35)
    expect(count(BUILDING_PLAIN, 'lamp')).toBeLessThan(0.55)
    expect(count(BUILDING_BARN, 'lamp')).toBeLessThan(0.12)
    expect(count(BUILDING_PLAIN, 'curtain')).toBeGreaterThan(0.2)
    expect(count(BUILDING_BARN, 'curtain')).toBe(0)
  })

  it('carry their curtain and their lamp on the sheet only, so a broken pane\'s teeth are clear and dark', () => {
    expect(windows.length).toBeGreaterThan(0)
    const dressing = { seed: 7, curtained: 1, lit: 1 }
    const glass = glassBuffers([openings], [dressing])!
    for (let pane = 0; pane < windows.length; pane++) {
      const { curtain, lamp } = paneDressing(dressing, glass.panes[pane * 2 + 1]!)
      expect(curtain).not.toBeNull()
      expect(lamp).toBeGreaterThan(0)
      for (let v = 0; v < PANE_VERTICES; v++) {
        const at = (pane * PANE_VERTICES + v) * 4
        const rgba = [...glass.colors.subarray(at, at + 4)]
        const want = v < 6 ? CURTAIN_RGBA[curtain!]! : GLASS_RGBA
        want.forEach((c, k) => expect(rgba[k]).toBeCloseTo(c, 5))
        expect(glass.custom[at]).toBeCloseTo(v < 6 ? lamp : 0, 5)
      }
    }
    // A building with no dressing has clear, dark panes.
    const bare = glassBuffers([openings], [null])!
    expect([...bare.custom].every((x) => x === 0)).toBe(true)
    expect(bare.colors[3]).toBeCloseTo(GLASS_RGBA[3], 5)
  })
})
