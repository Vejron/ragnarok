import { Color } from '@ragnarok/engine'
import type { Sphere } from '@ragnarok/engine'
import { ROOF_FLAT, type Bridge, type Building, type TileData } from '@/world'
import { shellOf } from '@/sim/carve'
import { facing, openingQuads, openingsOf, type Opening } from './windows'

/*
 * A tile's buildings and bridges as one flat-shaded triangle soup with a colour a vertex, as plain
 * arrays, as part of preparing a tile (`tileprep.ts`): in the tile worker or on the main thread, which
 * is why nothing here touches the scene.
 */

/**
 * House colours as they are painted here: falu red, ochre, white, render greys, sage, beige and dark
 * stained wood. Picked per building so the palette stays stable between reloads.
 */
const BRIDGE_DECK_COLOR = 0xb8b6ae
const BRIDGE_STEEL_COLOR = 0x6d7a86
const BUILDING_COLORS = [0xa8432f, 0xd9b45a, 0xeee9dc, 0xc9c4b8, 0xb5bda4, 0xd8c6a3, 0x6e5642, 0x8f3a2c, 0xe4dcc6, 0xa9a59c]

/** Roofs: slate, tile and timber for pitched ones, concrete grey for flat ones. */
const ROOF_COLORS = [0xb5543f, 0x6d7a8c, 0x8d6e63, 0x4f5d6b, 0x9c4a3c]
const FLAT_ROOF_COLORS = [0xb0b4ba, 0x9aa0a6, 0xc4c7cc]

/** What of a tile its buildings are drawn from: small enough to send to the worker. */
export interface BuildingTile {
  buildings: readonly Pick<Building, 'first' | 'walls' | 'roof' | 'roofKind' | 'cells' | 'paint' | 'look'>[]
  bridges: readonly Bridge[]
  collision: TileData['collision']
  look?: TileData['look']
}


/** A flat-shaded triangle soup: corners in metres, a normal and a colour a vertex, and the sphere round it. */
export interface BuildingBuffers {
  positions: Float32Array
  normals: Float32Array
  colors: Float32Array
  /** Four numbers a vertex: the first is the surface pattern drawn on it (`Surface`), the rest unused. */
  custom: Float32Array
  bounds: Sphere
  /**
   * For a tile's soup (`buildingBuffers`), each building's vertices in it: first, then count, two
   * numbers a building, a count of 0 for one left out. A building that falls later is taken out of
   * the drawn mesh in place (`rebuildBuildings`).
   */
  ranges?: Uint32Array
}

/** The arrays of `buffers`, so they move to the other thread instead of being copied. */
export function buildingTransferables(buffers: BuildingBuffers): ArrayBuffer[] {
  const arrays = [buffers.positions, buffers.normals, buffers.colors, buffers.custom, ...(buffers.ranges ? [buffers.ranges] : [])]
  return arrays.map((array) => array.buffer as ArrayBuffer)
}

/**
 * The wall and roof colours of building `index`: the ones the importer gave it, or else picked from
 * the palettes, stable between reloads and varied across tiles by `salt`.
 */
export function buildingColors(tile: Pick<BuildingTile, 'buildings'>, index: number, salt: number): { wall: number; roof: number } {
  const building = tile.buildings[index]!
  if (building?.paint) return { wall: building.paint[0], roof: building.paint[1] }
  const palette = building?.roofKind === ROOF_FLAT ? FLAT_ROOF_COLORS : ROOF_COLORS
  return { wall: BUILDING_COLORS[(index + salt) % BUILDING_COLORS.length]!, roof: palette[(index + salt) % palette.length]! }
}

/** Patterns drawn on a building's faces by the shader (walls.wgsl.ts in the engine), as a vertex's custom x. */
export const Surface = { Plain: 0, Upright: 1, Lap: 2, Brick: 3, Render: 4, Tiles: 5, Sheet: 6, Felt: 7 } as const
export type Surface = (typeof Surface)[keyof typeof Surface]

/** Falu red and dark stained wood are upright boards; ochre, white and beige lap boards; the dark red brick; greys and sage render. */
const WALL_SURFACES = new Map<number, Surface>([
  [0xa8432f, Surface.Upright], [0x6e5642, Surface.Upright],
  [0xd9b45a, Surface.Lap], [0xeee9dc, Surface.Lap], [0xe4dcc6, Surface.Lap], [0xd8c6a3, Surface.Lap],
  [0x8f3a2c, Surface.Brick],
  [0xc9c4b8, Surface.Render], [0xb5bda4, Surface.Render], [0xa9a59c, Surface.Render],
])

/**
 * The patterns on building `index`'s walls and roof: from the palette colour it was given, or, painted
 * by the importer, render (brick if the paint is a dark red); a flat roof is felt, a red one tiles and
 * any other sheet metal.
 */
export function buildingSurfaces(tile: Pick<BuildingTile, 'buildings'>, index: number, salt: number): { wall: Surface; roof: Surface } {
  const { wall, roof } = buildingColors(tile, index, salt)
  const channels = (hex: number) => [(hex >> 16) & 255, (hex >> 8) & 255, hex & 255] as const
  const [wr, wg, wb] = channels(wall)
  const wallSurface = WALL_SURFACES.get(wall) ?? (wr > wg * 1.6 && wr + wg + wb < 400 ? Surface.Brick : Surface.Render)
  const [rr, rg] = channels(roof)
  const roofSurface = tile.buildings[index]?.roofKind === ROOF_FLAT ? Surface.Felt : rr > rg * 1.3 ? Surface.Tiles : Surface.Sheet
  return { wall: wallSurface, roof: roofSurface }
}

/** A building's faces as `Shell.facesWithRoof` gives them. */
type Faces = { tris: Int32Array; roof: Uint8Array }

const NONE: ReadonlySet<number> = new Set()

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

  /** Appends the triangle `a b c` in the colour `r g b`, its normal `(c - b) × (a - b)` normalised. */
  triangle(ax: number, ay: number, az: number, bx: number, by: number, bz: number, cx: number, cy: number, cz: number, r: number, g: number, b: number) {
    const [ex, ey, ez] = [cx - bx, cy - by, cz - bz]
    const [fx, fy, fz] = [ax - bx, ay - by, az - bz]
    let [nx, ny, nz] = [ey * fz - ez * fy, ez * fx - ex * fz, ex * fy - ey * fx]
    const length = Math.sqrt(nx * nx + ny * ny + nz * nz) || 1
    ;[nx, ny, nz] = [nx / length, ny / length, nz / length]
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

  buffers(): BuildingBuffers {
    return { positions: this.positions, normals: this.normals, colors: this.colors, custom: this.custom, bounds: this.bounds() }
  }
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
  // Windows and doors on the buildings drawn from their looks: two quads, four triangles, each.
  const openings: (readonly Opening[] | null)[] = tile.buildings.map((_, index) => {
    const seen = destroyed.has(index) ? null : looked(index)
    return seen ? openingsOf(look!, seen) : null
  })
  let count = 0
  tile.buildings.forEach((building, index) => {
    const own = faces[index]
    const seen = destroyed.has(index) ? null : looked(index)
    if (seen) count += seen.walls + seen.roof + (openings[index]?.length ?? 0) * 4
    else if (own) count += own.roof.length
    else if (own === null) count += building.walls + building.roof
  })
  for (const bridge of tile.bridges) count += bridge.deck + bridge.steel
  if (count === 0) return null
  const soup = new Soup(count)
  const ranges = new Uint32Array(tile.buildings.length * 2)
  tile.buildings.forEach((building, index) => {
    if (destroyed.has(index)) return
    const own = faces[index]
    const seen = looked(index)
    const first = soup.vertices
    const [wall, roof] = linearColors(tile, index, salt)
    const surfaces = buildingSurfaces(tile, index, salt)
    if (seen) {
      soup.surface = surfaces.wall
      soup.collision({ collision: look! }, seen.first, seen.walls, wall)
      soup.surface = surfaces.roof
      soup.collision({ collision: look! }, seen.first + seen.walls, seen.roof, roof)
      soup.surface = Surface.Plain
      openingQuads(openings[index] ?? [], (corners, rgb, normal) => soup.quad(facing(corners, normal), rgb))
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
  // Bridges: concrete deck, piers and parapets, then steel pylon and cables.
  const [deck, steel] = [Color.hex(BRIDGE_DECK_COLOR), Color.hex(BRIDGE_STEEL_COLOR)]
  for (const bridge of tile.bridges) {
    soup.collision(tile, bridge.first, bridge.deck, deck)
    soup.collision(tile, bridge.first + bridge.deck, bridge.steel, steel)
  }
  return { ...soup.buffers(), ranges }
}

