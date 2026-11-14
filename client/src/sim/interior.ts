/**
 * What stands inside a house: its floors and the stairs between them, as the server works them out
 * (`crates/shared/src/interior.rs`), through the WebAssembly build (`interior_of`), so the client
 * draws and collides with the same rooms without a second copy of the rules. Nothing of it is in the
 * world file: it comes from a building's footprint and its look.
 */

import { interior_of } from './wasm/sim'

type P = [number, number, number]

/** What a part of a house's inside is. Mirrors `PART_*` in `interior.rs`. */
export const Part = { Floor: 0, Ceiling: 1, Ramp: 2, Step: 3 } as const

/** A building's triangles in the look, and what its inside is worked out from and remembered in. */
export interface Furnishable {
  first: number
  walls: number
  roof: number
  /** Its footprint, `x, z` pairs in centimetres, as the world file has it. */
  footprint?: Int32Array
  /** Its inside once worked out (`interior_of`): ten numbers a triangle, its corners in centimetres and its `Part`. */
  inside?: Int32Array
}

/** One triangle of a house's inside, in metres, and what it is. */
export interface InsidePiece {
  poly: [P, P, P]
  kind: number
}

/** A house's inside: every piece to draw, and the ones collided with as flat centimetres (nine numbers a triangle) for its cells. */
export interface Inside {
  pieces: readonly InsidePiece[]
  collided: Int32Array
}

const NONE: Inside = { pieces: [], collided: new Int32Array(0) }
const memo = new WeakMap<Int32Array, Inside>()

/**
 * The inside of building `range` of `look`, worked out once and kept on `range` (so it goes along to
 * a worker that has no WebAssembly). Empty for a house without rooms, or a building whose footprint
 * is not known.
 */
export function insideFor(look: { vertices: Float32Array; triangles: Uint32Array }, range: Furnishable): Inside {
  if (!range.inside) {
    if (!range.footprint) return NONE
    const flat = (from: number, count: number) => {
      const out = new Int32Array(count * 9)
      for (let k = 0; k < count * 3; k++) {
        const v = look.triangles[from * 3 + k]! * 3
        out[k * 3] = Math.round(look.vertices[v]! * 100)
        out[k * 3 + 1] = Math.round(look.vertices[v + 1]! * 100)
        out[k * 3 + 2] = Math.round(look.vertices[v + 2]! * 100)
      }
      return out
    }
    range.inside = interior_of(range.footprint, flat(range.first, range.walls), flat(range.first + range.walls, range.roof))
  }
  const raw = range.inside
  if (raw.length === 0) return NONE
  let inside = memo.get(raw)
  if (!inside) {
    const pieces: InsidePiece[] = []
    const collided: number[] = []
    for (let i = 0; i < raw.length; i += 10) {
      const corner = (k: number): P => [raw[i + k * 3]! / 100, raw[i + k * 3 + 1]! / 100, raw[i + k * 3 + 2]! / 100]
      const kind = raw[i + 9]!
      pieces.push({ poly: [corner(0), corner(1), corner(2)], kind })
      if (kind !== Part.Step) for (let k = 0; k < 9; k++) collided.push(raw[i + k]!)
    }
    inside = { pieces, collided: Int32Array.from(collided) }
    memo.set(raw, inside)
  }
  return inside
}
