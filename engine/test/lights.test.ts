import { describe, expect, it } from 'vitest'
import { LIGHT_CELL, LIGHT_CELL_MAX, LIGHT_GRID, LightGrid } from '../src/scene/lightgrid'
import { PointLights } from '../src/scene/lights'

describe('PointLights', () => {
  it('fades a flash out over its life', () => {
    const lights = new PointLights()
    lights.flash(0, 0, 0, { color: 0xffffff, intensity: 2, radius: 10, life: 0.5 })
    lights.update(0.25)
    expect(PointLights.decay(lights.live[0]!)).toBeCloseTo(0.25)
    lights.update(0.25)
    expect(lights.live).toHaveLength(0)
  })

  it('keeps a held light at full strength for the frame after it was held, then drops it', () => {
    const lights = new PointLights()
    lights.hold(0, 0, 0, { color: 0xffffff, intensity: 2, radius: 10 })
    lights.update(1 / 60)
    expect(lights.live).toHaveLength(1)
    expect(PointLights.decay(lights.live[0]!)).toBe(1)
    lights.update(1 / 60)
    expect(lights.live).toHaveLength(0)
  })

  it('holds a light through a frame that took no time', () => {
    const lights = new PointLights()
    lights.hold(0, 0, 0, { color: 0xffffff, intensity: 2, radius: 10 })
    lights.update(0)
    expect(lights.live).toHaveLength(1)
    lights.update(0)
    expect(lights.live).toHaveLength(0)
  })
})

describe('PointLights spots', () => {
  it('normalises a spot direction and keeps the inner cone inside the outer one', () => {
    const lights = new PointLights()
    lights.hold(0, 0, 0, { color: 0xffffff, intensity: 1, radius: 30, direction: { x: 0, y: 0, z: -2 }, angle: 0.4, penumbra: 0.5 })
    const l = lights.live[0]!
    expect([l.dx, l.dy, l.dz]).toEqual([0, 0, -1])
    expect(l.cosOuter).toBeCloseTo(Math.cos(0.4))
    expect(l.cosInner).toBeCloseTo(Math.cos(0.2))
  })

  it('gives a point light cosines that let every direction through', () => {
    const lights = new PointLights()
    lights.flash(0, 0, 0, { color: 0xffffff, intensity: 1, radius: 10, life: 1 })
    expect(lights.live[0]!.cosOuter).toBe(-2)
    expect(lights.live[0]!.cosInner).toBe(-1)
  })
})

describe('LightGrid', () => {
  const half = (LIGHT_GRID / 2) * LIGHT_CELL
  const cellOf = (grid: LightGrid, x: number, z: number) => [Math.floor((x - grid.originX) / LIGHT_CELL), Math.floor((z - grid.originZ) / LIGHT_CELL)] as const

  it('snaps its corner to whole cells around the camera', () => {
    const grid = new LightGrid()
    grid.build([], 13, 0, -3)
    expect(grid.originX).toBe(8 - half)
    expect(grid.originZ).toBe(-8 - half)
  })

  it('lists a light in the cells its radius reaches and no others', () => {
    const lights = new PointLights()
    lights.flash(4, 0, 4, { color: 0xffffff, intensity: 1, radius: 6, life: 1 })
    const grid = new LightGrid()
    grid.build(lights.live, 0, 0, 0)
    expect(grid.count).toBe(1)
    const [cx, cz] = cellOf(grid, 4, 4)
    expect(grid.listed(cx, cz)).toEqual([0])
    // 6 m reaches the neighbours' edges 4 m away and their corners 4·√2 ≈ 5.7 m away, not the cells past them 12 m away.
    expect(grid.listed(cx - 1, cz)).toEqual([0])
    expect(grid.listed(cx + 1, cz)).toEqual([0])
    expect(grid.listed(cx - 1, cz - 1)).toEqual([0])
    expect(grid.listed(cx + 1, cz + 1)).toEqual([0])
    expect(grid.listed(cx + 2, cz)).toEqual([])
    expect(grid.indexCount).toBe(9)
  })

  it('records a light strength-scaled colour and its spot fields', () => {
    const lights = new PointLights()
    lights.hold(1, 2, 3, { color: 0xffffff, intensity: 2, radius: 20, direction: { x: 1, y: 0, z: 0 }, angle: 0.5 })
    const grid = new LightGrid()
    grid.build(lights.live, 0, 0, 0)
    const r = Array.from(grid.records.subarray(0, 12))
    expect(r.slice(0, 7)).toEqual([1, 2, 3, 20, 2, 2, 2])
    expect(r[7]).toBeCloseTo(Math.cos(0.5))
    expect(r.slice(8, 11)).toEqual([1, 0, 0])
  })

  it('leaves out lights off the grid, out of view or gone dark', () => {
    const lights = new PointLights()
    lights.flash(half + 50, 0, 0, { color: 0xffffff, intensity: 1, radius: 10, life: 1 })
    lights.flash(0, 0, 50, { color: 0xffffff, intensity: 1, radius: 10, life: 1 })
    lights.flash(0, 0, 0, { color: 0xffffff, intensity: 0, radius: 10, life: 1 })
    lights.flash(0, 0, -50, { color: 0xffffff, intensity: 1, radius: 10, life: 1 })
    const grid = new LightGrid()
    grid.build(lights.live, 0, 0, 0, (_x, _y, z) => z < 0)
    expect(grid.count).toBe(1)
    expect(grid.records[2]).toBe(-50)
  })

  it('keeps the strongest lights in a full cell, strongest first', () => {
    const lights = new PointLights()
    for (let i = 0; i < LIGHT_CELL_MAX + 5; i++) lights.flash(1, 0, 1, { color: 0xffffff, intensity: 1 + i, radius: 2, life: 1 })
    const grid = new LightGrid()
    grid.build(lights.live, 0, 0, 0)
    const [cx, cz] = cellOf(grid, 1, 1)
    const listed = grid.listed(cx, cz)
    expect(listed).toHaveLength(LIGHT_CELL_MAX)
    // Records are strongest first, so the cell lists 0, 1, 2…: the brightest light first and the five dimmest left out.
    expect(listed).toEqual(Array.from({ length: LIGHT_CELL_MAX }, (_, i) => i))
    expect(grid.records[4]).toBe(LIGHT_CELL_MAX + 5)
  })

  it('lays the cells out one after the other', () => {
    const lights = new PointLights()
    lights.flash(-100, 0, -100, { color: 0xffffff, intensity: 1, radius: 3, life: 1 })
    lights.flash(100, 0, 100, { color: 0xffffff, intensity: 2, radius: 3, life: 1 })
    const grid = new LightGrid()
    grid.build(lights.live, 0, 0, 0)
    expect(grid.listed(...cellOf(grid, 100, 100))).toEqual([0])
    expect(grid.listed(...cellOf(grid, -100, -100))).toEqual([1])
    expect(grid.indexCount).toBe(2)
  })
})
