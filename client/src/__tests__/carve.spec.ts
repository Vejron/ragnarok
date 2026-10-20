import { describe, expect, it } from 'vitest'
import { Shell, unpack, unpackListed, type Tri } from '@/sim/carve'

/** A box `w × d × h` metres, walls only, as `boxed` in `crates/shared/src/carve.rs`. */
function boxed(w: number, d: number, h: number): Tri[] {
  const [W, D, H] = [w * 100, d * 100, h * 100]
  const quad = (a: number[], b: number[], c: number[], e: number[]): Tri[] => [[...a, ...b, ...c] as Tri, [...a, ...c, ...e] as Tri]
  return [
    ...quad([0, 0, 0], [W, 0, 0], [W, H, 0], [0, H, 0]),
    ...quad([W, 0, 0], [W, 0, D], [W, H, D], [W, H, 0]),
    ...quad([W, 0, D], [0, 0, D], [0, H, D], [W, H, D]),
    ...quad([0, 0, D], [0, 0, 0], [0, H, 0], [0, H, D]),
  ]
}

function roof(w: number, d: number, h: number): Tri[] {
  const [W, D, H] = [w * 100, d * 100, h * 100]
  return [
    [0, H, 0, W, H, 0, W, H, D],
    [0, H, 0, W, H, D, 0, H, D],
  ]
}

/** As `slanted` in `crates/shared/src/carve.rs`: a 3 m wall slanting across the grid. */
function slanted(): Tri[] {
  const [a, b, h] = [[-37, 12], [712, 530], 300] as const
  return [
    [a[0], 0, a[1], b[0], 0, b[1], b[0], h, b[1]],
    [a[0], 0, a[1], b[0], h, b[1], a[0], h, a[1]],
  ]
}

describe('carve', () => {
  it('cuts a building into the same cells as the server', () => {
    // Pinned by `cell_numbers_match_the_fixture` in crates/shared/src/carve.rs.
    const shell = Shell.of(boxed(5, 4, 3), roof(5, 4, 3))!
    expect(shell.cell).toBe(100)
    expect(shell.dims).toEqual([5, 3, 4])
    expect(shell.solidCount).toBe(48)
    // The faces, as `cell_numbers_match_the_fixture` sums them: each number by its place in its triangle.
    const sum = (faces: Int32Array) => faces.reduce((n, v, i) => n + v * ((i % 9) + 1), 0)
    const faces = shell.faces(new Set())
    expect([faces.length / 9, sum(faces)]).toEqual([36, 352_800])
    const diagonal = Shell.of(slanted(), [])!
    let cells = 0
    for (let id = 0; id < diagonal.dims[0] * diagonal.dims[1] * diagonal.dims[2]; id++) if (diagonal.isSolid(id)) cells += id
    expect([diagonal.origin, diagonal.dims, diagonal.solidCount, cells]).toEqual([[-100, 0, 0], [9, 3, 6], 42, 3309])
    const slant = diagonal.faces(new Set())
    expect([slant.length / 9, sum(slant)]).toEqual([72, 960_300])
    // The boxes, summed the same way: each number by its place in its box.
    const boxes = (s: Shell) => {
      const b = s.boxes(new Set())
      return [b.length / 6, b.reduce((n, v, i) => n + v * ((i % 6) + 1), 0)]
    }
    expect([boxes(shell), boxes(diagonal)]).toEqual([[5, 27_500], [6, 40_200]])
  })

  it('unpacks the cells the importer baked', () => {
    // The bytes `packed_cells_match_the_fixture` in crates/shared/src/carve.rs pins for this box.
    const cut = Shell.of(boxed(5, 4, 3), roof(5, 4, 3))!
    const baked = Shell.fromCells({
      origin: [0, 0, 0],
      dims: [5, 3, 4],
      solid: Uint8Array.from([63, 198, 15, 0, 1, 192, 57, 0, 0]),
      roof: Uint8Array.from([0, 4, 255, 255, 15]),
    })!
    for (let id = 0; id < 60; id++) {
      expect([baked.isSolid(id), baked.isRoof(id)]).toEqual([cut.isSolid(id), cut.isRoof(id)])
    }
    expect(baked.faces(new Set())).toEqual(cut.faces(new Set()))
    expect([...baked.cells]).toEqual([...Array(60).keys()].filter((id) => cut.isSolid(id)))
    // A run of zero bytes over several layers repeats the layer below it, cells listed and all.
    const tower = unpackListed(Uint8Array.from([1, 0, 4]), 48, 8)
    expect([[...tower.flags.keys()].filter((i) => tower.flags[i]), [...tower.ids]]).toEqual([[0, 8, 16, 24, 32, 40], [0, 8, 16, 24, 32, 40]])
    // A long run of zero bytes, with a two-byte length (`baked_cells_come_back_as_they_were`).
    const flags = unpack(Uint8Array.from([1, 0, 169, 2, 128]), 8 * 300, 8 * 300)
    expect([...flags.keys()].filter((i) => flags[i])).toEqual([0, 8 * 299 + 7])
  })

  it('collides with the cubes that are left', () => {
    const shell = Shell.of(boxed(5, 4, 3), roof(5, 4, 3))!
    const whole = shell.faces(new Set())
    // A cell out of the middle of a wall opens a hole: its four sides become faces.
    const hole = shell.faces(new Set([shell.cellAt(2.5, 1.5, 0.5)]))
    expect(hole.length).toBeGreaterThan(whole.length)
    // The cell is gone from the boxes, which still cover every other cube once.
    const boxes = shell.boxes(new Set([shell.cellAt(2.5, 1.5, 0.5)]))
    let cubes = 0
    for (let i = 0; i < boxes.length; i += 6) cubes += ((boxes[i + 3]! - boxes[i]!) * (boxes[i + 4]! - boxes[i + 1]!) * (boxes[i + 5]! - boxes[i + 2]!)) / 100 ** 3
    expect(cubes).toBe(shell.solidCount - 1)
  })
})
