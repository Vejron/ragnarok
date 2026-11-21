import { describe, expect, it } from 'vitest'
import type { Mesh } from '@ragnarok/engine'
import { BED_STRIDE, GARDEN_FLOWERS, GARDEN_RHUBARB, ITEM_STRIDE, gardenPlan } from '@/render/gardenplan'
import { Gardens } from '@/render/gardens'
import { grassMask } from '@/render/grassmask'
import { SURFACE_ASPHALT, SURFACE_CELL, SURFACE_GRASS, type TileSurface } from '@/sim/surface'
import { BUILDING_FARMHOUSE, BUILDING_HALL, COVER_URBAN, COVER_WOOD, type Point2 } from '@/world'

const square = (x0: number, z0: number, x1: number, z1: number): Point2[] => [[x0, z0], [x1, z0], [x1, z1], [x0, z1]]

/** A 256 m tile of grass. */
function grass(): TileSurface {
  const side = 256 / SURFACE_CELL
  return { x0: 0, z0: 0, side, cells: new Uint8Array(side * side).fill(SURFACE_GRASS) }
}

/** Old farmhouses 10 × 7 m on a grid 40 m apart, each with its road to the north (−z). */
function village(kind = BUILDING_FARMHOUSE) {
  const buildings = []
  for (let z = 20; z < 240; z += 40) for (let x = 20; x < 240; x += 40) buildings.push({ footprint: square(x, z, x + 10, z + 7), kind, front: [0, -1] as [number, number] })
  return buildings
}

const items = (plan: { items: Float32Array }) => Array.from({ length: plan.items.length / ITEM_STRIDE }, (_, i) => Array.from(plan.items.subarray(i * ITEM_STRIDE, (i + 1) * ITEM_STRIDE)))

describe('gardens', () => {
  const buildings = village()
  const plan = gardenPlan({ cover: [], buildings }, grass(), new Float32Array(0))
  const all = items(plan)

  it('grow rhubarb and flower beds round old farmhouses, the same every time', () => {
    for (const kind of [GARDEN_RHUBARB, GARDEN_FLOWERS]) expect(all.some((item) => item[0] === kind)).toBe(true)
    expect(plan.beds.length).toBeGreaterThan(0)
    expect(gardenPlan({ cover: [], buildings }, grass(), new Float32Array(0))).toEqual(plan)
  })

  it('lay a bed along the sunny (south) wall, not the door’s, and nothing inside a house', () => {
    for (let i = 0; i < plan.beds.length; i += BED_STRIDE) {
      // Each bed lies just south of a house's south wall (z = top + 7).
      const zs = [1, 3, 5, 7].map((k) => plan.beds[i + k]!)
      const wall = Math.round((Math.min(...zs) - 27) / 40) * 40 + 27
      expect(Math.min(...zs)).toBeGreaterThan(wall)
      expect(Math.max(...zs)).toBeLessThan(wall + 1.2)
    }
    for (const [, x, z] of all) {
      const inside = buildings.some(({ footprint }) => x! > footprint[0]![0] && x! < footprint[2]![0] && z! > footprint[0]![1] && z! < footprint[2]![1])
      expect(inside).toBe(false)
    }
  })

  it('stand the rhubarb at the back, away from the road', () => {
    const back = all.filter((item) => item[0] === GARDEN_RHUBARB)
    expect(back.length).toBeGreaterThan(5)
    // The road is to the north, so the back of each garden is south of its house.
    for (const [, , z] of back) expect(((z! - 20) % 40 + 40) % 40).toBeGreaterThan(7)
  })

  it('grow nothing round a factory, in a churchyard, on a road or in a wood', () => {
    expect(gardenPlan({ cover: [], buildings: village(BUILDING_HALL) }, grass(), new Float32Array(0)).items).toHaveLength(0)
    const graves = Float32Array.from(buildings.flatMap(({ footprint }) => [footprint[0]![0] + 12, footprint[0]![1] + 3]))
    expect(gardenPlan({ cover: [], buildings }, grass(), new Float32Array(0), graves).items).toHaveLength(0)
    const road = grass()
    road.cells.fill(SURFACE_ASPHALT)
    expect(gardenPlan({ cover: [], buildings }, road, new Float32Array(0)).items).toHaveLength(0)
    for (const kind of [COVER_WOOD, COVER_URBAN]) {
      expect(gardenPlan({ cover: [{ kind, rings: [square(0, 0, 256, 256)] }], buildings }, grass(), new Float32Array(0)).items).toHaveLength(0)
    }
  })

  it('keep the grass out of the beds', () => {
    const tile = { cover: [], roads: [], water: new Float32Array(), buildings, streams: [] }
    const mask = grassMask(tile, 0, 0, 256, null, undefined, null, plan.beds)
    const plain = grassMask(tile, 0, 0, 256)
    const at = (cells: Uint8Array, x: number, z: number) => cells[Math.floor(z / SURFACE_CELL) * mask.side + Math.floor(x / SURFACE_CELL)]
    const [x, z] = [(plan.beds[0]! + plan.beds[4]!) / 2, (plan.beds[1]! + plan.beds[5]!) / 2]
    expect(at(plain.cells, x, z)).toBe(1)
    expect(at(mask.cells, x, z)).toBe(0)
  })

  it('are drawn about each thing’s root, and not at all in a winter world', () => {
    const gardens = new Gardens(128, 256, () => 2)
    gardens.addTile({ col: 0, row: 0 }, { items: Float32Array.from(all.flat().map((v, i) => (i % ITEM_STRIDE === 1 || i % ITEM_STRIDE === 2 ? v - 128 : v))), beds: plan.beds })
    for (let frame = 0; frame < 200; frame++) gardens.update(0, 0)
    const chunks = gardens.root.children[0]!.children as Mesh[]
    const built = chunks.filter((mesh) => mesh.visible)
    expect(built.length).toBeGreaterThan(5)
    for (const { geometry } of built) {
      const { positions, custom } = geometry
      for (let i = 0; i < geometry.vertexCount; i++) {
        const y = positions[i * 3 + 1]!
        // Nothing much under the ground or over a rhubarb's height, and every vertex within a leaf's reach of its root.
        expect(y).toBeGreaterThan(1.8)
        expect(y).toBeLessThan(2 + 1.2)
        expect(Math.hypot(positions[i * 3]! - custom![i * 4]!, positions[i * 3 + 2]! - custom![i * 4 + 1]!)).toBeLessThan(1.5)
      }
    }
    gardens.setEnabled(false)
    expect(gardens.root.children[0]!.children).toHaveLength(0)
  })
})
