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
  return encode([WORLD_VERSION, [0, 0, 0], cm(half), cm(tileSize), [], tiles.map(([col, row]) => [col, row, 0]), cm(groundCell), 0, [], false, []])
}

/** A building's cubes when none were baked: `dims` all 0. */
export const NO_CELLS = [[0, 0, 0], [0, 0, 0], new Uint8Array(), new Uint8Array()]

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
  const houses = buildings.map((b) => [cm(b.height), b.footprint.map((p) => p.map(cm)), cm(b.base), b.first, b.walls, b.roof, b.roofKind, 0, [0, 0], NO_CELLS, null])
  return encode([houses, [], mesh, packGround(Int32Array.from(ground, cm)), [], [], new Uint8Array(), new Uint8Array(), new Uint8Array(), [], [], new Uint8Array(), [], [], [], new Uint8Array(), new Uint8Array(), [[], [], []], []])
}

/**
 * Ground heights, centimetres, as `heights` in `world.rs` writes them, at a 1 cm step so the heights
 * are exact: `[step, bin]`, each sample's difference from the plane through its left, above and
 * above-left neighbours, zigzagged, low bytes then high.
 */
export function packGround(cm: Int32Array): [number, Uint8Array] {
  const n = cm.length
  const side = Math.round(Math.sqrt(n))
  if (side * side !== n) throw new Error(`${n} ground heights are not a square grid`)
  const planes = new Uint8Array(2 * n)
  for (let i = 0; i < n; i++) {
    const x = i % side
    const predicted = i < side ? (x > 0 ? cm[i - 1]! : 0) : x > 0 ? cm[i - 1]! + cm[i - side]! - cm[i - side - 1]! : cm[i - side]!
    const residual = cm[i]! - predicted
    const zigzag = (residual << 1) ^ (residual >> 31)
    if (zigzag >>> 0 > 0xffff) throw new Error(`a ground height ${residual} cm off the plane`)
    planes[i] = zigzag & 0xff
    planes[n + i] = zigzag >>> 8
  }
  return [1, planes]
}

/** The server's physics on a flat floor at y = 0, 200 m square. */
export function flatMotor(): Promise<SimMotor> {
  return SimMotor.create(manifestBytes(100, 200))
}
