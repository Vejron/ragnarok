import { describe, expect, it } from 'vitest'
import { Shell, type Tri } from '@/sim/carve'

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
    expect(shell.triangleCount).toBe(416)
    expect(shell.solidCount).toBe(48)
    let sum = 0
    for (let i = 0; i < shell.triangleCount; i++) sum += shell.triangleCell(i)
    expect(sum).toBe(14_192)
    const diagonal = Shell.of(slanted(), [])!
    let cells = 0
    for (let id = 0; id < diagonal.dims[0] * diagonal.dims[1] * diagonal.dims[2]; id++) if (diagonal.isSolid(id)) cells += id
    expect([diagonal.origin, diagonal.dims, diagonal.solidCount, cells]).toEqual([[-100, 0, 0], [9, 3, 6], 42, 3309])
  })

  it('leaves out the triangles of removed cells', () => {
    const shell = Shell.of(boxed(5, 4, 3), roof(5, 4, 3))!
    const some = shell.triangleCell(0)
    const left = shell.standing(new Set([some]))
    expect(left.length).toBeLessThan(shell.triangleCount)
    expect(shell.standing(new Set()).length).toBe(shell.triangleCount)
  })
})
