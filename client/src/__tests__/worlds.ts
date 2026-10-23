import { encode } from '@msgpack/msgpack'
import { SimMotor } from '@/sim/motor'
import { WORLD_VERSION } from '@/world'

/** Lengths are metres here and centimetres in the files. */
const cm = (metres: number) => Math.round(metres * 100)

/**
 * A `world.msgpack` for a square of `half` metres a side, in tiles of `tileSize`, with ground samples
 * `groundCell` apart (0 for flat), made of the `tiles` `[col, row]` (none listed is every one).
 */
export function manifestBytes(half: number, tileSize: number, groundCell = 0, tiles: [number, number][] = []): Uint8Array {
  return encode([WORLD_VERSION, [0, 0], cm(half), cm(tileSize), [], tiles.map(([col, row]) => [col, row, 0]), cm(groundCell)])
}

/** A building as a tile file holds it, metres; with no baked cells, both sides cut its cubes from its walls and roof. */
export interface TestBuilding {
  height: number
  footprint: number[][]
  base: number
  first: number
  walls: number
  roof: number
  roofKind: number
}

/**
 * A tile file holding only what collides, as `crates/shared`'s `Tile` encodes it: a collision mesh
 * (`vertices` in metres), the buildings that claim its triangles, and ground heights (metres, row-major).
 */
export function tileBytes({ vertices = [], triangles = [], ground = [], buildings = [] }: { vertices?: number[][]; triangles?: number[][]; ground?: ArrayLike<number>; buildings?: TestBuilding[] }): Uint8Array {
  const mesh = [vertices.map((v) => v.map(cm)), triangles]
  const houses = buildings.map((b) => [cm(b.height), b.footprint.map((p) => p.map(cm)), cm(b.base), b.first, b.walls, b.roof, b.roofKind])
  return encode([houses, [], mesh, Array.from(ground, cm), [], [], [], [], [], [], []])
}

/** The server's physics on a flat floor at y = 0, 200 m square. */
export function flatMotor(): Promise<SimMotor> {
  return SimMotor.create(manifestBytes(100, 200))
}
