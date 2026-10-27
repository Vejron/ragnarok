import { describe, expect, it } from 'vitest'
import { Craters, Mesh } from '@ragnarok/engine'
import { CELL } from '@/sim/carve'
import { CHUNK, Construct, MATERIAL_BRICK, MATERIALS, cellAt, chunkKey, chunkOf, cubeCentre, materialOf } from '@/sim/construct'
import { ConstructView } from '@/render/construct'
import { WorldView } from '@/worldview'
import { flatMotor } from './worlds'
import type { Effects } from '@/render/effects'

const effects = { dust: () => {} } as unknown as Effects

/** A brick of `colour`. */
const brick = (colour: number) => ({ colour, material: MATERIAL_BRICK })

/** One frame of `view`, with nothing else in the world to move. */
const frame = (view: WorldView) => view.update(1 / 60, effects, { x: 0, y: 0, z: 0 } as never, { smoke: { spawn: () => {} } } as never)

/** The meshes `view` draws its chunks with (not the ghost cubes). */
const chunks = (view: ConstructView) => view.root.children.filter((c): c is Mesh => c instanceof Mesh && c.geometry.colors !== undefined && c.position.lengthSq() === 0)

describe('placed cubes', () => {
  it('are on the lattice of the buildings, either side of zero, as Rust finds them', () => {
    expect(cellAt(0, 0, 0)).toEqual([0, 0, 0])
    expect(cellAt(0.719, 0.72, -0.001)).toEqual([0, 1, -1])
    expect(cellAt(-0.72, -0.721, 1.44)).toEqual([-1, -2, 2])
    expect(cubeCentre([1, 0, -1])).toEqual([(CELL * 1.5) / 100, CELL / 200, -CELL / 200])
    expect(chunkOf([CHUNK - 1, -1, -CHUNK])).toEqual([0, -1, -1])
  })

  it('are found where they were put and go when taken', () => {
    const built = new Construct()
    expect(built.place([3, 0, -20], brick(4))).toBe(chunkKey([0, 0, -2]))
    expect(built.place([3, 0, -20], brick(5))).toBeNull()
    expect(built.get([3, 0, -20])).toEqual(brick(4))
    expect(built.has([3, 1, -20])).toBe(false)
    expect(built.size).toBe(1)
    expect(built.remove([3, 0, -20])).toBe(chunkKey([0, 0, -2]))
    expect(built.remove([3, 0, -20])).toBeNull()
    expect(built.size).toBe(0)
    built.forget(chunkKey([0, 0, -2]))
    expect([...built.keys()]).toEqual([])
  })

  it('are drawn a chunk to a mesh, a lone cube as twelve triangles', () => {
    const built = new Construct()
    const view = new ConstructView()
    const key = built.place([2, 2, 2], brick(0))!
    view.redraw(built, key)
    expect(chunks(view)).toHaveLength(1)
    expect(chunks(view)[0]!.geometry.positions.length / 9).toBe(12)
    // A row of cubes merges into one box's faces.
    built.place([3, 2, 2], brick(0))
    built.place([4, 2, 2], brick(0))
    view.redraw(built, key)
    expect(chunks(view)).toHaveLength(1)
    expect(chunks(view)[0]!.geometry.positions.length / 9).toBe(12)
    for (const x of [2, 3, 4]) built.remove([x, 2, 2])
    view.redraw(built, key)
    expect(chunks(view)).toHaveLength(0)
  })

  it('that fell come down to the ground and are cleared away', () => {
    const view = new ConstructView()
    view.drop([{ pos: [0, 20, 0], cube: brick(1) }, { pos: [0, 21, 0], cube: brick(1) }], () => 0)
    const piece = view.root.children.find((c) => c instanceof Mesh && c.position.y > 0)!
    expect(piece).toBeDefined()
    for (let i = 0; i < 120; i++) view.update(1 / 60, effects)
    expect(piece.position.y).toBe(0)
    for (let i = 0; i < 60 * 5; i++) view.update(1 / 60, effects)
    expect(view.root.children.includes(piece)).toBe(false)
  })

  it('are solid to the stickman once the world view has them', async () => {
    const view = new WorldView({ add: () => {}, craters: new Craters() } as never)
    view.motor = await flatMotor()
    view.built({ live: false, placed: [{ x: 0, y: 0, z: 0, color: 2, material: 0 }, { x: 0, y: 1, z: 0, color: 2, material: 0 }], removed: [], fell: [] })
    frame(view)
    const top = (2 * CELL) / 100
    const down = view.motor.castRay([0.36, 10, 0.36], [0, -1, 0], 20)
    expect(down?.t).toBeCloseTo(10 - top, 3)
    view.built({ live: true, placed: [], removed: [{ x: 0, y: 1, z: 0 }], fell: [{ x: 0, y: 0, z: 0 }] })
    frame(view)
    expect(view.construct.size).toBe(0)
    expect(view.motor.castRay([0.36, 10, 0.36], [0, -1, 0], 9.9)).toBeNull()
  })

  it('hide the faces they share with a cube of the next chunk', () => {
    const built = new Construct()
    const view = new ConstructView()
    // Either side of the line between chunk 0 and chunk -1 along x.
    const a = built.place([0, 0, 0], brick(0))!
    const b = built.place([-1, 0, 0], brick(0))!
    expect(a).not.toBe(b)
    view.redraw(built, a)
    view.redraw(built, b)
    const counts = chunks(view).map((mesh) => mesh.geometry.positions.length / 9)
    expect(counts).toEqual([10, 10])
  })

  it('this player wore down are drawn darker, and whole again once broken and placed anew', () => {
    const built = new Construct()
    const view = new ConstructView()
    const key = built.place([2, 2, 2], brick(4))!
    view.redraw(built, key)
    const before = Math.max(...chunks(view)[0]!.geometry.colors!)
    expect(built.dent([2, 2, 2], materialOf(MATERIAL_BRICK).hp / 2)).toBe(true)
    expect(built.dent([9, 9, 9], 10)).toBe(false)
    expect(built.wear([2, 2, 2])).toBeCloseTo(0.5)
    view.redraw(built, key)
    expect(Math.max(...chunks(view)[0]!.geometry.colors!)).toBeLessThan(before)
    built.remove([2, 2, 2])
    built.place([2, 2, 2], brick(4))
    expect(built.wear([2, 2, 2])).toBe(0)
  })

  it('keep the material the server says, from the same table as its own', () => {
    expect(MATERIALS[MATERIAL_BRICK]).toEqual({ id: 0, name: 'Brick', hp: 40, blastReach: 1 })
    const view = new WorldView({ add: () => {}, craters: new Craters() } as never)
    view.built({ live: false, placed: [{ x: 1, y: 0, z: 1, color: 3, material: MATERIAL_BRICK }], removed: [], fell: [] })
    expect(view.construct.get([1, 0, 1])).toEqual({ colour: 3, material: MATERIAL_BRICK })
  })
})
