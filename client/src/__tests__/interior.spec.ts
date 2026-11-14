import { describe, expect, it } from 'vitest'
import { interior_of } from '@/sim/wasm/sim'
import { Part, insideFor } from '@/sim/interior'
import { Shell } from '@/sim/carve'
import { doorway, facadeOf } from '@/render/facade'
import { openingsOf } from '@/render/windows'
import { lookChunk, lookFracture } from '@/render/lookfracture'

/** A house 12 × 8 m with 6 m walls under a flat roof, as `interior.rs`'s tests build one: wound inwards, centimetres. */
function house() {
  const footprint = [[0, 0], [1200, 0], [1200, 800], [0, 800]] as const
  const walls: number[] = []
  footprint.forEach((p, i) => {
    const q = footprint[(i + 1) % footprint.length]!
    walls.push(p[0], 0, p[1], q[0], 0, q[1], q[0], 600, q[1])
    walls.push(p[0], 0, p[1], q[0], 600, q[1], p[0], 600, p[1])
  })
  const roof = [0, 600, 0, 1200, 600, 0, 1200, 600, 800, 0, 600, 0, 1200, 600, 800, 0, 600, 800]
  return { footprint: Int32Array.from(footprint.flat()), walls: Int32Array.from(walls), roof: Int32Array.from(roof) }
}

/** The house as a tile's look, in metres, with its range. */
function looked() {
  const { footprint, walls, roof } = house()
  const flat = [...walls, ...roof]
  const look = { vertices: Float32Array.from(flat.map((v) => v / 100)), triangles: Uint32Array.from(flat.slice(0, flat.length / 3).map((_, i) => i)) }
  return { look, range: { first: 0, walls: walls.length / 9, roof: roof.length / 9, footprint } }
}

describe('the inside of a house', () => {
  it('is the same through WebAssembly as the server works it out', () => {
    const { footprint, walls, roof } = house()
    const raw = interior_of(footprint, walls, roof)
    // As `the_same_house_gets_the_same_floors` in `interior.rs` sums it.
    const mask = (1n << 64n) - 1n
    let sum = 0n
    for (let i = 0; i < raw.length; i += 10) {
      sum = (sum * 31n + BigInt(raw[i + 9]!)) & mask
      for (let k = 0; k < 9; k++) sum = (sum * 31n + BigInt(raw[i + k]! >>> 0)) & mask
    }
    expect(raw.length / 10).toBe(PARTS)
    expect(Number(sum % 1_000_000_007n)).toBe(SUM)
  })

  it('has two floors and stairs, and a shed has none', () => {
    const { look, range } = looked()
    const inside = insideFor(look, range)
    const levels = new Set(inside.pieces.filter((p) => p.kind === Part.Floor).map((p) => p.poly[0][1]))
    expect(levels.size).toBe(2)
    expect(inside.pieces.some((p) => p.kind === Part.Ramp)).toBe(true)
    // The upper floor is a slab: it has an underside.
    expect(inside.pieces.some((p) => p.kind === Part.Ceiling)).toBe(true)
    // The steps are drawn and not collided with.
    expect(inside.collided.length / 9).toBe(inside.pieces.filter((p) => p.kind !== Part.Step).length)
    expect(insideFor(look, { first: 0, walls: range.walls, roof: range.roof }).pieces).toEqual([])
  })

  it('is cut into the cells of the house, which keep their numbers', () => {
    const { look, range } = looked()
    const { walls, roof } = house()
    const shell = Shell.ofFlat(walls, roof)!
    const before = shell.solidCount
    const upper = Math.max(...insideFor(look, range).pieces.filter((p) => p.kind === Part.Floor).map((p) => p.poly[0][1]))
    // The middle of the upper floor is air until the house is furnished.
    const at = shell.index(Math.floor(300 / 72), Math.floor((upper * 100 - 1 - shell.origin[1]) / 72), Math.floor(500 / 72))
    expect(shell.isSolid(at)).toBe(false)
    shell.furnish(insideFor(look, range).collided)
    expect(shell.isSolid(at)).toBe(true)
    expect(shell.solidCount).toBeGreaterThan(before + 200)
  })

  it('is drawn with a hole where a cell of a floor is gone', () => {
    const { look, range } = looked()
    const { walls, roof } = house()
    const shell = Shell.ofFlat(walls, roof)!
    shell.furnish(insideFor(look, range).collided)
    const upper = Math.max(...insideFor(look, range).pieces.filter((p) => p.kind === Part.Floor).map((p) => p.poly[0][1]))
    const gone = shell.index(Math.floor(300 / 72), Math.floor((upper * 100 - 1 - shell.origin[1]) / 72), Math.floor(500 / 72))
    const [lo, hi] = [shell.centre(gone).map((v) => (v - 36) / 100), shell.centre(gone).map((v) => (v + 36) / 100)]
    // How many drawn triangles lie in the floor's plane with their middle inside the cell.
    const inCell = (positions: Float32Array) => {
      let n = 0
      for (let i = 0; i < positions.length; i += 9) {
        const ys = [positions[i + 1]!, positions[i + 4]!, positions[i + 7]!]
        if (ys.some((y) => Math.abs(y - upper) > 1e-3)) continue
        const mid = [0, 1, 2].map((k) => (positions[i + k]! + positions[i + 3 + k]! + positions[i + 6 + k]!) / 3)
        if (mid[0]! > lo[0]! && mid[0]! < hi[0]! && mid[2]! > lo[2]! && mid[2]! < hi[2]!) n++
      }
      return n
    }
    const rgb = [0.5, 0.5, 0.5] as const
    // A dent beside it slices the floor there without taking any of it.
    const dented = lookFracture(look, range, shell, new Set(), new Map([[gone, 10]]), rgb, rgb)
    expect(inCell(dented.positions)).toBeGreaterThan(0)
    const holed = lookFracture(look, range, shell, new Set([gone]), new Map(), rgb, rgb)
    expect(inCell(holed.positions)).toBe(0)
    // The piece that went is what a falling section is made of.
    const fallen = lookChunk(look, range, shell, new Set([gone]), [0, 0, 0], rgb, rgb)
    expect(inCell(fallen.positions)).toBeGreaterThan(0)
  })

  it('opens the door of a house with rooms and leaves a shed shut', () => {
    const { look, range } = looked()
    const door = openingsOf(look, range).find((o) => o.door)!
    const [a, b, , d] = doorway(door).corners
    const mid = [(a[0] + b[0]) / 2, (a[1] + d[1]) / 2, (a[2] + b[2]) / 2]
    const covers = (walls: readonly (readonly number[])[][]) =>
      walls.some((poly) => {
        // In the door's wall and round its middle: by the box of the piece, which the cuts leave square.
        const lo = [0, 1, 2].map((k) => Math.min(...poly.map((p) => p[k]!)))
        const hi = [0, 1, 2].map((k) => Math.max(...poly.map((p) => p[k]!)))
        return [0, 1, 2].every((k) => mid[k]! >= lo[k]! - 1e-3 && mid[k]! <= hi[k]! + 1e-3) && poly.length > 3
      })
    const open = facadeOf(look, range)
    expect(open.inside.length).toBeGreaterThan(0)
    expect(open.floor).toEqual([])
    const shut = facadeOf(look, { first: 0, walls: range.walls, roof: range.roof })
    expect(shut.inside).toEqual([])
    expect(shut.floor.length).toBeGreaterThan(0)
    // The shut house's walls are its own triangles, the door's wall whole; the open one's is cut round the doorway.
    expect(open.walls.length).toBeGreaterThan(shut.walls.length)
    expect(covers(open.walls)).toBe(false)
  })
})

const PARTS = 112
const SUM = 490359646
