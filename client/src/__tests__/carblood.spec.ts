import { afterEach, describe, expect, it, vi } from 'vitest'
import { Geometry, Mesh, Node, SCAR_BLOOD, Vector3 } from '@ragnarok/engine'
import { PART_HEAD, PART_TORSO } from '@/net/codec'
import { spatterCar } from '@/render/carblood'
import { Gore } from '@/render/gore'
import { HullMarks } from '@/render/hullmarks'
import { toonMaterial } from '@/render/toon'

afterEach(() => vi.restoreAllMocks())

/** A pane `z` metres along +z from the origin, square to it, 4 m across. */
function pane(z: number, name: string) {
  const positions = new Float32Array([-2, -2, z, 2, -2, z, 2, 2, z, -2, -2, z, 2, 2, z, -2, 2, z])
  return new Mesh(new Geometry({ positions }), toonMaterial({ name }))
}

describe('a soldier shot in a car', () => {
  it('spatters the glass behind them and the glass the round came through, and nothing smokes out of the blood', () => {
    vi.spyOn(Math, 'random').mockReturnValue(0.5)
    const added: { style: number; radius: number }[] = []
    const scars = { alloc: () => 1, add: (_slot: number, scar: { style: number; radius: number }) => void added.push(scar), release: () => {}, setGrime: () => {} }
    const marks = new HullMarks(scars as never)
    const root = new Node()
    // The rear window a metre behind the head, and the windscreen half a metre in front of it.
    root.add(pane(1, 'glass'))
    root.add(pane(-0.5, 'glass'))
    // Shot from ahead (-z), so the round goes on along +z: from bearing 0, which faces +z here.
    spatterCar(marks, 7, root, { x: 0, y: 0, z: 0 }, 0, 30, true)
    expect(added.length).toBe(5)
    expect(added.every((scar) => scar.style === SCAR_BLOOD)).toBe(true)
    // The splats thrown back onto the windscreen are smaller than the one behind.
    expect(added[3]!.radius).toBeLessThan(added[0]!.radius)
    expect(added[4]!.radius).toBeLessThan(added[3]!.radius)
    expect(marks.vents(7)).toHaveLength(0)
  })

  it('marks only glass: a seat in the way takes nothing, and nothing goes on past it', () => {
    vi.spyOn(Math, 'random').mockReturnValue(0.5)
    const added: unknown[] = []
    const scars = { alloc: () => 1, add: (_slot: number, scar: unknown) => void added.push(scar), release: () => {}, setGrime: () => {} }
    const root = new Node()
    root.add(pane(0.6, 'seat'))
    root.add(pane(1, 'glass'))
    spatterCar(new HullMarks(scars as never), 7, root, { x: 0, y: 0, z: 0 }, 0, 30, true)
    expect(added).toHaveLength(0)
  })

  it('leaves nothing when there is nothing near enough to land on', () => {
    const added: unknown[] = []
    const scars = { alloc: () => 1, add: (_slot: number, scar: unknown) => void added.push(scar), release: () => {}, setGrime: () => {} }
    const root = new Node()
    root.add(pane(5, 'glass'))
    spatterCar(new HullMarks(scars as never), 7, root, { x: 0, y: 0, z: 0 }, 0, 30, true)
    expect(added).toHaveLength(0)
  })

  it('bleeds from the head it is drawn from when the server gives no spot, not from above it', () => {
    const particles = { spawn: vi.fn() }
    const gore = new Gore({} as never, particles as never, {} as never, {} as never, () => 0)
    const head = new Vector3(3, 1.4, -2)
    const seated = { position: head, seated: true, pin: () => (out: Vector3) => out.copy(head), stump: () => {} }
    expect(gore.hit(seated as never, PART_HEAD, 0, 20).y).toBeCloseTo(1.4)
    expect(gore.hit(seated as never, PART_TORSO, 0, 20).y).toBeLessThan(1.4)
    // Standing, it is drawn from its middle: the head is above it.
    const standing = { ...seated, seated: false }
    expect(gore.hit(standing as never, PART_HEAD, 0, 20).y).toBeCloseTo(2.15)
  })
})
