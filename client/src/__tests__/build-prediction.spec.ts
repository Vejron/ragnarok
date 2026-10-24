import { describe, expect, it } from 'vitest'
import { BUILD_COOLDOWN_TICKS } from '@/sim/consts'
import { BrickTrigger, mayPlace, type BuildWorld } from '@/sim/bricks'
import { Construct, MATERIAL_BRICK, cubeCentre, type Pos } from '@/sim/construct'
import { WorldView } from '@/worldview'
import type { Effects } from '@/render/effects'
import { flatMotor } from './worlds'

const brick = (colour: number) => ({ colour, material: MATERIAL_BRICK })

/** Flat ground at 0 with nothing on it but `construct`'s cubes, a building wherever `building` says, and nobody standing about. */
const flat = (construct = new Construct(), building: (p: readonly number[]) => boolean = () => false): BuildWorld => ({
  construct,
  groundAt: () => 0,
  cubeBlocked: () => false,
  buildingNear: (p) => building(p),
  body: null,
})

const effects = { dust: () => {} } as unknown as Effects
const frame = (view: WorldView) => view.update(1 / 60, effects, { x: 0, y: 0, z: 0 } as never, { smoke: { spawn: () => {} } } as never)

describe("the local player's bricks", () => {
  it('follow the server\'s rules for where a cube may go', () => {
    const world = flat()
    expect(mayPlace(world, [0, 0, 0])).toBe(true)
    expect(mayPlace(world, [0, 5, 0])).toBe(false)
    expect(mayPlace(world, [0, -2, 0])).toBe(false)
    world.construct.place([0, 0, 0], brick(0))
    expect(mayPlace(world, [0, 0, 0])).toBe(false)
    expect(mayPlace(world, [0, 1, 0])).toBe(true)
    expect(mayPlace(flat(new Construct(), () => true), [0, 5, 0])).toBe(true)
    expect(mayPlace({ ...flat(), cubeBlocked: () => true }, [0, 0, 0])).toBe(false)
    const at = cubeCentre([0, 0, 0])
    expect(mayPlace({ ...flat(), body: [at[0], 0.9, at[2]] }, [0, 0, 0])).toBe(false)
  })

  it('are used once fire is held and every cooldown after', () => {
    const trigger = new BrickTrigger()
    const used = Array.from({ length: BUILD_COOLDOWN_TICKS * 3 }, () => trigger.tick(true)).filter(Boolean).length
    expect(used).toBe(3)
    expect(trigger.tick(false)).toBe(false)
  })

  it('show a cube at once, which the server then confirms', async () => {
    const view = new WorldView({ add: () => {} } as never)
    view.motor = await flatMotor()
    const pos: Pos = [0, 0, 0]
    view.predict(pos, brick(3), performance.now() + 1000)
    expect(view.construct.get(pos)).toEqual(brick(3))
    frame(view)
    expect(view.motor.castRay([0.36, 10, 0.36], [0, -1, 0], 20)?.t).toBeCloseTo(10 - 0.72, 3)
    view.built({ live: true, placed: [{ x: 0, y: 0, z: 0, color: 3, material: MATERIAL_BRICK }], removed: [], fell: [] })
    // Confirmed, so the wait running out no longer takes it back.
    view.predict([5, 0, 5], brick(3), -1)
    frame(view)
    expect(view.construct.get(pos)).toEqual(brick(3))
    expect(view.construct.has([5, 0, 5])).toBe(false)
  })

  it('take a cube back that the server never placed, and put back one it never dug', async () => {
    const view = new WorldView({ add: () => {} } as never)
    view.motor = await flatMotor()
    view.predict([0, 0, 0], brick(1), -1)
    frame(view)
    expect(view.construct.size).toBe(0)
    expect(view.motor.castRay([0.36, 10, 0.36], [0, -1, 0], 9.9)).toBeNull()
    view.built({ live: false, placed: [{ x: 2, y: 0, z: 2, color: 4, material: MATERIAL_BRICK }], removed: [], fell: [] })
    view.predict([2, 0, 2], null, performance.now() + 1000)
    expect(view.construct.has([2, 0, 2])).toBe(false)
    // The server agrees: it stays dug.
    view.built({ live: true, placed: [], removed: [{ x: 2, y: 0, z: 2 }], fell: [] })
    frame(view)
    expect(view.construct.has([2, 0, 2])).toBe(false)
    view.built({ live: false, placed: [{ x: 3, y: 0, z: 3, color: 4, material: MATERIAL_BRICK }], removed: [], fell: [] })
    view.predict([3, 0, 3], null, -1)
    frame(view)
    expect(view.construct.get([3, 0, 3])).toEqual(brick(4))
  })

  it('give way to the cube the server put there instead', () => {
    const view = new WorldView({ add: () => {} } as never)
    view.predict([0, 0, 0], brick(1), performance.now() + 1000)
    view.built({ live: true, placed: [{ x: 0, y: 0, z: 0, color: 6, material: MATERIAL_BRICK }], removed: [], fell: [] })
    expect(view.construct.get([0, 0, 0])).toEqual(brick(6))
    expect(view.construct.size).toBe(1)
  })
})
