import { decode } from '@msgpack/msgpack'
import tables from './sim/tables.generated.json'
import { span } from './spans'

/** Mirrors `crates/shared/src/world.rs`. Coordinates are metres: +X east, +Z south, +Y up. */

export const WORLD_VERSION = 45

/** `Manifest::theme`, mirrors `crates/shared/src/world.rs`. */
export const THEME_WINTER = 1

/** Where `world.msgpack`, `tiles/` and `minimap.webp` are served from: `/` normally, `/dev/` for the dev world. */
export let WORLD_BASE: string = withSlash(import.meta.env.VITE_WORLD_BASE || '/')

/** Whether a world's water is ice: a winter world whose water was not left open (`Manifest::frozen`). */
export function frozen(world: Pick<WorldData, 'theme' | 'openWater'>): boolean {
  return world.theme === THEME_WINTER && !world.openWater
}

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
export const ROAD_TRACK = 3
export const ROAD_FAINT = 4
/** A footway or cycleway with asphalt on it: drawn and ridden as asphalt, walked as a path. */
export const ROAD_PAVED_PATH = 5
/** A track on its own ballast bed, with sleepers. */
export const RAIL_TRAIN = 0
/** A track set flush into the street. */
export const RAIL_TRAM = 1
/** How much wider a track's ballast bed is than its gauge, metres (`BALLAST_EXTRA` in the importer). */
export const BALLAST_EXTRA = 2.2

/** `Line::kind`: a wire fence on wooden posts, waist high. */
export const LINE_FENCE = 0
/** A game fence: wire netting two metres high on tall posts. */
export const LINE_FENCE_HIGH = 1
/** A power line on wooden poles, with three wires on a crossarm. */
export const LINE_POWER = 2
/** A major power line on steel pylons. */
export const LINE_POWER_MAJOR = 3
/** A low stone wall, as round a garden or a field. */
export const LINE_WALL = 4
/** A high stone wall, over head height. */
export const LINE_WALL_HIGH = 5
/** A steel guard rail on posts down the middle of a divided road (mitträcke). */
export const LINE_GUARDRAIL = 6

/**
 * What a kind of fence or wall is: `LINE_KINDS` in `crates/shared/src/world.rs`, through
 * `sim/tables.generated.json`. A power line is not in it.
 */
export interface LineKindDef {
  kind: number
  /** Height of its top over the ground, metres. */
  height: number
  /** Half its thickness across the line, metres. */
  half: number
  /** The longest a piece is, metres. */
  step: number
  /** Whether it is a wall, solid to everything, and not a fence. */
  wall: boolean
}

export const LINE_KINDS: readonly LineKindDef[] = tables.lineKinds

/** The definition of a fence or wall kind; undefined for a power line. */
export const lineKindDef = (kind: number): LineKindDef | undefined => LINE_KINDS.find((def) => def.kind === kind)

/**
 * The index of a piece of a fence or wall in its tile, as the server names it (`line_piece_index` in
 * `crates/shared/src/world.rs`): line `line` of the tile's lines, the piece after its point `segment`.
 */
export const linePieceIndex = (line: number, segment: number): number => ((line << 16) | (segment & 0xffff)) >>> 0

export const COVER_WATER = 0
export const COVER_WOOD = 1
export const COVER_PARK = 2
export const COVER_FARM = 3
export const COVER_SAND = 4
export const COVER_URBAN = 5
export const COVER_INDUSTRIAL = 6
export const COVER_THICKET_SLOW = 7
export const COVER_THICKET_WALK = 8
export const COVER_THICKET_FIGHT = 9
export const COVER_MARSH = 10
export const COVER_ROCK = 11
export const COVER_MARSH_DEEP = 12
export const COVER_MARSH_FAINT = 13
/** A golf course's mown surfaces. */
export const COVER_GOLF_FAIRWAY = 14
export const COVER_GOLF_GREEN = 15
export const COVER_GOLF_TEE = 16
export const COVER_GOLF_BUNKER = 17
/** The gardens of a residential area: mown lawn, with gravel round the houses. */
export const COVER_GARDEN = 18
/** Asphalt laid as an area, not as a road: a car park. */
export const COVER_ASPHALT = 19

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
/** A golf hole's flagstick and its cup. */
export const TREE_FLAG = 10

/**
 * How an archetype's collider is sized and whether it can fall: `TREE_KINDS` in
 * `crates/shared/src/world.rs`, through `sim/tables.generated.json`.
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

/** In the server's order, the birch first. */
export const TREE_KINDS: readonly TreeKindDef[] = tables.treeKinds

/** The definition of an archetype. Unknown kinds are birches, as they are drawn. */
export const treeKindDef = (kind: number): TreeKindDef => TREE_KINDS.find((def) => def.kind === kind) ?? TREE_KINDS[0]!

export const GRAVE_HEADSTONE = 0
export const GRAVE_CROSS = 1
export const GRAVE_LOW = 2

/**
 * The size of a kind of tombstone, metres: `GRAVE_KINDS` in `crates/shared/src/world.rs`, through
 * `sim/tables.generated.json`. Its collider is this box, and it is drawn to fill it.
 */
export interface GraveKindDef {
  kind: number
  name: string
  /** Half its width across its face. */
  halfWidth: number
  /** Half its thickness from face to back. */
  halfDepth: number
  /** Height of its top over the ground. */
  height: number
}

/** In the server's order, the headstone first. */
export const GRAVE_KINDS: readonly GraveKindDef[] = tables.graveKinds

/** The definition of a kind of tombstone. Unknown kinds are headstones, as the server collides them. */
export const graveKindDef = (kind: number): GraveKindDef => GRAVE_KINDS.find((def) => def.kind === kind) ?? GRAVE_KINDS[0]!

/** One tombstone of a graveyard (`Grave` in `crates/shared/src/world.rs`). Metres and radians. */
export interface Grave {
  x: number
  z: number
  /** Height of the ground at its foot. */
  base: number
  /** The way its face looks, about the vertical: along +Z at 0, turning towards +X as it grows. */
  yaw: number
  /** `GRAVE_*`. */
  kind: number
}

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
  /**
   * This building's triangles in the tile's collision mesh: walls, then roof (then the floor, not
   * drawn). A building with `cells` has no wall or roof triangles, only its floor: it is its cubes.
   */
  first: number
  walls: number
  roof: number
  /** `ROOF_*`: what the laser data fitted the roof as. */
  roofKind: number
  /** `BUILDING_*`: what the building is, where that changes its windows, its inside and its colours. */
  kind: number
  /**
   * Which way the road it stands by lies from its middle, `[x, z]` of a unit vector times 127: the
   * wall facing that way is its front, with its door. `[0, 0]` where no road is near.
   */
  front: [number, number]
  /** Its 1 m cubes as the importer baked them, or null for a building with none (it keeps its triangles). */
  cells: Cells | null
  /** `[wall, roof]` as `0xRRGGBB`, each where the importer knows it (the map's `building:colour` and `roof:colour`, or in France the roof from the aerial photo), else null. */
  paint: [number | null, number | null]
  /** What its walls are clad in (`CLAD_*`) and its roof is laid with (`ROOFING_*`) where the map says, 0 for each it does not. */
  clad: [number, number]
  /**
   * Its own walls and roof in the tile's `look`, to be drawn: `walls` triangles from `first`, then
   * `roof`. Null when it has none (a world made before looks): it is then drawn as its cubes.
   */
  look?: { first: number; walls: number; roof: number; kind?: number; front?: readonly [number, number]; footprint?: Int32Array; inside?: Int32Array } | null
}

/**
 * A building's cubes as the importer baked them (`world::Cells`): the grid, centimetres, and one flag
 * a cell, packed (`unpack` in `sim/carve.ts`), for whether there is a cube there and whether it is roof.
 */
export interface Cells {
  origin: [number, number, number]
  dims: [number, number, number]
  solid: Uint8Array
  roof: Uint8Array
}

/**
 * One bridge's steel in the tile's collision mesh: `steel` triangles (pylon, stay cables) from
 * `first`. Its concrete is in sections that are buildings (`BUILDING_BRIDGE`).
 */
export interface Bridge {
  first: number
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

/**
 * `Building.kind`, mirrors `BUILDING_*` in `crates/shared/src/world.rs`: a building like any other, a
 * church, a barn, an outbuilding (a garage, a shed), a hall (a factory, a warehouse), a shop, a block
 * of flats, a public building (a school, a town hall), a cabin, an old wooden house, a post-war villa
 * and a fire station.
 */
export const BUILDING_PLAIN = 0
export const BUILDING_CHURCH = 1
export const BUILDING_BARN = 2
export const BUILDING_OUTBUILDING = 3
export const BUILDING_HALL = 4
export const BUILDING_SHOP = 5
export const BUILDING_FLATS = 6
export const BUILDING_PUBLIC = 7
export const BUILDING_CABIN = 8
export const BUILDING_FARMHOUSE = 9
export const BUILDING_VILLA = 10
export const BUILDING_FIRE_STATION = 11
/** A section of a bridge: drawn from its own concrete, carved by blasts, no windows and no inside. Its roof is the roadway. */
export const BUILDING_BRIDGE = 12
/** A lean-to (vindskydd): three walls, its `front` side open, a roof that falls to the back, no door or windows. */
export const BUILDING_SHELTER = 13
/** `Building.clad[0]`, what the walls are of: not known, boards or timber, brick, render or concrete, cut stone, profiled sheet metal. Mirrors `CLAD_*`. */
export const CLAD_UNKNOWN = 0
export const CLAD_WOOD = 1
export const CLAD_BRICK = 2
export const CLAD_RENDER = 3
export const CLAD_STONE = 4
export const CLAD_SHEET = 5
/** `Building.clad[1]`, what the roof is laid with: not known, tiles or slates, sheet metal, felt, wooden shingles. Mirrors `ROOFING_*`. */
export const ROOFING_UNKNOWN = 0
export const ROOFING_TILES = 1
export const ROOFING_SHEET = 2
export const ROOFING_FELT = 3
export const ROOFING_SHINGLES = 4
/** The colour of a bridge's concrete, `0xRRGGBB`: its sides, piers and railings. Mirrors `BRIDGE_CONCRETE`. */
export const BRIDGE_CONCRETE = 0xb8b6ae
/** The colour of a bridge's deck under a paved road or path, `0xRRGGBB`: drawn as asphalt. Mirrors `DECK_ASPHALT`. */
export const DECK_ASPHALT = 0x6e6e70
/** The colour of a bridge's deck under a footpath or a track, `0xRRGGBB`: drawn as planks. Mirrors `DECK_PLANKS`. */
export const DECK_PLANKS = 0x8a7356
/** The colour of a bridge's deck under a railway, `0xRRGGBB`: drawn as ballast. Mirrors `DECK_BALLAST`. */
export const DECK_BALLAST = 0x8a8478

export interface Road {
  width: number
  kind: number
  /** Whether a bridge's deck carries this stretch: driven and mapped like any road, not drawn on the ground under it. */
  carried: boolean
  points: Point2[]
  /** How many lanes it has, both ways together; 0 for a path or a track. */
  lanes: number
  /** How many of its `lanes` run the way its `points` do. */
  forward: number
  /** Which lines are painted on it, `MARK_*` flags. */
  marks: number
  /** How far along the way it was cut from its first point is, metres: a dashed line's rhythm carries on from it. */
  along: number
  /** Whether it is driven one way only: along its points when all its lanes are `forward`, against them when none is. */
  oneWay: boolean
}

/** White dashed lines along both edges (kantlinje): 1 m of paint, 2 m gap. Mirrors `MARK_EDGE`. */
export const MARK_EDGE = 1
/** Solid white lines along both edges, as on a motorway. Mirrors `MARK_EDGE_SOLID`. */
export const MARK_EDGE_SOLID = 2
/** Dashed lines between lanes that run the same way. Mirrors `MARK_LANES`. */
export const MARK_LANES = 4
/** A dashed line between the two directions. Mirrors `MARK_CENTRE`. */
export const MARK_CENTRE = 8
/** A solid line between the two directions (spärrlinje). Mirrors `MARK_CENTRE_SOLID`. */
export const MARK_CENTRE_SOLID = 16
/** No edge line on the left of the way its points run: a guard rail stands beside it. Mirrors `MARK_NO_LEFT_EDGE`. */
export const MARK_NO_LEFT_EDGE = 32
/** The edge line on the left is solid: a slow divided road paved up to its other carriageway. Mirrors `MARK_LEFT_SOLID`. */
export const MARK_LEFT_SOLID = 64

/** A railway track's centre line. Drawn, not simulated. */
export interface Rail {
  /** Distance between the rails, metres. */
  gauge: number
  kind: number
  points: Point2[]
  /** The deck that carries it, or null where it lies on the ground. */
  deck: RailDeck | null
}

/** What a stretch of track on a bridge lies on. Mirrors `RailDeck`. */
export interface RailDeck {
  /** The bridge's section it lies on, by its place in the same tile's `buildings`. */
  building: number
  /** Height of the deck's top under each of the track's points, metres. */
  tops: number[]
}

/**
 * A fence, a wall or a power line (`Line` in `crates/shared/src/world.rs`). A fence or wall is cut at
 * the tile's edges and has a point at every post: each piece between two points is solid and breaks on
 * its own (`linePieceIndex`). A power line is its poles, span after span, and its last point is a pole
 * of the next tile unless `end`.
 */
export interface Line {
  kind: number
  points: Point2[]
  /** Height of the ground under each point, metres. */
  base: number[]
  end: boolean
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
/** `TileData.stones`: the share of a cell under stones, out of this (`STONES_DENSITY` in `crates/shared`). */
export const STONES_DENSITY = 0x7f
/** `TileData.stones`: a boulder field's blocks rather than cobbles (`STONES_ANGULAR`). */
export const STONES_ANGULAR = 0x80
/** `TileData.crops`: nothing grown in the cell, and wheat (`CROP_NONE`, `CROP_WHEAT` in `crates/shared`). */
export const CROP_NONE = 0
export const CROP_WHEAT = 1

/** `Tile::water` entry (centimetres) for a ground sample with no water. */
const NO_WATER_CM = -2147483648

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
/** A groomed ski track's centre line, clipped to the tile. */
export interface SkiTrail {
  /** Groomed width, metres. */
  width: number
  points: Point2[]
}

/**
 * A vehicle dealer's carport (`Dealer` in `crates/shared/src/world.rs`, DEALERS.md), in metres and
 * radians: the middle of its open mouth, how far its frame is turned (its +Z, out of the mouth onto the
 * store's car park, goes to `(sin, cos)`), and the height of its floor (`shared::dealer`).
 */
export interface Dealer {
  x: number
  z: number
  yaw: number
  base: number
}

/**
 * A fuel station (`FuelStation` in `crates/shared/src/world.rs`), in metres and radians: the middle of
 * its lot, how far its frame is turned (its +Z, back from the road, goes to `(sin, cos)`), and the
 * height of its paving (`shared::station`).
 */
export interface FuelStation {
  x: number
  z: number
  yaw: number
  base: number
}

/**
 * A ski lift (`Lift` in `crates/shared/src/world.rs`), in metres: its line from the bottom station
 * over the pylons to the top, the ground under each point and the cable's height over each, from the
 * world's zero. Every lift is drawn as a chair lift; `kind` is what OpenStreetMap calls it (`LIFT_*`).
 */
export interface Lift {
  kind: number
  seats: number
  points: Point2[]
  base: number[]
  cable: number[]
}

/**
 * A bus line (`BusLine` in `crates/shared/src/world.rs`), metres: the middle of the road it follows
 * from one end to the other, how far along that each of its stops is, and where each stop's one
 * sign stands.
 */
export interface BusLine {
  points: Point2[]
  stops: number[]
  signs: Point2[]
}

/** The point `along` metres from the start of `line` (held to its ends) and the unit direction the line runs there. */
export function busLineAt(line: BusLine, along: number): { at: Point2; way: Point2 } {
  let left = Math.max(along, 0)
  let last: { at: Point2; way: Point2 } = { at: line.points[0] ?? [0, 0], way: [0, -1] }
  for (let i = 1; i < line.points.length; i++) {
    const [a, b] = [line.points[i - 1]!, line.points[i]!]
    const length = Math.hypot(b[0] - a[0], b[1] - a[1])
    if (length < 1e-3) continue
    const way: Point2 = [(b[0] - a[0]) / length, (b[1] - a[1]) / length]
    if (left <= length) return { at: [a[0] + way[0] * left, a[1] + way[1] * left], way }
    left -= length
    last = { at: b, way }
  }
  return last
}

export interface WorldData {
  /**
   * How local metres lie on the Earth (`Frame` in `crates/shared/src/world.rs`): `[0, lat, lon]` for
   * Sweden's equirectangular frame around `lat, lon`, `[1, e0, n0]` for Lambert-93 from `e0, n0`.
   */
  frame: [number, number, number]
  halfExtent: number
  /** Side of a tile, metres. Tile `(col, row)` starts at `-halfExtent + col * tileSize`. */
  tileSize: number
  spawns: Point2[]
  tiles: TileRef[]
  /** Distance between ground samples, metres; 0 when the ground is the plane y = 0. */
  groundCell: number
  /** `THEME_*`: 0 is the world as it always was. */
  theme: number
  /** The ski lifts, whole (`Manifest::lifts`): drawn wherever the player is. */
  lifts: Lift[]
  /** Winter with the lakes and the sea left open (`Manifest::open_water`), so boats still sail. */
  openWater?: boolean
  /** The bus lines, whole (`Manifest::bus_lines`): a sign stands at every stop. None on a world with no public transport. */
  busLines: BusLine[]
  /** The colour of the rivers' deep water, sRGB hex (`Manifest::river_tint`). */
  riverTint?: number
  /** Whether the lakes and the sea are that colour too (`Manifest::tint_all`). */
  tintAll?: boolean
  /** The fuel stations (`Manifest::fuel_stations`): a vehicle near one has its tank filled, and one through its wash is healed (FUEL.md). */
  fuelStations?: FuelStation[]
  /** The vehicle dealers (`Manifest::dealers`): a soldier by one buys a vehicle on the phone (DEALERS.md). */
  dealers?: Dealer[]
  /**
   * Which tiles of the square grid are part of the world, `grid × grid` row-major, or `null` when
   * every one is. Tiles outside it have no ground and are walled off.
   */
  inside?: boolean[] | null
  /** Collision that is not part of any tile. Empty in a generated world; tests fill it in. */
  collision: CollisionMesh
  /** The `world.msgpack` this was decoded from, which the soldier's physics reads for itself. */
  bytes?: Uint8Array
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
   * How wide the open water is around each ground sample (same layout as `water`), in steps of
   * `REACH_STEP` metres: the waves grow with it. Absent when the tile has no water, or in a tile made up in a test.
   */
  reach?: Uint8Array
  /**
   * Which way and how fast a river runs at each ground sample: `x` then `z`, two entries a sample in the
   * layout of `water`, in steps of `FLOW_STEP` m/s. Absent when no water in the tile flows.
   */
  flow?: Int8Array
  /**
   * How much of a river's surface is under foam at each ground sample (same layout as `water`), in
   * 255ths: what its rapids make, carried downstream. Absent when the tile has none.
   */
  foam?: Uint8Array
  /**
   * How a river's current stirs its surface at each ground sample: two entries a sample in the layout of
   * `water`, in steps of `STIR_STEP` 1/s. First how it wells up in boils (above 0) or gathers into a foam
   * line (below 0), then how fast it spins, along an eddy line or behind a boulder. Absent when no water
   * in the tile flows.
   */
  stir?: Int8Array
  /**
   * A river's stream function at each ground sample (same layout as `water`), in `1 / LANE_STEPS` of
   * `LANE_FLUX` m³/s and wrapping round every 64 of them: the water runs along a line where it is the
   * same, so its streaks are drawn along those lines. Absent when no water in the tile flows.
   */
  lane?: Int16Array
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
  ski: SkiTrail[]
  /** Fences and power lines; absent in a tile made up in a test. */
  lines?: Line[]
  /** The tombstones of the tile's graveyards; absent in a tile made up in a test. */
  graves?: Grave[]
  /**
   * Stony ground, `n × n` cells over the tile with `n = sqrt(length)`, rows along +Z: how stony each
   * is in the low bits (`STONES_DENSITY`, 0 none) and `STONES_ANGULAR` for a boulder field's blocks.
   * Absent when the tile has none.
   */
  stones?: Uint8Array
  /**
   * Standing crops, a grid laid out as `stones` is: what grows in each cell (`CROP_*`). Absent when
   * the tile has none.
   */
  crops?: Uint8Array
  /**
   * The buildings' own walls and roofs, for drawing only (`Building.look` says whose are where),
   * wound as `collision` is. Absent when the world has none.
   */
  look?: CollisionMesh
  /**
   * The tile file this was decoded from, which the soldier's physics reads for itself when the tile
   * is attached. Gone after that (`WorldView.attachTile`): nothing else here is a view into it, so
   * the file is then held nowhere.
   */
  bytes?: Uint8Array
}

/** FNV-1a, matching `world_hash` on the server. */
export function worldHash(bytes: Uint8Array): number {
  let hash = 0x811c9dc5
  // Indexed, not `for…of`: the iterator was most of the cost on a 1 m tile.
  for (let i = 0; i < bytes.length; i++) hash = Math.imul(hash ^ bytes[i]!, 0x01000193)
  return hash >>> 0
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
  const [version, frame, halfExtent, tileSize, spawns, tiles, groundCell, theme, lifts, openWater, busLines, riverTint, tintAll, fuelStations, dealers] = decode(bytes) as [number, [number, number, number], number, number, number[][], number[][], number, number, [number, number, number[][], number[], number[]][], boolean, [number[][], number[], number[][]][], [number, number, number], boolean, [[number, number], number, number][], [[number, number], number, number][]]
  if (version !== WORLD_VERSION) throw new Error(`world version ${version}, expected ${WORLD_VERSION}`)
  const refs = tiles.map(([col, row, hash]) => ({ col: col!, row: row!, hash: hash! }))
  const grid = Math.max(1, Math.ceil((2 * halfExtent) / Math.max(tileSize, 1)))
  let inside: boolean[] | null = null
  if (refs.length > 0 && refs.length < grid * grid) {
    inside = new Array<boolean>(grid * grid).fill(false)
    for (const { col, row } of refs) inside[row * grid + col] = true
  }
  return {
    frame,
    halfExtent: metres(halfExtent),
    tileSize: metres(tileSize),
    spawns: spawns.map(point),
    tiles: refs,
    groundCell: metres(groundCell),
    theme,
    lifts: lifts.map(([kind, seats, points, base, cable]) => ({ kind, seats, points: points.map(point), base: base.map(metres), cable: cable.map(metres) })),
    openWater,
    busLines: busLines.map(([points, stops, signs]) => ({ points: points.map(point), stops: stops.map(metres), signs: signs.map(point) })),
    riverTint: (riverTint[0] << 16) | (riverTint[1] << 8) | riverTint[2],
    tintAll,
    fuelStations: fuelStations.map(([[x, z], yaw, base]) => ({ x: metres(x), z: metres(z), yaw: yaw / 1000, base: metres(base) })),
    dealers: dealers.map(([[x, z], yaw, base]) => ({ x: metres(x), z: metres(z), yaw: yaw / 1000, base: metres(base) })),
    inside,
    collision: noCollision(),
    bytes,
  }
}

// Plain loops, not `Float32Array.from(array, map)`: its generic path was most of `decodeTile` on a
// 1 m tile, whose ground alone is 66,049 numbers.

/**
 * A msgpack `bin` of little-endian `i32`s (`samples32` in `world.rs`) as an `Int32Array`: a view of
 * the file's bytes, copied once only when the view would not start on a multiple of 4. Every
 * platform a browser runs on is little-endian.
 */
function int32s(bin: Uint8Array): Int32Array {
  const bytes = bin.byteOffset % 4 === 0 ? bin : bin.slice()
  return new Int32Array(bytes.buffer, bytes.byteOffset, bytes.length >> 2)
}

/**
 * A tile's ground heights, metres, as the file writes them (`heights` in `world.rs`): `[step, residuals]`,
 * each sample in steps as its difference from the plane through its left, above and above-left
 * neighbours, zigzagged and split into a plane of low bytes and one of high bytes.
 */
function groundMetres([step, planes]: [number, Uint8Array]): Float32Array {
  const n = planes.length >> 1
  const side = Math.round(Math.sqrt(n))
  if (side * side !== n || planes.length !== 2 * n) throw new Error(`ground residuals of ${planes.length} bytes are not a square grid`)
  const steps = new Int32Array(n)
  const out = new Float32Array(n)
  for (let i = 0; i < n; i++) {
    const x = i % side
    const predicted = i < side ? (x > 0 ? steps[i - 1]! : 0) : x > 0 ? steps[i - 1]! + steps[i - side]! - steps[i - side - 1]! : steps[i - side]!
    const zigzag = planes[i]! | (planes[n + i]! << 8)
    steps[i] = predicted + ((zigzag >>> 1) ^ -(zigzag & 1))
    out[i] = (steps[i]! * step) / 100
  }
  return out
}

/** Centimetres to metres, as `metres` would map them. With `water`, `NO_WATER_CM` becomes `NO_WATER`. */
function inMetres(cm: ArrayLike<number>, water = false): Float32Array {
  const out = new Float32Array(cm.length)
  for (let i = 0; i < cm.length; i++) out[i] = water && cm[i] === NO_WATER_CM ? NO_WATER : cm[i]! / 100
  return out
}

/** Vertices `[x, y, z]` in centimetres, flattened, in metres. */
function flatMetres(points: readonly number[][]): Float32Array {
  const out = new Float32Array(points.length * 3)
  for (let i = 0; i < points.length; i++) {
    const p = points[i]!
    out[i * 3] = p[0]! / 100
    out[i * 3 + 1] = p[1]! / 100
    out[i * 3 + 2] = p[2]! / 100
  }
  return out
}

/** Triangles `[a, b, c]`, flattened. */
function flatIndices(triangles: readonly number[][]): Uint32Array {
  const out = new Uint32Array(triangles.length * 3)
  for (let i = 0; i < triangles.length; i++) {
    const t = triangles[i]!
    out[i * 3] = t[0]!
    out[i * 3 + 1] = t[1]!
    out[i * 3 + 2] = t[2]!
  }
  return out
}

export function decodeTile(bytes: Uint8Array): TileData {
  const [buildings, roads, [vertices, triangles], ground, cover, trees, water, reach, flow, foam, stir, lane, streams, bridges, detail, rails, ski, lines, stones, crops, look, graves] = decode(bytes) as [
    [number, number[][], number, number, number, number, number, number, [number, number], [number[], number[], Uint8Array, Uint8Array], [number | null, number | null], [number, number]][],
    [number, number, boolean, number[][], number, number, number, number, boolean][],
    [number[][], number[][]],
    [number, Uint8Array],
    [number, number[][][]][],
    [number, number, number, number, number, number, boolean][],
    Uint8Array,
    Uint8Array,
    Uint8Array,
    Uint8Array,
    Uint8Array,
    Uint8Array,
    [number, number[][]][],
    [number, number, number][],
    Uint8Array,
    [number, number, number[][], [number, number[]] | null][],
    [number, number[][]][],
    [number, number[][], number[], boolean][],
    Uint8Array,
    Uint8Array,
    [number[][], number[][], number[][]],
    [number, number, number, number, number][],
  ]
  const looks = look[2]
  return {
    buildings: buildings.map(([height, footprint, base, first, walls, roof, roofKind, kind, front, cells, paint, clad], index) => ({
      height: metres(height),
      footprint: footprint.map(point),
      base: metres(base),
      first,
      walls,
      roof,
      roofKind,
      kind,
      front,
      cells:
        cells[1][0]! * cells[1][1]! * cells[1][2]! > 0
          ? // Copies of their own, as `detail` is: views would keep the whole file alive.
            { origin: cells[0] as [number, number, number], dims: cells[1] as [number, number, number], solid: cells[2].slice(), roof: cells[3].slice() }
          : null,
      paint,
      clad,
      look: looks[index] && looks[index]![1]! + looks[index]![2]! > 0 ? { first: looks[index]![0]!, walls: looks[index]![1]!, roof: looks[index]![2]!, kind, front, footprint: Int32Array.from(footprint.flat()) } : null,
    })),
    roads: roads.map(([width, kind, carried, points, lanes, forward, marks, along, oneWay]) => ({ width: metres(width), kind, carried, points: points.map(point), lanes, forward, marks, along: metres(along), oneWay })),
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
    collision: { vertices: flatMetres(vertices), triangles: flatIndices(triangles) },
    ground: groundMetres(ground),
    water: inMetres(int32s(water), true),
    ...(reach.length > 0 ? { reach: reach.slice() } : {}),
    // A copy of its own, as `detail` is.
    ...(flow.length > 0 ? { flow: new Int8Array(flow.slice().buffer) } : {}),
    ...(foam.length > 0 ? { foam: foam.slice() } : {}),
    ...(stir.length > 0 ? { stir: new Int8Array(stir.slice().buffer) } : {}),
    // Little-endian `i16`s, which every machine the game runs on is.
    ...(lane.length > 0 ? { lane: new Int16Array(lane.slice().buffer) } : {}),
    streams: streams.map(([width, points]) => ({ width: metres(width), points: points.map(point) })),
    bridges: bridges.map(([first, steel]) => ({ first, steel })),
    // A copy of its own: a view would keep the whole file alive, and be cloned whole to the worker.
    detail: detail.length > 0 ? new Int8Array(detail.slice().buffer) : undefined,
    rails: rails.map(([gauge, kind, points, deck]) => ({ gauge: metres(gauge), kind, points: points.map(point), deck: deck ? { building: deck[0], tops: deck[1].map(metres) } : null })),
    ski: ski.map(([width, points]) => ({ width: metres(width), points: points.map(point) })),
    lines: lines.map(([kind, points, base, end]) => ({ kind, points: points.map(point), base: base.map(metres), end })),
    graves: graves.map(([x, z, base, yaw, kind]) => ({ x: metres(x), z: metres(z), base: metres(base), yaw: yaw / 1000, kind })),
    ...(stones.length > 0 ? { stones: stones.slice() } : {}),
    ...(crops.length > 0 ? { crops: crops.slice() } : {}),
    ...(look[1].length > 0 ? { look: { vertices: flatMetres(look[0]), triangles: flatIndices(look[1]) } } : {}),
    bytes,
  }
}

/** Loads the manifest. `hash` is what the server reports in `welcome` if the worlds match. */
export async function loadManifest(url?: string): Promise<{ world: WorldData; hash: number }> {
  if (url === undefined) {
    await resolveWorldBase()
    url = `${WORLD_BASE}world.msgpack`
  }
  // Always asked for (a 304 when unchanged): a copy kept from before a deploy is another world than the server's.
  const response = await fetch(url, { cache: 'no-cache' })
  if (!response.ok) throw new Error(`${url}: ${response.status}; run \`cargo run -p osm-import\``)
  const bytes = new Uint8Array(await response.arrayBuffer())
  return { world: decodeManifest(bytes), hash: worldHash(bytes) }
}

/** Fetches one tile and checks it is the one the manifest names. */
export async function fetchTile(ref: TileRef, base = WORLD_BASE): Promise<TileData> {
  const url = `${base}tiles/${ref.col}_${ref.row}.msgpack`
  // The hash makes the address name the content, so the service worker can keep the tile for good.
  const response = await fetch(`${url}?h=${(ref.hash >>> 0).toString(16)}`)
  if (!response.ok) throw new Error(`${url}: ${response.status}`)
  const bytes = new Uint8Array(await response.arrayBuffer())
  if (span('tile.hash', () => worldHash(bytes)) !== ref.hash) throw new Error(`${url} does not match the manifest; reload`)
  return span('tile.decode', () => decodeTile(bytes))
}
