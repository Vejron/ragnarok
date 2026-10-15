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

describe('carve', () => {
  it('cuts a building into the same cells as the server', () => {
    // Pinned by `cell_numbers_match_the_fixture` in crates/shared/src/carve.rs.
    const shell = Shell.of(boxed(5, 4, 3), roof(5, 4, 3))!
    expect(shell.cell).toBe(100)
    expect(shell.dims).toEqual([6, 4, 5])
    expect(shell.triangles.length).toBe(416)
    expect(shell.solidCount).toBe(71)
    expect(shell.triangles.reduce((sum, t) => sum + t.cell, 0)).toBe(23_456)
  })

  it('leaves out the triangles of removed cells', () => {
    const shell = Shell.of(boxed(5, 4, 3), roof(5, 4, 3))!
    const some = shell.triangles[0]!.cell
    const left = shell.standing(new Set([some]))
    expect(left.length).toBeLessThan(shell.triangles.length)
    expect(shell.standing(new Set()).length).toBe(shell.triangles.length)
  })
})
