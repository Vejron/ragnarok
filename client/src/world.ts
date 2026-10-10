import { decode } from '@msgpack/msgpack'

/** Mirrors `crates/shared/src/world.rs`. Coordinates are metres: +X east, +Z south, +Y up. */

export const WORLD_VERSION = 7

export const ROAD_MAJOR = 0
export const ROAD_MINOR = 1
export const ROAD_PATH = 2

export const COVER_WATER = 0
export const COVER_WOOD = 1
export const COVER_PARK = 2
export const COVER_FARM = 3
export const COVER_SAND = 4
export const COVER_URBAN = 5
export const COVER_INDUSTRIAL = 6

export const TREE_BIRCH = 0
export const TREE_PINE = 1
export const TREE_SPRUCE = 2
export const TREE_ORNAMENTAL = 3
export const TREE_BROADLEAF = 4
export const TREE_COLUMNAR = 5
export const TREE_LOWWIDE = 6

export type Point2 = [number, number]

/** One tree: its foot at `(x, base, z)`, `height` tall with a crown of radius `crown`. */
export interface Tree {
  x: number
  z: number
  base: number
  height: number
  crown: number
  /** `TREE_*`: the archetype. */
  kind: number
  /** Whether the trunk is a collider (see `Tree::solid`). */
  solid: boolean
  /** The trunk's collider: radius, and height above `base`. Mirrors `Tree::trunk_radius` / `trunk_height`. */
  trunkRadius: number
  trunkHeight: number
}

export interface Building {
  height: number
  footprint: Point2[]
  /** Height of the floor, metres. The top of the roof is at `base + height`. */
  base: number
  /** This building's triangles in the tile's collision mesh: walls, then roof (then the floor, not drawn). */
  first: number
  walls: number
  roof: number
  /** `ROOF_*`: what the laser data fitted the roof as. */
  roofKind: number
}

export const ROOF_FLAT = 0
export const ROOF_SHED = 1
export const ROOF_GABLE = 2
export const ROOF_HIP = 3

export interface Road {
  width: number
  kind: number
  points: Point2[]
}

/** A patch of ground cover: rings filled even-odd, so inner rings are holes. Drawn, not simulated. */
export interface Cover {
  kind: number
  rings: Point2[][]
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
  /** Distance between ground samples, metres; 0 when the ground is the plane y = 0. */
  groundCell: number
  /** Collision that is not part of any tile. Empty in a generated world; tests fill it in. */
  collision: CollisionMesh
}

/** One tile file: the buildings and roads inside a square of the world. Metres, world-absolute. */
export interface TileData {
  buildings: Building[]
  roads: Road[]
  /** Ground cover, in the order to paint it. */
  cover: Cover[]
  trees: Tree[]
  collision: CollisionMesh
  /** Ground heights, metres, row-major (rows along +Z), `tileSize / groundCell + 1` a side. Empty on a flat world. */
  ground: Float32Array
}

/** FNV-1a, matching `world_hash` on the server. */
export function worldHash(bytes: Uint8Array): number {
  let hash = 0x811c9dc5
  for (const byte of bytes) hash = Math.imul(hash ^ byte, 0x01000193) >>> 0
  return hash
}

const metres = (cm: number) => cm / 100
const point = ([x, z]: number[]): Point2 => [metres(x!), metres(z!)]

/** `Tree::trunk_radius` on the server, in centimetres with the same integer arithmetic. */
function trunkRadiusCm(kind: number, heightCm: number): number {
  const [radius, height] =
    kind === TREE_PINE || kind === TREE_SPRUCE || kind === TREE_COLUMNAR
      ? [20, 1600]
      : kind === TREE_ORNAMENTAL
        ? [10, 600]
        : kind === TREE_BROADLEAF
          ? [30, 1400]
          : kind === TREE_LOWWIDE
            ? [15, 800]
            : [15, 1200]
  return Math.min(60, Math.max(5, Math.trunc((radius * heightCm) / height)))
}

const noCollision = (): CollisionMesh => ({ vertices: new Float32Array(), triangles: new Uint32Array() })

export function decodeManifest(bytes: Uint8Array): WorldData {
  const [version, origin, halfExtent, tileSize, spawns, tiles, groundCell] = decode(bytes) as [number, [number, number], number, number, number[][], number[][], number]
  if (version !== WORLD_VERSION) throw new Error(`world version ${version}, expected ${WORLD_VERSION}`)
  return {
    origin,
    halfExtent: metres(halfExtent),
    tileSize: metres(tileSize),
    spawns: spawns.map(point),
    tiles: tiles.map(([col, row, hash]) => ({ col: col!, row: row!, hash: hash! })),
    groundCell: metres(groundCell),
    collision: noCollision(),
  }
}

export function decodeTile(bytes: Uint8Array): TileData {
  const [buildings, roads, [vertices, triangles], ground, cover, trees] = decode(bytes) as [
    [number, number[][], number, number, number, number, number][],
    [number, number, number[][]][],
    [number[][], number[][]],
    number[],
    [number, number[][][]][],
    [number, number, number, number, number, number, boolean][],
  ]
  return {
    buildings: buildings.map(([height, footprint, base, first, walls, roof, roofKind]) => ({
      height: metres(height),
      footprint: footprint.map(point),
      base: metres(base),
      first,
      walls,
      roof,
      roofKind,
    })),
    roads: roads.map(([width, kind, points]) => ({ width: metres(width), kind, points: points.map(point) })),
    cover: cover.map(([kind, rings]) => ({ kind, rings: rings.map((ring) => ring.map(point)) })),
    trees: trees.map(([x, z, base, height, crown, kind, solid]) => ({
      x: metres(x),
      z: metres(z),
      base: metres(base),
      height: metres(height),
      crown: metres(crown),
      kind,
      solid,
      trunkRadius: metres(trunkRadiusCm(kind, height)),
      trunkHeight: metres(Math.trunc(height / 2)),
    })),
    collision: {
      vertices: Float32Array.from(vertices.flat(), metres),
      triangles: Uint32Array.from(triangles.flat()),
    },
    ground: Float32Array.from(ground, metres),
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
