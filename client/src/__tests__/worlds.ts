import { encode } from '@msgpack/msgpack'
import { SimMotor } from '@/sim/motor'
import { WORLD_VERSION } from '@/world'

/** Lengths are metres here and centimetres in the files. */
const cm = (metres: number) => Math.round(metres * 100)

/** A `world.msgpack` for a square of `half` metres a side, in tiles of `tileSize`, with ground samples `groundCell` apart (0 for flat). */
export function manifestBytes(half: number, tileSize: number, groundCell = 0): Uint8Array {
  return encode([WORLD_VERSION, [0, 0], cm(half), cm(tileSize), [], [], cm(groundCell)])
}

/**
 * A tile file holding only what collides: a mesh claimed by no building (`vertices` in metres) and
 * ground heights (metres, row-major), as `crates/shared`'s `Tile` encodes them.
 */
export function tileBytes({ vertices = [], triangles = [], ground = [] }: { vertices?: number[][]; triangles?: number[][]; ground?: ArrayLike<number> }): Uint8Array {
  const mesh = [vertices.map((v) => v.map(cm)), triangles]
  return encode([[], [], mesh, Array.from(ground, cm), [], [], [], [], [], [], []])
}

/** The server's physics on a flat floor at y = 0, 200 m square. */
export function flatMotor(): Promise<SimMotor> {
  return SimMotor.create(manifestBytes(100, 200))
}
