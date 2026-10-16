import { decode } from '@msgpack/msgpack'

/** Mirrors `crates/shared/src/world.rs`. Coordinates are metres: +X east, +Z south, +Y up. */

export const WORLD_VERSION = 10

/** Where `world.msgpack`, `tiles/` and `minimap.png` are served from: `/` normally, `/dev/` for the dev world. */
export let WORLD_BASE: string = withSlash(import.meta.env.VITE_WORLD_BASE || '/')

function withSlash(base: string): string {
  return base.endsWith('/') ? base : `${base}/`
}

/**
 * In dev, with no `VITE_WORLD_BASE`, the world the map editor activated (`worlds/active.json`,
 * served by Vite at `/worlds/`), else the generated world at the site root. Sets `WORLD_BASE`.
 */
export async function resolveWorldBase(): Promise<string> {
  if (import.meta.env.VITE_WORLD_BASE || !import.meta.env.DEV) return WORLD_BASE
  try {
    const response = await fetch('/worlds/active.json')
    if (response.ok) {
      const { name } = (await response.json()) as { name?: string }
      if (name && /^[a-z0-9_-]+$/.test(name) && (await fetch(`/worlds/${name}/world.msgpack`, { method: 'HEAD' })).ok) {
        WORLD_BASE = withSlash(`/worlds/${name}`)
      }
    }
  } catch {
    // No editor worlds: the default stands.
  }
  return WORLD_BASE
}

export const ROAD_MAJOR = 0
export const ROAD_MINOR = 1
export const ROAD_PATH = 2
/** A track on its own ballast bed, with sleepers. */
export const RAIL_TRAIN = 0
/** A track set flush into the street. */
export const RAIL_TRAM = 1
/** How much wider a track's ballast bed is than its gauge, metres (`BALLAST_EXTRA` in the importer). */
export const BALLAST_EXTRA = 2.2

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
export const TREE_ROCK_BOULDER = 7
export const TREE_ROCK_SLAB = 8
export const TREE_BUSH = 9

/**
 * How an archetype's collider is sized and whether it can fall. Mirrors `TREE_KINDS` in
 * `crates/shared/src/world.rs`; `parity.spec.ts` checks it against the server's table.
 */
export interface TreeKindDef {
  kind: number
  name: string
  /** Collider radius, centimetres, at `refHeight`, scaled with the object's height. */
  radius: number
  refHeight: number
  /** When non-zero the radius is this percentage of the crown radius instead (rocks). */
  radiusPctOfCrown: number
  maxRadius: number
  /** Collider height as a percentage of the object's height. */
  colliderHeightPct: number
  /** Whether shots, blasts and vehicles can fell it. Rocks cannot. */
  destructible: boolean
}

const treeKind = (kind: number, name: string, radius: number, refHeight: number): TreeKindDef => ({
  kind, name, radius, refHeight, radiusPctOfCrown: 0, maxRadius: 60, colliderHeightPct: 50, destructible: true,
})

export const TREE_KINDS: readonly TreeKindDef[] = [
  treeKind(TREE_BIRCH, 'Birch', 15, 1200),
  treeKind(TREE_PINE, 'Pine', 20, 1600),
  treeKind(TREE_SPRUCE, 'Spruce', 20, 1600),
  treeKind(TREE_ORNAMENTAL, 'Ornamental', 10, 600),
  treeKind(TREE_BROADLEAF, 'Broadleaf', 30, 1400),
  treeKind(TREE_COLUMNAR, 'Columnar', 20, 1600),
  treeKind(TREE_LOWWIDE, 'Low and wide', 15, 800),
  { ...treeKind(TREE_ROCK_BOULDER, 'Boulder', 0, 1), radiusPctOfCrown: 80, maxRadius: 200, colliderHeightPct: 80, destructible: false },
  { ...treeKind(TREE_ROCK_SLAB, 'Rock slab', 0, 1), radiusPctOfCrown: 85, maxRadius: 250, colliderHeightPct: 85, destructible: false },
  treeKind(TREE_BUSH, 'Bush', 10, 100),
]

/** The definition of an archetype. Unknown kinds are birches, as they are drawn. */
export const treeKindDef = (kind: number): TreeKindDef => TREE_KINDS.find((def) => def.kind === kind) ?? TREE_KINDS[0]!

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

/**
 * One bridge's triangles in the tile's collision mesh: `deck` triangles of concrete (deck, piers,
 * parapets) from `first`, then `steel` triangles (pylon, stay cables).
 */
export interface Bridge {
  first: number
  deck: number
  steel: number
}

/**
 * Where each building's triangles are in the tile's collision mesh, as `[start, end)`: from its
 * `first` to the start of whatever the importer wrote next (another building, a bridge, or the
 * end), which takes in the floor that is collided with but not drawn. The server cuts the same
 * ranges (`WorldFile::assemble`), so both sides remove the same triangles.
 */
export function buildingTriangleRanges(tile: TileData): [number, number][] {
  const end = tile.collision.triangles.length / 3
  const starts = [...tile.buildings.map((b) => b.first), ...tile.bridges.map((b) => b.first)]
  return tile.buildings.map((building) => {
    const next = Math.min(end, ...starts.filter((start) => start > building.first))
    return [Math.min(building.first, next), next] as [number, number]
  })
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

/** A railway track's centre line. Drawn, not simulated. */
export interface Rail {
  /** Distance between the rails, metres. */
  gauge: number
  kind: number
  points: Point2[]
}

/** A patch of ground cover: rings filled even-odd, so inner rings are holes. Drawn, not simulated. */
export interface Cover {
  kind: number
  rings: Point2[][]
}

/** A stream's centre line in the direction the water flows, drawn as a ribbon draped on the ground. */
export interface Stream {
  width: number
  points: Point2[]
}

/** Units per unit of slope in `TileData.detail` (`DETAIL_SLOPE_SCALE` in `crates/shared`). */
export const DETAIL_SLOPE_SCALE = 63.5

/** `Tile::water` entry (centimetres) for a ground sample with no water. */
const NO_WATER_CM = -32768

/** `TileData.water` entry for a ground sample with no water. */
export const NO_WATER = -Infinity

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
  /**
   * Which tiles of the square grid are part of the world, `grid × grid` row-major, or `null` when
   * every one is. Tiles outside it have no ground and are walled off.
   */
  inside?: boolean[] | null
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
  /**
   * Height of the water surface at each ground sample, metres (same layout as `ground`), or
   * `NO_WATER`. Empty when the tile has no water. The depth under a sample is `water - ground`.
   */
  water: Float32Array
  /**
   * Slopes `(dh/dx, dh/dz)` of the ground on a finer grid than `ground`, `n × n` with
   * `n = sqrt(length / 2)`, in units of `1 / DETAIL_SLOPE_SCALE`; for lighting only. Absent or empty
   * when the tile has none.
   */
  detail?: Int8Array
  /** Narrow watercourses, flowing along their points. */
  streams: Stream[]
  bridges: Bridge[]
  rails: Rail[]
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
export function trunkRadiusCm(kind: number, heightCm: number, crownCm: number): number {
  const def = treeKindDef(kind)
  const radius = def.radiusPctOfCrown > 0 ? Math.trunc((crownCm * def.radiusPctOfCrown) / 100) : Math.trunc((def.radius * heightCm) / def.refHeight)
  return Math.min(def.maxRadius, Math.max(5, radius))
}

const noCollision = (): CollisionMesh => ({ vertices: new Float32Array(), triangles: new Uint32Array() })

export function decodeManifest(bytes: Uint8Array): WorldData {
  const [version, origin, halfExtent, tileSize, spawns, tiles, groundCell] = decode(bytes) as [number, [number, number], number, number, number[][], number[][], number]
  if (version !== WORLD_VERSION) throw new Error(`world version ${version}, expected ${WORLD_VERSION}`)
  const refs = tiles.map(([col, row, hash]) => ({ col: col!, row: row!, hash: hash! }))
  const grid = Math.max(1, Math.ceil((2 * halfExtent) / Math.max(tileSize, 1)))
  let inside: boolean[] | null = null
  if (refs.length > 0 && refs.length < grid * grid) {
    inside = new Array<boolean>(grid * grid).fill(false)
    for (const { col, row } of refs) inside[row * grid + col] = true
  }
  return {
    origin,
    halfExtent: metres(halfExtent),
    tileSize: metres(tileSize),
    spawns: spawns.map(point),
    tiles: refs,
    groundCell: metres(groundCell),
    inside,
    collision: noCollision(),
  }
}

export function decodeTile(bytes: Uint8Array): TileData {
  const [buildings, roads, [vertices, triangles], ground, cover, trees, water, streams, bridges, detail, rails] = decode(bytes) as [
    [number, number[][], number, number, number, number, number][],
    [number, number, number[][]][],
    [number[][], number[][]],
    number[],
    [number, number[][][]][],
    [number, number, number, number, number, number, boolean][],
    number[],
    [number, number[][]][],
    [number, number, number][] | undefined,
    number[] | undefined,
    [number, number, number[][]][] | undefined,
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
      trunkRadius: metres(trunkRadiusCm(kind, height, crown)),
      trunkHeight: metres(Math.trunc((height * treeKindDef(kind).colliderHeightPct) / 100)),
    })),
    collision: {
      vertices: Float32Array.from(vertices.flat(), metres),
      triangles: Uint32Array.from(triangles.flat()),
    },
    ground: Float32Array.from(ground, metres),
    water: Float32Array.from(water, (cm) => (cm === NO_WATER_CM ? NO_WATER : metres(cm))),
    streams: streams.map(([width, points]) => ({ width: metres(width), points: points.map(point) })),
    bridges: (bridges ?? []).map(([first, deck, steel]) => ({ first, deck, steel })),
    detail: detail && detail.length > 0 ? Int8Array.from(detail) : undefined,
    rails: (rails ?? []).map(([gauge, kind, points]) => ({ gauge: metres(gauge), kind, points: points.map(point) })),
  }
}

/** Loads the manifest. `hash` is what the server reports in `welcome` if the worlds match. */
export async function loadManifest(url?: string): Promise<{ world: WorldData; hash: number }> {
  if (url === undefined) {
    await resolveWorldBase()
    url = `${WORLD_BASE}world.msgpack`
  }
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${url}: ${response.status}; run \`cargo run -p osm-import\``)
  const bytes = new Uint8Array(await response.arrayBuffer())
  return { world: decodeManifest(bytes), hash: worldHash(bytes) }
}

/** Fetches one tile and checks it is the one the manifest names. */
export async function fetchTile(ref: TileRef, base = WORLD_BASE): Promise<TileData> {
  const url = `${base}tiles/${ref.col}_${ref.row}.msgpack`
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${url}: ${response.status}`)
  const bytes = new Uint8Array(await response.arrayBuffer())
  if (worldHash(bytes) !== ref.hash) throw new Error(`${url} does not match the manifest; reload`)
  return decodeTile(bytes)
}
