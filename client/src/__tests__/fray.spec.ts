import { describe, expect, it } from 'vitest'
import { coverMask } from '@/render/marshnoise'
import { coverIds, FRAY_REACH, frayMoves } from '@/render/fray'
import { COVER_GARDEN, COVER_PARK, COVER_ROCK, COVER_WOOD, type Point2 } from '@/world'

const square = (x0: number, z0: number, x1: number, z1: number): Point2[] => [[x0, z0], [x1, z0], [x1, z1], [x0, z1]]

/** A 64 m tile at 4 texels a metre, split down x = 32 between a wood and `kind`. */
const split = (kind: number) => ({ cover: [{ kind: COVER_WOOD, rings: [square(0, 0, 32, 64)] }, { kind, rings: [square(32, 0, 64, 64)] }] })
const n = 256
const texel = 64 / n

describe('frayed edges', () => {
  it('a wood and the heath beside it fray into each other near their edge', () => {
    const moves = frayMoves(split(COVER_PARK), 0, 0, 64, n)
    expect(moves.length).toBeGreaterThan(200)
    let into = 0, out = 0
    for (let m = 0; m < moves.length; m += 2) {
      const x = ((moves[m]! % n) + 0.5) * texel
      expect(Math.abs(x - 32)).toBeLessThanOrEqual(FRAY_REACH + texel)
      if (x < 32) into++
      else out++
    }
    // Both ways: the heath takes bites of the wood, and the wood of the heath.
    expect(into).toBeGreaterThan(50)
    expect(out).toBeGreaterThan(50)
  })

  it('a wood and a garden keep their straight line', () => {
    expect(frayMoves(split(COVER_GARDEN), 0, 0, 64, n).length).toBe(0)
  })

  it('frays the same every time', () => {
    expect(frayMoves(split(COVER_PARK), 0, 0, 64, n)).toEqual(frayMoves(split(COVER_PARK), 0, 0, 64, n))
  })

  it('nothing moves at the edge of a tile, where its neighbour cannot match it', () => {
    const moves = frayMoves(split(COVER_PARK), 0, 0, 64, n)
    for (let m = 0; m < moves.length; m += 2) {
      const row = Math.floor(moves[m]! / n)
      expect(row).toBeGreaterThan(0)
      expect(row).toBeLessThan(n - 1)
    }
  })

  it('bare rock is masked where its painted edge frays to, not where it was drawn', () => {
    const tile = split(COVER_ROCK)
    const ids = coverIds(tile, 0, 0, 64, n)
    const mask = coverMask(tile, 0, 0, 64, n, (kind) => (kind === COVER_ROCK ? 1 : 0))!
    const moves = frayMoves(tile, 0, 0, 64, n)
    expect(moves.length).toBeGreaterThan(0)
    // Every texel of wood that took the rock's cover is rock to the shader.
    let taken = 0
    for (let m = 0; m < moves.length; m += 2) {
      const [to, from] = [moves[m]!, moves[m + 1]!]
      if (ids[to] !== COVER_WOOD || ids[from] !== COVER_ROCK) continue
      taken++
      expect(mask[to]).toBeGreaterThan(0)
    }
    expect(taken).toBeGreaterThan(50)
    // And the rock reaches no further into the wood than the fraying and the blur take it.
    const row = 128
    for (let column = 0; column < Math.floor((32 - FRAY_REACH) / texel) - 2; column++) expect(mask[row * n + column]).toBe(0)
  })
})
