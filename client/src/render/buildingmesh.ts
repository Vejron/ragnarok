import { Color } from '@ragnarok/engine'
import type { Sphere } from '@ragnarok/engine'
import { BUILDING_BARN, BUILDING_BRIDGE, BUILDING_CABIN, BUILDING_CHURCH, BUILDING_FARMHOUSE, BUILDING_FIRE_STATION, BUILDING_FLATS, BUILDING_HALL, BUILDING_PLAIN, BUILDING_VILLA, BUILDING_OUTBUILDING, BUILDING_PUBLIC, BUILDING_SHOP, DECK_ASPHALT, DECK_BALLAST, DECK_PLANKS, ROOF_FLAT, type Bridge, type Building, type TileData } from '@/world'
import { winter } from './season'
import { shellOf } from '@/sim/carve'
import { drawnOpenings, facing, openingQuads, openingTriangles, openingsFor, type Opening } from './windows'
import { eaveQuads, eaveTriangles, eavesFor, type EaveStrip } from './eaves'
import { CHIMNEY_RGB, DETAIL_TRIM_RGB, detailTriangles, detailsOf, type DetailQuad } from './details'
import { glassBuffers, type GlassBuffers } from './glass'
import { FLOOR_RGB, INSIDE_RGB, INTERIOR_SHADE, facadeFor, innerOf, type Facade } from './facade'
import { normalOf, type Poly } from './geom'

/*
 * A tile's buildings and bridges as one flat-shaded triangle soup with a colour a vertex, as plain
 * arrays, as part of preparing a tile (`tileprep.ts`): in the tile worker or on the main thread, which
 * is why nothing here touches the scene.
 */

/**
 * House colours as they are painted here: falu red, ochre, white, render greys, sage, beige and dark
 * stained wood. Picked per building so the palette stays stable between reloads.
 */
const BRIDGE_STEEL_COLOR = 0x6d7a86
const BUILDING_COLORS = [0xa8432f, 0xd9b45a, 0xeee9dc, 0xc9c4b8, 0xb5bda4, 0xd8c6a3, 0x6e5642, 0x8f3a2c, 0xe4dcc6, 0xa9a59c]

/** Roofs: slate, tile and timber for pitched ones, concrete grey for flat ones. */
const ROOF_COLORS = [0xb5543f, 0x6d7a8c, 0x8d6e63, 0x4f5d6b, 0x9c4a3c]
const FLAT_ROOF_COLORS = [0xb0b4ba, 0x9aa0a6, 0xc4c7cc]
/** A church: whitewashed render under a copper roof gone dark green. */
const CHURCH_COLORS = { wall: 0xf4f0e6, roof: 0x4a5a55 }
/** Sheet metal on a pitched roof that is not a house's: dark, darker and weathered grey. */
const SHEET_ROOF_COLORS = [0x5a5f63, 0x3f4447, 0x7d8388]
/**
 * The walls of the kinds that are not houses (`BUILDING_*`), and whether their pitched roofs are
 * sheet metal: a barn falu red, now and then tarred dark or left to go grey; an outbuilding as the
 * wooden houses are; a hall profiled sheet in grey, blue, green, cream or red; a shop light render or
 * brick; a block of flats render or brick; a public building yellow or red brick or white render; a
 * cabin dark timber; an old house red, ochre or white boards; a villa brick, pale boards or render; a fire station red or yellow brick, or render.
 */
const KIND_COLORS: Readonly<Record<number, { walls: readonly number[]; sheet: boolean }>> = {
  [BUILDING_BARN]: { walls: [0xa8432f, 0xa8432f, 0xa8432f, 0x6e5642, 0x8c857a], sheet: true },
  [BUILDING_OUTBUILDING]: { walls: [0xa8432f, 0xa8432f, 0x6e5642, 0xd9b45a, 0xeee9dc], sheet: true },
  [BUILDING_HALL]: { walls: [0xa3abb0, 0x7f93a6, 0x8a9a86, 0xd6d2c8, 0x94564a], sheet: true },
  [BUILDING_SHOP]: { walls: [0xeee9dc, 0xd8d3c8, 0xe4dcc6, 0x8f3a2c], sheet: false },
  [BUILDING_FLATS]: { walls: [0xd8d3c8, 0xc9c4b8, 0xe9c9b6, 0x8f3a2c, 0xead0a0, 0xb5bda4], sheet: false },
  [BUILDING_PUBLIC]: { walls: [0xcfae6a, 0xcfae6a, 0x8f3a2c, 0xf4f0e6], sheet: false },
  [BUILDING_CABIN]: { walls: [0x6e5642, 0x6e5642, 0x4a3a2e, 0xa8432f], sheet: true },
  [BUILDING_FARMHOUSE]: { walls: [0xa8432f, 0xa8432f, 0xd9b45a, 0xeee9dc, 0xe4dcc6], sheet: false },
  [BUILDING_VILLA]: { walls: [0x8f3a2c, 0xcfae6a, 0xeee9dc, 0xc9c4b8, 0xd8c6a3, 0xa9a59c], sheet: false },
  [BUILDING_FIRE_STATION]: { walls: [0x8f3a2c, 0x8f3a2c, 0xcfae6a, 0xd8d3c8], sheet: false },
}

/** What of a tile its buildings are drawn from: small enough to send to the worker. */
export interface BuildingTile {
  buildings: readonly (Pick<Building, 'first' | 'walls' | 'roof' | 'roofKind' | 'cells' | 'paint' | 'look'> & { kind?: number; footprint?: Building['footprint'] })[]
  bridges: readonly Bridge[]
  collision: TileData['collision']
  look?: TileData['look']
}


/**
 * Flat-shaded triangles: corners in metres, a normal and a colour a vertex, and the sphere round it.
 * A vertex is shared by the triangles of one building that have the same corner, normal, colour and
 * pattern (`weld`), which is most of two fifths of them: the two triangles of a wall's quad, the faces
 * side by side on a flat wall.
 */
export interface BuildingBuffers {
  positions: Float32Array
  normals: Float32Array
  colors: Float32Array
  /** Four numbers a vertex: the first is the surface pattern drawn on it (`Surface`), the rest unused. */
  custom: Float32Array
  /** Three vertices a triangle. */
  indices: Uint32Array
  bounds: Sphere
  /**
   * For a tile's buildings (`buildingBuffers`), each building's vertices: first, then count, two
   * numbers a building, a count of 0 for one left out. No triangle of another building uses them, so
   * a building that falls later is taken out of the drawn mesh in place (`rebuildBuildings`).
   */
  ranges?: Uint32Array
  /** The glass in the windows of the buildings drawn from their looks, drawn apart and translucent (`glass.ts`); absent when there is none. */
  glass?: GlassBuffers
}

/** The arrays of `buffers`, so they move to the other thread instead of being copied. */
export function buildingTransferables(buffers: BuildingBuffers): ArrayBuffer[] {
  const glass = buffers.glass ? [buffers.glass.positions, buffers.glass.normals, buffers.glass.panes] : []
  const arrays = [buffers.positions, buffers.normals, buffers.colors, buffers.custom, buffers.indices, ...(buffers.ranges ? [buffers.ranges] : []), ...glass]
  return arrays.map((array) => array.buffer as ArrayBuffer)
}

/** An outbuilding is painted as the house within this of it, metres between their middles. */
const YARD = 40
/** The kinds an outbuilding belongs to: the houses. */
const HOUSES: ReadonlySet<number> = new Set([BUILDING_PLAIN, BUILDING_FARMHOUSE, BUILDING_VILLA, BUILDING_CABIN])
const yards = new WeakMap<object, Map<number, number | null>>()

/** The house of the tile that outbuilding `index` stands beside (the nearest within `YARD`), if any. */
function houseOf(tile: Pick<BuildingTile, 'buildings'>, index: number): number | null {
  let known = yards.get(tile.buildings)
  if (!known) yards.set(tile.buildings, (known = new Map()))
  const cached = known.get(index)
  if (cached !== undefined) return cached
  const middle = (footprint: Building['footprint']) => {
    let [x, z] = [0, 0]
    for (const p of footprint) {
      x += p[0] / footprint.length
      z += p[1] / footprint.length
    }
    return [x, z] as const
  }
  let best: number | null = null
  const own = tile.buildings[index]?.footprint
  if (own && own.length > 0) {
    const [x, z] = middle(own)
    let nearest = YARD
    tile.buildings.forEach((other, at) => {
      if (at === index || !other.footprint || other.footprint.length === 0 || !HOUSES.has(other.kind ?? 0)) return
      const [ox, oz] = middle(other.footprint)
      const distance = Math.hypot(ox - x, oz - z)
      if (distance < nearest) {
        nearest = distance
        best = at
      }
    })
  }
  known.set(index, best)
  return best
}

/**
 * The wall and roof colours of building `index`: the ones the importer gave it, or a church's, or
 * else picked from the palettes (its kind's, where that has its own), stable between reloads and
 * varied across tiles by `salt`. An outbuilding takes the colours of the wooden house in its yard.
 */
export function buildingColors(tile: Pick<BuildingTile, 'buildings'>, index: number, salt: number): { wall: number; roof: number } {
  const building = tile.buildings[index]!
  if (building?.paint) {
    const [wall, roof] = building.paint
    // A railway's deck is snowed on as its track's bed on the ground is.
    return { wall, roof: winter && roof === DECK_BALLAST && building.kind === BUILDING_BRIDGE ? WINTER_BALLAST_COLOR : roof }
  }
  if (building?.kind === BUILDING_CHURCH) return CHURCH_COLORS
  const kind = KIND_COLORS[building?.kind ?? 0]
  const palette = building?.roofKind === ROOF_FLAT ? FLAT_ROOF_COLORS : kind?.sheet ? SHEET_ROOF_COLORS : ROOF_COLORS
  const walls = kind?.walls ?? BUILDING_COLORS
  const own = { wall: walls[(index + salt) % walls.length]!, roof: palette[(index + salt) % palette.length]! }
  // An outbuilding beside a wooden house is painted as the house, and roofed as it where both roofs are pitched.
  const house = building?.kind === BUILDING_OUTBUILDING ? houseOf(tile, index) : null
  if (house === null) return own
  const colors = buildingColors(tile, house, salt)
  const boards = WALL_SURFACES.get(colors.wall)
  if (boards !== Surface.Upright && boards !== Surface.Lap) return own
  const pitched = building!.roofKind !== ROOF_FLAT && tile.buildings[house]!.roofKind !== ROOF_FLAT
  return { wall: colors.wall, roof: pitched ? colors.roof : own.roof }
}

/** Patterns drawn on a building's faces by the shader (walls.wgsl.ts in the engine), as a vertex's custom x. */
/** A railway's ballast under snow, as `rails.ts` draws it on the ground. */
const WINTER_BALLAST_COLOR = 0xdfe3e8

export const Surface = { Plain: 0, Upright: 1, Lap: 2, Brick: 3, Render: 4, Tiles: 5, Sheet: 6, Felt: 7, Ashlar: 8, Shingles: 9, Asphalt: 10, Planks: 11, Ballast: 12 } as const
export type Surface = (typeof Surface)[keyof typeof Surface]

/**
 * Falu red, dark stained and bare grey wood are upright boards, and so is a hall's profiled sheet,
 * ribbed the same way; ochre, white and beige lap boards; the dark red brick; greys and sage render.
 */
const WALL_SURFACES = new Map<number, Surface>([
  [0xa8432f, Surface.Upright], [0x6e5642, Surface.Upright], [0x8c857a, Surface.Upright], [0x4a3a2e, Surface.Upright],
  [0xa3abb0, Surface.Upright], [0x7f93a6, Surface.Upright], [0x8a9a86, Surface.Upright], [0xd6d2c8, Surface.Upright], [0x94564a, Surface.Upright],
  [0xd9b45a, Surface.Lap], [0xeee9dc, Surface.Lap], [0xe4dcc6, Surface.Lap], [0xd8c6a3, Surface.Lap],
  [0x8f3a2c, Surface.Brick], [0xcfae6a, Surface.Brick],
  [0xc9c4b8, Surface.Render], [0xb5bda4, Surface.Render], [0xa9a59c, Surface.Render],
  // France (the importer's `paint::WALLS`): limestone and sandstone are cut stone, the brick brick, plaster and ochre render.
  [0xf0e6cc, Surface.Ashlar], [0xe6d3b2, Surface.Ashlar], [0xdcc6a2, Surface.Ashlar],
  [0xf4f0e6, Surface.Render], [0xd8d3c8, Surface.Render], [0xe9c9b6, Surface.Render], [0xead0a0, Surface.Render],
  [0xc0785a, Surface.Brick],
])

/** Roofs that are not red but laid in rows all the same: slate (France's `paint::ROOFS` and the grey-blue here). */
const SLATE = new Set([0x56606e, 0x7b8694, 0x4f5d6b])
/** The timber roof: wooden shingles. */
const SHINGLED = new Set([0x8d6e63])

/**
 * The patterns on building `index`'s walls and roof: from the palette colour it was given (Sweden's
 * here, or France's as the importer painted it), else render, or brick if it is a dark red; a flat roof
 * is felt, the timber one wooden shingles, a red or slate one tiles and any other sheet metal. A
 * bridge is plain concrete with what it carries on its deck (`DECK_ASPHALT`, `DECK_PLANKS`, `DECK_BALLAST`).
 */
export function buildingSurfaces(tile: Pick<BuildingTile, 'buildings'>, index: number, salt: number): { wall: Surface; roof: Surface } {
  const { wall, roof } = buildingColors(tile, index, salt)
  // A bridge is plain concrete with what it carries on its deck: asphalt, planks, or for a railway ballast.
  if (tile.buildings[index]?.kind === BUILDING_BRIDGE) {
    const deck = roof === DECK_ASPHALT ? Surface.Asphalt : roof === DECK_PLANKS ? Surface.Planks : roof === DECK_BALLAST || roof === WINTER_BALLAST_COLOR ? Surface.Ballast : Surface.Plain
    return { wall: Surface.Plain, roof: deck }
  }
  const channels = (hex: number) => [(hex >> 16) & 255, (hex >> 8) & 255, hex & 255] as const
  const [wr, wg, wb] = channels(wall)
  const wallSurface = WALL_SURFACES.get(wall) ?? (wr > wg * 1.6 && wr + wg + wb < 400 ? Surface.Brick : Surface.Render)
  const [rr, rg] = channels(roof)
  const roofSurface = tile.buildings[index]?.roofKind === ROOF_FLAT ? Surface.Felt : SHINGLED.has(roof) ? Surface.Shingles : rr > rg * 1.3 || SLATE.has(roof) ? Surface.Tiles : Surface.Sheet
  return { wall: wallSurface, roof: roofSurface }
}

/** A building's faces as `Shell.facesWithRoof` gives them. */
type Faces = { tris: Int32Array; roof: Uint8Array }

const NONE: ReadonlySet<number> = new Set()
/** A drawn face's normal is rounded to a multiple of this in each axis (about 0.01°): too little to see, enough for faces in one plane to share their corners. */
const NORMAL_STEP = 1 / 4096

/**
 * Flat-shaded triangle soup written straight into arrays of its final size: corners in metres, and
 * a normal and a colour a vertex. The size is known before anything is written (a building's faces
 * are merged first), so a tile's buildings take three allocations and one pass.
 */
class Soup {
  readonly positions: Float32Array
  readonly normals: Float32Array
  readonly colors: Float32Array
  readonly custom: Float32Array
  /** The pattern the next triangles are drawn with. */
  surface: Surface = Surface.Plain
  private at = 0
  /** The box round every corner written: min x, y, z, then max. */
  private readonly box = [Infinity, Infinity, Infinity, -Infinity, -Infinity, -Infinity]

  constructor(triangles: number) {
    this.positions = new Float32Array(triangles * 9)
    this.normals = new Float32Array(triangles * 9)
    this.colors = new Float32Array(triangles * 9)
    this.custom = new Float32Array(triangles * 12)
  }

  /**
   * Appends the triangle `a b c` in the colour `r g b`, its normal `normal` (that of the polygon it
   * is cut from) or else `(c - b) × (a - b)` normalised. The normal is rounded to `NORMAL_STEP`, so
   * the corners of faces in one plane are alike to the bit and `weld` shares them.
   */
  triangle(ax: number, ay: number, az: number, bx: number, by: number, bz: number, cx: number, cy: number, cz: number, r: number, g: number, b: number, normal?: readonly [number, number, number]) {
    let [nx, ny, nz] = normal ?? [0, 0, 0]
    if (!normal) {
      const [ex, ey, ez] = [cx - bx, cy - by, cz - bz]
      const [fx, fy, fz] = [ax - bx, ay - by, az - bz]
      ;[nx, ny, nz] = [ey * fz - ez * fy, ez * fx - ex * fz, ex * fy - ey * fx]
      const length = Math.sqrt(nx * nx + ny * ny + nz * nz) || 1
      ;[nx, ny, nz] = [nx / length, ny / length, nz / length]
    }
    ;[nx, ny, nz] = [Math.round(nx / NORMAL_STEP) * NORMAL_STEP, Math.round(ny / NORMAL_STEP) * NORMAL_STEP, Math.round(nz / NORMAL_STEP) * NORMAL_STEP]
    const { positions, normals, colors } = this
    const i = this.at
    positions[i] = ax
    positions[i + 1] = ay
    positions[i + 2] = az
    positions[i + 3] = bx
    positions[i + 4] = by
    positions[i + 5] = bz
    positions[i + 6] = cx
    positions[i + 7] = cy
    positions[i + 8] = cz
    const box = this.box
    box[0] = Math.min(box[0]!, ax, bx, cx)
    box[1] = Math.min(box[1]!, ay, by, cy)
    box[2] = Math.min(box[2]!, az, bz, cz)
    box[3] = Math.max(box[3]!, ax, bx, cx)
    box[4] = Math.max(box[4]!, ay, by, cy)
    box[5] = Math.max(box[5]!, az, bz, cz)
    for (let k = i; k < i + 9; k += 3) {
      normals[k] = nx
      normals[k + 1] = ny
      normals[k + 2] = nz
      colors[k] = r
      colors[k + 1] = g
      colors[k + 2] = b
    }
    if (this.surface !== Surface.Plain) for (let k = (i / 3) * 4; k < (i / 3 + 3) * 4; k += 4) this.custom[k] = this.surface
    this.at = i + 9
  }

  /**
   * Building faces (`Shell.facesWithRoof`: nine centimetre numbers a triangle), roof triangles in
   * `roof` and the rest in `wall`, with `offset` (metres) taken off every corner.
   */
  faces({ tris, roof }: Faces, wall: Color, top: Color, [ox, oy, oz]: readonly [number, number, number] = [0, 0, 0]) {
    for (let t = 0, i = 0; t < roof.length; t++, i += 9) {
      const c = roof[t] ? top : wall
      this.triangle(
        tris[i]! / 100 - ox, tris[i + 1]! / 100 - oy, tris[i + 2]! / 100 - oz,
        tris[i + 3]! / 100 - ox, tris[i + 4]! / 100 - oy, tris[i + 5]! / 100 - oz,
        tris[i + 6]! / 100 - ox, tris[i + 7]! / 100 - oy, tris[i + 8]! / 100 - oz,
        c.r, c.g, c.b,
      )
    }
  }

  /**
   * `count` triangles of the tile's collision mesh from `from`, in `color`: the importer's own
   * triangles, so what is drawn is what is collided with. The importer winds footprints
   * counter-clockwise in (x, z); seen from +Y that is clockwise, so they are reversed to face
   * outwards and up.
   */
  collision(tile: Pick<BuildingTile, 'collision'>, from: number, count: number, color: Color) {
    const { vertices, triangles } = tile.collision
    for (let t = from; t < from + count; t++) {
      const [a, b, c] = [triangles[t * 3]! * 3, triangles[t * 3 + 2]! * 3, triangles[t * 3 + 1]! * 3]
      this.triangle(vertices[a]!, vertices[a + 1]!, vertices[a + 2]!, vertices[b]!, vertices[b + 1]!, vertices[b + 2]!, vertices[c]!, vertices[c + 1]!, vertices[c + 2]!, color.r, color.g, color.b)
    }
  }

  /** The quad `corners` (in order round it) as two triangles in linear `rgb`, wound as `facing` puts them. */
  quad(corners: [number, number, number][], [r, g, b]: readonly [number, number, number]) {
    const [a, c, d, e] = corners as [[number, number, number], [number, number, number], [number, number, number], [number, number, number]]
    this.triangle(...a, ...c, ...d, r, g, b)
    this.triangle(...a, ...d, ...e, r, g, b)
  }

  /** The polygon `poly` (in order round it) as a fan of triangles in linear `rgb`, all with the polygon's own normal. */
  poly(poly: Poly, [r, g, b]: readonly [number, number, number]) {
    const normal = normalOf(poly)
    for (let i = 1; i + 1 < poly.length; i++) this.triangle(...poly[0]!, ...poly[i]!, ...poly[i + 1]!, r, g, b, normal)
  }

  /** The sphere round the box round every corner: a little looser than `Geometry` finds, but without a second pass. */
  private bounds(): Sphere {
    const [x0, y0, z0, x1, y1, z1] = this.box as [number, number, number, number, number, number]
    if (x0 > x1) return { x: 0, y: 0, z: 0, radius: 0 }
    return { x: (x0 + x1) / 2, y: (y0 + y1) / 2, z: (z0 + z1) / 2, radius: Math.hypot(x1 - x0, y1 - y0, z1 - z0) / 2 }
  }

  /** Vertices written so far. */
  get vertices(): number {
    return this.at / 3
  }

  /**
   * The soup with its equal vertices shared. `ranges` (first and count, two numbers a building, in
   * the order they were written) is rewritten to the shared vertices; none is shared across a range's
   * ends, nor between a range and what was written after the last one.
   */
  buffers(ranges?: Uint32Array): BuildingBuffers {
    return { ...weld(this.positions, this.normals, this.colors, this.custom, this.vertices, ranges), bounds: this.bounds() }
  }
}

/**
 * Shares the equal vertices of a triangle soup of `count` vertices: the arrays cut down to one of each
 * (the first met, so in the soup's order), and the indices that draw the same triangles from them. A
 * vertex is equal to another when every number of it is, bit for bit. Each of `ranges` (as
 * `Soup.buffers` takes them) is shared within itself only, and rewritten.
 *
 * Half a building's vertices are another's twin (faces in one plane share a rounded normal,
 * `Soup.triangle`), and a vertex costs the GPU 64 bytes where an index costs 4: a town's tile is
 * several MB lighter for it, and draws fewer vertices.
 */
function weld(positions: Float32Array, normals: Float32Array, colors: Float32Array, custom: Float32Array, count: number, ranges?: Uint32Array): Pick<BuildingBuffers, 'positions' | 'normals' | 'colors' | 'custom' | 'indices'> {
  const [p, n, c, x] = [bitsOf(positions), bitsOf(normals), bitsOf(colors), bitsOf(custom)]
  const indices = new Uint32Array(count)
  // Open addressing, at most half full: a slot holds a kept vertex's number plus one, or 0.
  let size = 16
  while (size < count * 2) size *= 2
  const slots = new Uint32Array(size)
  const mask = size - 1
  let kept = 0
  /** Shares soup vertices `from` up to `to` among themselves. */
  const share = (from: number, to: number) => {
    const first = kept
    for (let v = from; v < to; v++) {
      const [v3, v4] = [v * 3, v * 4]
      let hash = 0x811c9dc5
      for (let k = 0; k < 3; k++) {
        hash = Math.imul(hash ^ p[v3 + k]!, 0x01000193)
        hash = Math.imul(hash ^ n[v3 + k]!, 0x01000193)
        hash = Math.imul(hash ^ c[v3 + k]!, 0x01000193)
      }
      hash = Math.imul(hash ^ x[v4]!, 0x01000193)
      let slot = (hash ^ (hash >>> 15)) & mask
      let found = -1
      for (;;) {
        const held = slots[slot]!
        if (held === 0) break
        const u = held - 1
        // One kept before this range began is another building's, however alike.
        if (u >= first) {
          const [u3, u4] = [u * 3, u * 4]
          if (
            p[u3] === p[v3] && p[u3 + 1] === p[v3 + 1] && p[u3 + 2] === p[v3 + 2] &&
            n[u3] === n[v3] && n[u3 + 1] === n[v3 + 1] && n[u3 + 2] === n[v3 + 2] &&
            c[u3] === c[v3] && c[u3 + 1] === c[v3 + 1] && c[u3 + 2] === c[v3 + 2] &&
            x[u4] === x[v4] && x[u4 + 1] === x[v4 + 1] && x[u4 + 2] === x[v4 + 2] && x[u4 + 3] === x[v4 + 3]
          ) {
            found = u
            break
          }
        }
        slot = (slot + 1) & mask
      }
      if (found < 0) {
        found = kept++
        slots[slot] = found + 1
        // Kept vertices are never ahead of the soup's, so they are packed down in place.
        const [f3, f4] = [found * 3, found * 4]
        for (let k = 0; k < 3; k++) {
          p[f3 + k] = p[v3 + k]!
          n[f3 + k] = n[v3 + k]!
          c[f3 + k] = c[v3 + k]!
        }
        for (let k = 0; k < 4; k++) x[f4 + k] = x[v4 + k]!
      }
      indices[v] = found
    }
    return first
  }
  let done = 0
  if (ranges) {
    for (let i = 0; i < ranges.length; i += 2) {
      const [from, length] = [ranges[i]!, ranges[i + 1]!]
      if (length === 0) continue
      ranges[i] = share(from, from + length)
      ranges[i + 1] = kept - ranges[i]!
      done = from + length
    }
  }
  share(done, count)
  // Copies of the kept part, so the soup's full-size arrays are let go.
  return { positions: positions.slice(0, kept * 3), normals: normals.slice(0, kept * 3), colors: colors.slice(0, kept * 3), custom: custom.slice(0, kept * 4), indices }
}

/** The bits of `floats`, to tell two of them apart without comparing as numbers. */
function bitsOf(floats: Float32Array): Uint32Array {
  return new Uint32Array(floats.buffer, floats.byteOffset, floats.length)
}

/**
 * Building `index` as cubes (`voxels.ts`): the outer faces of its cells, as `Shell.facesWithRoof`
 * merges them; it is collided with as boxes. Null when it has no cells to draw (the importer left
 * it no walls, or it is too big to cut).
 */
function buildingFaces(tile: BuildingTile, index: number): Faces | null {
  return shellOf(tile, index)?.facesWithRoof(NONE) ?? null
}

/** Building `index`'s wall and roof colours, as linear `Color`s for vertex colours. */
function linearColors(tile: BuildingTile, index: number, salt: number): [Color, Color] {
  const { wall, roof } = buildingColors(tile, index, salt)
  return [Color.hex(wall), Color.hex(roof)]
}

/**
 * The tile's buildings (as cubes) and bridges as one soup, leaving out the buildings in
 * `destroyed`. Null when there is nothing to draw.
 */
export function buildingBuffers(tile: BuildingTile, salt: number, destroyed: ReadonlySet<number>): BuildingBuffers | null {
  // Each building's faces first, so the arrays are made once at their final size. A building
  // without cells is drawn from its collision triangles.
  // A building with a look is drawn from it, whole: its own walls and roof, not its cubes.
  const look = tile.look
  const looked = (index: number) => (look && tile.buildings[index]?.look) || null
  const faces: (Faces | null | undefined)[] = tile.buildings.map((_, index) => (destroyed.has(index) || looked(index) ? undefined : buildingFaces(tile, index)))
  // Windows and doors on the buildings drawn from their looks: two quads, four triangles, each; and their roofs' overhangs.
  // The walls with their windows cut out, the roof, and the room inside: the same faces turned in, and a floor,
  // or for a house with rooms its floors, the walls between them and its stairs.
  const facades: (Facade | null)[] = tile.buildings.map((_, index) => {
    const seen = destroyed.has(index) ? null : looked(index)
    return seen ? facadeFor(look!, seen) : null
  })
  const openings: (readonly Opening[] | null)[] = tile.buildings.map((_, index) => {
    const seen = destroyed.has(index) ? null : looked(index)
    return seen ? drawnOpenings(openingsFor(look!, seen), (facades[index]?.inside.length ?? 0) > 0) : null
  })
  // The glass goes with every looked building, hurt or not: its panes break on their own (`Glazing`).
  const glass = glassBuffers(tile.buildings.map((_, index) => (looked(index) ? openingsFor(look!, looked(index)!) : null)))
  const eaves: (readonly EaveStrip[] | null)[] = tile.buildings.map((_, index) => {
    const seen = destroyed.has(index) ? null : looked(index)
    return seen ? eavesFor(look!, seen) : null
  })
  // A house's chimney and porch.
  const details: (DetailQuad[] | null)[] = tile.buildings.map((_, index) => {
    const seen = destroyed.has(index) ? null : looked(index)
    return seen ? detailsOf(look!, seen, openings[index] ?? []) : null
  })
  const fan = (polys: readonly Poly[]) => polys.reduce((n, poly) => n + poly.length - 2, 0)
  let count = 0
  tile.buildings.forEach((building, index) => {
    const own = faces[index]
    const seen = destroyed.has(index) ? null : looked(index)
    const facade = facades[index]
    if (seen && facade) count += (building.kind === BUILDING_BRIDGE ? 1 : 2) * (fan(facade.walls) + fan(facade.roof)) + fan(facade.floor) + 2 * facade.inside.length + openingTriangles(openings[index] ?? []) + eaveTriangles(eaves[index] ?? []) + detailTriangles(details[index] ?? [])
    else if (own) count += own.roof.length
    else if (own === null) count += building.walls + building.roof
  })
  for (const bridge of tile.bridges) count += bridge.steel
  if (count === 0) return glass ? { ...new Soup(0).buffers(), ranges: new Uint32Array(tile.buildings.length * 2), glass } : null
  const soup = new Soup(count)
  const ranges = new Uint32Array(tile.buildings.length * 2)
  tile.buildings.forEach((building, index) => {
    if (destroyed.has(index)) return
    const own = faces[index]
    const seen = looked(index)
    const first = soup.vertices
    const [wall, roof] = linearColors(tile, index, salt)
    const surfaces = buildingSurfaces(tile, index, salt)
    const facade = facades[index]
    if (seen && facade) {
      const dim = (c: Color) => [c.r * INTERIOR_SHADE, c.g * INTERIOR_SHADE, c.b * INTERIOR_SHADE] as const
      soup.surface = surfaces.wall
      for (const poly of facade.walls) soup.poly(poly, [wall.r, wall.g, wall.b])
      soup.surface = surfaces.roof
      for (const poly of facade.roof) soup.poly(poly, [roof.r, roof.g, roof.b])
      // The room inside: the walls' and roof's inner faces, dim, and the floor. A whole bridge has no
      // inside to see, and its rails are thinner than a wall, so their inner faces would stand out beside them.
      soup.surface = Surface.Plain
      if (building.kind !== BUILDING_BRIDGE) {
        for (const poly of facade.walls) soup.poly(innerOf(poly, normalOf(poly), facade.inset), dim(wall))
        for (const poly of facade.roof) soup.poly(innerOf(poly, normalOf(poly), facade.inset), dim(roof))
      }
      for (const poly of facade.floor) soup.poly(poly, FLOOR_RGB)
      // Its rooms, seen from either side: a floor is the ceiling of the storey under it.
      for (const { poly, kind } of facade.inside) {
        const rgb = INSIDE_RGB[kind] ?? FLOOR_RGB
        soup.poly(poly, rgb)
        soup.poly([poly[2], poly[1], poly[0]], rgb)
      }
      // The overhang: the roof's pattern carries on over the top of it; the boards under and along it are plain.
      eaveQuads(eaves[index] ?? [], [roof.r, roof.g, roof.b], (corners, rgb, part) => {
        soup.surface = part === 'top' ? surfaces.roof : Surface.Plain
        soup.quad(corners, rgb)
      })
      soup.surface = Surface.Plain
      openingQuads(openings[index] ?? [], (corners, rgb, normal, masonry) => {
        // The wall filled in round an arched head is wall, in its colour and pattern.
        soup.surface = masonry ? surfaces.wall : Surface.Plain
        soup.quad(facing(corners, normal), masonry ? [wall.r, wall.g, wall.b] : rgb)
      })
      for (const { corners, normal, paint } of details[index] ?? []) {
        soup.surface = paint === 'chimney' ? Surface.Brick : paint === 'roof' ? surfaces.roof : Surface.Plain
        soup.quad(facing(corners, normal), paint === 'chimney' ? CHIMNEY_RGB : paint === 'roof' ? [roof.r, roof.g, roof.b] : DETAIL_TRIM_RGB)
      }
      soup.surface = Surface.Plain
    } else if (own) soup.faces(own, wall, roof)
    else {
      soup.surface = surfaces.wall
      soup.collision(tile, building.first, building.walls, wall)
      soup.surface = surfaces.roof
      soup.collision(tile, building.first + building.walls, building.roof, roof)
      soup.surface = Surface.Plain
    }
    ranges[index * 2] = first
    ranges[index * 2 + 1] = soup.vertices - first
  })
  // Bridges' steel pylons and cables: their concrete is in sections drawn as buildings.
  const steel = Color.hex(BRIDGE_STEEL_COLOR)
  for (const bridge of tile.bridges) soup.collision(tile, bridge.first, bridge.steel, steel)
  return { ...soup.buffers(ranges), ranges, ...(glass ? { glass } : {}) }
}

