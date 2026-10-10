import { decode } from '@msgpack/msgpack'

/** Mirrors `crates/shared/src/world.rs`. Coordinates are metres: +X east, +Z south, +Y up. */

export const WORLD_VERSION = 2

export const ROAD_MAJOR = 0
export const ROAD_MINOR = 1
export const ROAD_PATH = 2

export type Point2 = [number, number]

export interface Building {
  height: number
  footprint: Point2[]
}

export interface Road {
  width: number
  kind: number
  points: Point2[]
}

export interface CollisionMesh {
  vertices: Float32Array
  triangles: Uint32Array
}

/** One entry of the manifest's tile list: which tile, and the hash of its file. */
export interface TileRef {
  col: number
  row: number
  hash: number
}

/** `world.msgpack`: where the world is and which tiles make it up. */
export interface WorldData {
  origin: [number, number]
  halfExtent: number
  /** Side of a tile, metres. Tile `(col, row)` starts at `-halfExtent + col * tileSize`. */
  tileSize: number
  spawns: Point2[]
  tiles: TileRef[]
  /** Collision that is not part of any tile. Empty in a generated world; tests fill it in. */
  collision: CollisionMesh
}

/** One tile file: the buildings and roads inside a square of the world. Metres, world-absolute. */
export interface TileData {
  buildings: Building[]
  roads: Road[]
  collision: CollisionMesh
}

/** FNV-1a, matching `world_hash` on the server. */
export function worldHash(bytes: Uint8Array): number {
  let hash = 0x811c9dc5
  for (const byte of bytes) hash = Math.imul(hash ^ byte, 0x01000193) >>> 0
  return hash
}

const metres = (cm: number) => cm / 100
const point = ([x, z]: number[]): Point2 => [metres(x!), metres(z!)]
const noCollision = (): CollisionMesh => ({ vertices: new Float32Array(), triangles: new Uint32Array() })

export function decodeManifest(bytes: Uint8Array): WorldData {
  const [version, origin, halfExtent, tileSize, spawns, tiles] = decode(bytes) as [number, [number, number], number, number, number[][], number[][]]
  if (version !== WORLD_VERSION) throw new Error(`world version ${version}, expected ${WORLD_VERSION}`)
  return {
    origin,
    halfExtent: metres(halfExtent),
    tileSize: metres(tileSize),
    spawns: spawns.map(point),
    tiles: tiles.map(([col, row, hash]) => ({ col: col!, row: row!, hash: hash! })),
    collision: noCollision(),
  }
}

export function decodeTile(bytes: Uint8Array): TileData {
  const [buildings, roads, [vertices, triangles]] = decode(bytes) as [[number, number[][]][], [number, number, number[][]][], [number[][], number[][]]]
  return {
    buildings: buildings.map(([height, footprint]) => ({ height: metres(height), footprint: footprint.map(point) })),
    roads: roads.map(([width, kind, points]) => ({ width: metres(width), kind, points: points.map(point) })),
    collision: {
      vertices: Float32Array.from(vertices.flat(), metres),
      triangles: Uint32Array.from(triangles.flat()),
    },
  }
}

/** Loads the manifest. `hash` is what the server reports in `welcome` if the worlds match. */
export async function loadManifest(url = '/world.msgpack'): Promise<{ world: WorldData; hash: number }> {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${url}: ${response.status}; run \`cargo run -p osm-import\``)
  const bytes = new Uint8Array(await response.arrayBuffer())
  return { world: decodeManifest(bytes), hash: worldHash(bytes) }
}

/** Fetches one tile and checks it is the one the manifest names. */
export async function fetchTile(ref: TileRef, base = '/'): Promise<TileData> {
  const url = `${base}tiles/${ref.col}_${ref.row}.msgpack`
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${url}: ${response.status}`)
  const bytes = new Uint8Array(await response.arrayBuffer())
  if (worldHash(bytes) !== ref.hash) throw new Error(`${url} does not match the manifest; reload`)
  return decodeTile(bytes)
}
