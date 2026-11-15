import { describe, expect, it } from 'vitest'
import { SCAR_DENT, SCAR_LIGHT_POINTS, SCAR_POINTS, SCAR_SLOTS, SCAR_SLOT_FLOATS, SCAR_VEC4S, Scars, type Scar } from '../src/scene/scars'

const scar = (x: number, style = SCAR_DENT): Scar => ({ x, y: 0, z: 0, radius: 0.3, dx: 0, dy: 0, dz: 1, style, depth: 0.1, birth: 0 })
/** The x of every scar slot `handle` lists, as the shader reads them: the first `count`. */
function listed(scars: Scars, handle: number) {
  const o = (handle - 1) * SCAR_SLOT_FLOATS
  const count = scars.data[o]!
  return Array.from({ length: count }, (_, i) => scars.data[o + 4 + i * SCAR_VEC4S * 4]!)
}

describe('Scars', () => {
  it('lists a mesh\'s scars for its shader, and forgets them when its slot is given back', () => {
    const scars = new Scars()
    const handle = scars.alloc()
    expect(handle).toBeGreaterThan(0)
    scars.add(handle, scar(1))
    scars.add(handle, scar(2))
    expect(listed(scars, handle).slice(SCAR_LIGHT_POINTS)).toEqual([1, 2])
    expect(scars.takeDirty()).toEqual([handle - 1])
    scars.release(handle)
    expect(listed(scars, scars.alloc())).toEqual([])
  })

  it('keeps a bullet\'s chips in a ring of their own, so a hail of them never wipes out a shell\'s dent', () => {
    const scars = new Scars()
    const handle = scars.alloc()
    scars.add(handle, scar(7))
    for (let i = 0; i < 50; i++) scars.add(handle, scar(100 + i), true)
    const xs = listed(scars, handle)
    expect(xs).toContain(7)
    expect(xs.slice(0, SCAR_LIGHT_POINTS).every((x) => x >= 144)).toBe(true)
  })

  it('overwrites a mesh\'s oldest heavy scar once it has all it keeps', () => {
    const scars = new Scars()
    const handle = scars.alloc()
    const heavy = SCAR_POINTS - SCAR_LIGHT_POINTS
    for (let i = 0; i < heavy + 3; i++) scars.add(handle, scar(i))
    const xs = listed(scars, handle)
    expect(xs).toHaveLength(SCAR_POINTS)
    expect(xs.slice(SCAR_LIGHT_POINTS)).not.toContain(0)
    expect(xs).toContain(heavy + 2)
  })

  it('has no slot to give once every one is taken', () => {
    const scars = new Scars()
    for (let i = 0; i < SCAR_SLOTS; i++) expect(scars.alloc()).toBeGreaterThan(0)
    expect(scars.alloc()).toBe(0)
  })
})
