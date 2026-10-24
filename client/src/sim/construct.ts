/**
 * The cubes players placed with bricks, kept as `crates/shared/src/construct.rs` keeps them: one sparse
 * layer for the whole world on the lattice of the buildings' cells, in chunks of `CHUNK` cubes a side.
 * The server says what is placed, dug, blasted and falls (`Built`); the client keeps the same cubes,
 * draws each changed chunk (`render/construct.ts`) and gives it to the physics (`SimMotor.setConstructChunk`),
 * which merges it into boxes with the server's own code.
 */

import { CELL } from './carve'
import tables from './tables.generated.json'

/** What a cube is made of, as `construct::MATERIALS` has it (from `tables.generated.json`). */
export interface Material {
  id: number
  name: string
  /** Bullet damage that breaks a cube of it. */
  hp: number
  /** Share of a blast's wall-biting reach that takes it out. */
  blastReach: number
}

/** Every material, by id. */
export const MATERIALS: readonly Material[] = tables.materials

/** Mirrors `MATERIAL_BRICK`. */
export const MATERIAL_BRICK = 0

/** The material `id` names; brick for an unknown one. Mirrors `material`. */
export const materialOf = (id: number): Material => MATERIALS[id] ?? MATERIALS[MATERIAL_BRICK]!

/** One placed cube: its colour (the builder's) and its material. */
export interface Cube {
  colour: number
  material: number
}

/** Cubes a chunk is along each side. Mirrors `CHUNK`. */
export const CHUNK = 16
/** Cubes in one chunk. */
export const CHUNK_CELLS = CHUNK * CHUNK * CHUNK

/** A cube's place on the world lattice: its corner with the smallest coordinates is `pos * CELL` centimetres. */
export type Pos = readonly [number, number, number]

/** The six cubes that share a face with a cube, as steps. Mirrors `NEIGHBOURS`. */
export const NEIGHBOURS: readonly Pos[] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]

/** The lattice cube holding the point `(x, y, z)`, metres. Mirrors `cell_at`. */
export const cellAt = (x: number, y: number, z: number): Pos => [Math.floor(Math.floor(x * 100) / CELL), Math.floor(Math.floor(y * 100) / CELL), Math.floor(Math.floor(z * 100) / CELL)]

/** The middle of cube `pos`, metres. */
export const cubeCentre = (pos: Pos): [number, number, number] => [(pos[0] * CELL + CELL / 2) / 100, (pos[1] * CELL + CELL / 2) / 100, (pos[2] * CELL + CELL / 2) / 100]

/** The chunk cube `pos` is in. */
export const chunkOf = (pos: Pos): Pos => [Math.floor(pos[0] / CHUNK), Math.floor(pos[1] / CHUNK), Math.floor(pos[2] / CHUNK)]

/** The corner of chunk `chunk` with the smallest coordinates, centimetres. */
export const chunkOrigin = (chunk: Pos): [number, number, number] => [chunk[0] * CHUNK * CELL, chunk[1] * CHUNK * CELL, chunk[2] * CHUNK * CELL]

/** A chunk's name in a map. */
export const chunkKey = (chunk: Pos) => `${chunk[0]},${chunk[1]},${chunk[2]}`

const mod = (v: number) => ((v % CHUNK) + CHUNK) % CHUNK

/** Cube `pos`'s number in its chunk: `(y * CHUNK + z) * CHUNK + x`, as `slot` numbers it. */
const slot = (pos: Pos) => (mod(pos[1]) * CHUNK + mod(pos[2])) * CHUNK + mod(pos[0])

interface Chunk {
  readonly chunk: Pos
  /** One byte a cube, in cell order: 0 for none, else its colour plus one (`Construct::chunk_cells`). */
  readonly cells: Uint8Array
  /** Each cube's material, in cell order. */
  readonly materials: Uint8Array
  count: number
}

/** A cube's name in a map. */
export const posKey = (pos: Pos) => `${pos[0]},${pos[1]},${pos[2]}`

/** Every placed cube the server has told of. */
export class Construct {
  private readonly chunks = new Map<string, Chunk>()
  private count = 0

  /** How many cubes are placed. */
  get size() {
    return this.count
  }

  /** The cube at `pos`, or undefined. */
  get(pos: Pos): Cube | undefined {
    const data = this.chunks.get(chunkKey(chunkOf(pos)))
    const at = slot(pos)
    const byte = data?.cells[at] ?? 0
    return byte === 0 ? undefined : { colour: byte - 1, material: data!.materials[at]! }
  }

  has(pos: Pos): boolean {
    return this.get(pos) !== undefined
  }

  /** Puts `cube` at `pos`. Returns the key of the chunk that changed, or null when one was there. */
  place(pos: Pos, cube: Cube): string | null {
    const chunk = chunkOf(pos)
    const key = chunkKey(chunk)
    let data = this.chunks.get(key)
    if (!data) this.chunks.set(key, (data = { chunk, cells: new Uint8Array(CHUNK_CELLS), materials: new Uint8Array(CHUNK_CELLS), count: 0 }))
    const at = slot(pos)
    if (data.cells[at] !== 0) return null
    data.cells[at] = Math.min(255, cube.colour + 1)
    data.materials[at] = cube.material
    data.count++
    this.count++
    return key
  }

  /** Takes the cube at `pos` away. Returns the key of the chunk that changed, or null when there was none. */
  remove(pos: Pos): string | null {
    const key = chunkKey(chunkOf(pos))
    const data = this.chunks.get(key)
    const at = slot(pos)
    if (!data || data.cells[at] === 0) return null
    data.cells[at] = 0
    data.materials[at] = 0
    this.hurt.delete(posKey(pos))
    data.count--
    this.count--
    // An empty chunk stays as an empty array until it is drawn and collided with as nothing (`forget`).
    return key
  }

  /**
   * Damage this player's own rounds have done to cubes, by `posKey`: drawn darker, as a building's
   * wall is, until the server breaks the cube out. Other players' rounds are not seen.
   */
  private readonly hurt = new Map<string, number>()

  /** A round of this player's did `damage` to the cube at `pos`. Returns whether there is one to hurt. */
  dent(pos: Pos, damage: number): boolean {
    if (!this.has(pos)) return false
    const key = posKey(pos)
    this.hurt.set(key, (this.hurt.get(key) ?? 0) + damage)
    return true
  }

  /** How much of the cube at `pos` this player's rounds have taken, 0 (whole) to 1 (about to break). */
  wear(pos: Pos): number {
    const taken = this.hurt.get(posKey(pos))
    const cube = taken ? this.get(pos) : undefined
    return cube ? Math.min(1, taken! / materialOf(cube.material).hp) : 0
  }

  /** Chunk `key`'s place and cubes, if it is known. */
  chunk(key: string): { chunk: Pos; cells: Uint8Array; materials: Uint8Array; count: number } | undefined {
    return this.chunks.get(key)
  }

  /** Drops chunk `key` if it holds no cubes, once nothing needs its empty array any more. */
  forget(key: string) {
    if (this.chunks.get(key)?.count === 0) this.chunks.delete(key)
  }

  /** Every chunk's key. */
  keys(): IterableIterator<string> {
    return this.chunks.keys()
  }
}
