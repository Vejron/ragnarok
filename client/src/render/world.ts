import { Geometry, Group, Material, Mesh, planeGeometry, type Node, type Sphere } from '@ragnarok/engine'
import * as THREE from 'three'
import { toonMaterial } from './toon'
import type { TileGround } from '@/sim/terrain'
import { ROOF_FLAT, type TileData } from '@/world'
import { shellOf } from '@/sim/carve'
import { GRASS_COLOR, bakeTerrainTexture, coverTexture, reliefTexture } from './cover'
import { buildRails } from './rails'
import { buildRoads } from './roads'
import { heightSampler, type HeightSource } from './shade'
import { reliefPixels, terrainBuffers, TERRAIN_LOD_ERRORS, type Pixels, type TerrainBuffers } from './terrainmesh'
import type { PreparedTerrain } from './terrainworker'
import { buildTrees } from './trees'
import { TERRAIN_RAMP } from './groundlight'

export type { HeightSource }

/** Candy-coloured buildings. Picked per building so the palette stays stable between reloads. */
const BRIDGE_DECK_COLOR = 0xb8b6ae
const BRIDGE_STEEL_COLOR = 0x6d7a86
const BUILDING_COLORS = [0xffd6a5, 0xfdffb6, 0xcaffbf, 0x9bf6ff, 0xa0c4ff, 0xbdb2ff, 0xffc6ff, 0xffadad]

/** Roofs: slate, tile and timber for pitched ones, concrete grey for flat ones. */
const ROOF_COLORS = [0xb5543f, 0x6d7a8c, 0x8d6e63, 0x4f5d6b, 0x9c4a3c]
const FLAT_ROOF_COLORS = [0xb0b4ba, 0x9aa0a6, 0xc4c7cc]

/** Every tile's buildings share this one; each vertex carries its colour. */
const buildingMaterial = toonMaterial({ vertexColors: true })

/** Materials are shared by every tile, so streaming one in or out never compiles a shader. */
const materials = new Map<string, Material>()
function material(kind: 'building', color: number) {
  const key = `${kind}:${color}`
  let found = materials.get(key)
  if (!found) {
    found = toonMaterial({ color })
    materials.set(key, found)
  }
  return found
}

/**
 * The grass, as one plane under the whole world: the ground of a flat world, and on a hilly one
 * what shows through where a tile has not streamed in yet. `y` sits it just under the terrain.
 */
export function buildGround(halfExtent: number, y = 0): Mesh {
  const ground = new Mesh(planeGeometry(halfExtent * 2, halfExtent * 2), toonMaterial({ color: GRASS_COLOR }))
  ground.position.y = y
  ground.receiveShadow = true
  return ground
}

/**
 * One tile's terrain (`terrainBuffers`), from what the terrain worker made of it or, without one,
 * built here and now at the finest level. Its colour is the painted cover and its shading the relief
 * map, both the tile's own, so the mesh can be simplified without losing either.
 */
function buildTerrain(tile: Pick<TileData, 'cover' | 'detail'>, ground: TileGround, around?: HeightSource, prepared?: PreparedTerrain): Mesh {
  const { x0, z0, cell, side } = ground
  const geometry = new Geometry(prepared?.buffers ?? terrainBuffers(ground, TERRAIN_LOD_ERRORS[0]))
  const map = prepared ? (prepared.cover ? coverTexture(prepared.cover) : null) : bakeTerrainTexture(tile, x0, z0, cell * (side - 1))
  const normalMap = reliefTexture(prepared?.relief ?? reliefPixels(ground, around, tile.detail))
  const mesh = new Mesh(geometry, toonMaterial({ ...(map ? { map } : { color: GRASS_COLOR }), normalMap, normalMapTone: true, ramp: TERRAIN_RAMP }))
  mesh.userData.ownsMaterial = true
  mesh.userData.ground = ground
  mesh.userData.detail = tile.detail
  mesh.receiveShadow = true
  return mesh
}

/** A tile's terrain mesh, if it has one. */
function terrainMesh(group: Group): Mesh | null {
  for (const child of group.children) if (child.userData.ground && child instanceof Mesh) return child
  return null
}

/** Works out a terrain mesh's relief along its edges again once a neighbouring tile has arrived, so the seam closes. */
export function restitchTerrain(group: Group, around: HeightSource) {
  const mesh = terrainMesh(group)
  const relief = mesh?.material.normalMap
  if (!mesh || !relief) return
  shadeTerrain(group, reliefPixels(mesh.userData.ground as TileGround, around, mesh.userData.detail as Int8Array | undefined, relief.source as Pixels))
}

/** Gives a tile's terrain a new relief map, as `restitchTerrain` or the terrain worker made it. */
export function shadeTerrain(group: Group, relief: Pixels) {
  const texture = terrainMesh(group)?.material.normalMap
  if (!texture) return
  texture.source = relief
  texture.needsUpdate()
}

/** Gives a tile's terrain a new mesh: the same ground at another level of detail. */
export function remeshTerrain(group: Group, buffers: TerrainBuffers) {
  const mesh = terrainMesh(group)
  if (!mesh) return
  mesh.geometry.dispose()
  mesh.geometry = new Geometry(buffers)
}

/**
 * One tile's terrain and buildings (extruded footprints), merged by colour so a tile is a handful
 * of draw calls. `salt` keeps colours stable between reloads and varied across tiles. Roads and
 * tracks are ribbons draped on the terrain; ground cover is painted into the terrain's texture.
 */
export function buildTile(
  tile: TileData,
  salt: number,
  ground: TileGround | null = null,
  around?: HeightSource,
  destroyed: ReadonlySet<number> = NONE,
  terrain?: PreparedTerrain,
): Group {
  const group = new Group()
  if (ground) {
    group.add(buildTerrain(tile, ground, around, terrain))
    const size = (ground.side - 1) * ground.cell
    const height = heightSampler(ground, around)
    const border = [ground.x0, ground.z0, ground.x0 + size, ground.z0 + size] as const
    group.add(...buildRoads(tile.roads, height, ground.cell, border, TERRAIN_RAMP))
    group.add(...buildRails(tile.rails, height, ground.cell, border, TERRAIN_RAMP))
  }
  group.add(...buildMerged(tile, salt, destroyed))
  group.add(...buildTrees(tile.trees))
  return group
}

const NONE: ReadonlySet<number> = new Set()

/** The wall and roof colours of building `index`: stable between reloads, varied across tiles by `salt`. */
export function buildingColors(tile: TileData, index: number, salt: number): { wall: number; roof: number } {
  const building = tile.buildings[index]!
  const palette = building?.roofKind === ROOF_FLAT ? FLAT_ROOF_COLORS : ROOF_COLORS
  return { wall: BUILDING_COLORS[(index + salt) % BUILDING_COLORS.length]!, roof: palette[(index + salt) % palette.length]! }
}

/** Numbers appended three at a time. */
interface Triples {
  readonly length: number
  push(a: number, b: number, c: number): unknown
}

/**
 * Flat-shaded triangle soup written straight into arrays of its final size: corners in metres, and
 * a normal and a colour a vertex. The size is known before anything is written (a building's faces
 * are merged first), so a tile's buildings take three allocations and one pass.
 */
class Soup {
  readonly positions: Float32Array
  readonly normals: Float32Array
  readonly colors: Float32Array
  private at = 0
  /** The box round every corner written: min x, y, z, then max. */
  private readonly box = [Infinity, Infinity, Infinity, -Infinity, -Infinity, -Infinity]

  constructor(triangles: number) {
    this.positions = new Float32Array(triangles * 9)
    this.normals = new Float32Array(triangles * 9)
    this.colors = new Float32Array(triangles * 9)
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
    this.at = i + 9
  }

  /**
   * Building faces (`Shell.facesWithRoof`: nine centimetre numbers a triangle), roof triangles in
   * `roof` and the rest in `wall`, with `offset` (metres) taken off every corner.
   */
  faces({ tris, roof }: Faces, wall: THREE.Color, top: THREE.Color, [ox, oy, oz]: readonly [number, number, number] = [0, 0, 0]) {
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

  /** `count` triangles of the tile's collision mesh from `from` (`trianglePositions`), in `color`. */
  collision(tile: TileData, from: number, count: number, color: THREE.Color) {
    const { vertices, triangles } = tile.collision
    for (let t = from; t < from + count; t++) {
      // Reversed to face outwards and up, as `trianglePositions` does.
      const [a, b, c] = [triangles[t * 3]! * 3, triangles[t * 3 + 2]! * 3, triangles[t * 3 + 1]! * 3]
      this.triangle(vertices[a]!, vertices[a + 1]!, vertices[a + 2]!, vertices[b]!, vertices[b + 1]!, vertices[b + 2]!, vertices[c]!, vertices[c + 1]!, vertices[c + 2]!, color.r, color.g, color.b)
    }
  }

  /** The sphere round the box round every corner: a little looser than `Geometry` finds, but without a second pass. */
  bounds(): Sphere {
    const [x0, y0, z0, x1, y1, z1] = this.box as [number, number, number, number, number, number]
    if (x0 > x1) return { x: 0, y: 0, z: 0, radius: 0 }
    return { x: (x0 + x1) / 2, y: (y0 + y1) / 2, z: (z0 + z1) / 2, radius: Math.hypot(x1 - x0, y1 - y0, z1 - z0) / 2 }
  }

  geometry(): Geometry {
    return new Geometry({ positions: this.positions, normals: this.normals, colors: this.colors, bounds: this.bounds() })
  }
}

/** A building's faces as `Shell.facesWithRoof` gives them. */
type Faces = { tris: Int32Array; roof: Uint8Array }

/**
 * Triangle soup for `count` triangles of the tile's collision mesh from `from`: the importer's
 * own triangles, so what is drawn is what is collided with. The importer winds footprints
 * counter-clockwise in (x, z); seen from +Y that is clockwise, so they are reversed to face
 * outwards and up. `lower` is taken off every height.
 */
function trianglePositions<T extends Triples>(tile: TileData, from: number, count: number, out: T, lower = 0): T {
  const { vertices, triangles } = tile.collision
  for (let t = from; t < from + count; t++) {
    for (const k of [0, 2, 1]) {
      const v = triangles[t * 3 + k]! * 3
      out.push(vertices[v]!, vertices[v + 1]! - lower, vertices[v + 2]!)
    }
  }
  return out
}

/**
 * Building `index` as cubes (`voxels.ts`): the outer faces of its cells, as `Shell.facesWithRoof`
 * merges them; it is collided with as boxes. Null when it has no cells to draw (the importer left
 * it no walls, or it is too big to cut).
 */
function buildingFaces(tile: TileData, index: number): Faces | null {
  return shellOf(tile, index)?.facesWithRoof(NONE) ?? null
}

/** Building `index`'s wall and roof colours, as linear `THREE.Color`s for vertex colours. */
function linearColors(tile: TileData, index: number, salt: number): [THREE.Color, THREE.Color] {
  const { wall, roof } = buildingColors(tile, index, salt)
  return [new THREE.Color(wall), new THREE.Color(roof)]
}

/**
 * The tile's buildings (as cubes) and bridges as one mesh with a colour per vertex, leaving out the
 * buildings in `destroyed`. One draw per tile (and one shadow draw) however many colours it holds.
 */
function buildMerged(tile: TileData, salt: number, destroyed: ReadonlySet<number>): Mesh[] {
  // Each building's faces first, so the arrays are made once at their final size. A building
  // without cells is drawn from its collision triangles.
  const faces: (Faces | null | undefined)[] = tile.buildings.map((_, index) => (destroyed.has(index) ? undefined : buildingFaces(tile, index)))
  let count = 0
  tile.buildings.forEach((building, index) => {
    const own = faces[index]
    if (own) count += own.roof.length
    else if (own === null) count += building.walls + building.roof
  })
  for (const bridge of tile.bridges) count += bridge.deck + bridge.steel
  if (count === 0) return []
  const soup = new Soup(count)
  tile.buildings.forEach((building, index) => {
    const own = faces[index]
    if (own === undefined) return
    const [wall, roof] = linearColors(tile, index, salt)
    if (own) return soup.faces(own, wall, roof)
    soup.collision(tile, building.first, building.walls, wall)
    soup.collision(tile, building.first + building.walls, building.roof, roof)
  })
  // Bridges: concrete deck, piers and parapets, then steel pylon and cables.
  const [deck, steel] = [new THREE.Color(BRIDGE_DECK_COLOR), new THREE.Color(BRIDGE_STEEL_COLOR)]
  for (const bridge of tile.bridges) {
    soup.collision(tile, bridge.first, bridge.deck, deck)
    soup.collision(tile, bridge.first + bridge.deck, bridge.steel, steel)
  }
  const mesh = new Mesh(soup.geometry(), buildingMaterial)
  mesh.castShadow = true
  mesh.receiveShadow = true
  mesh.userData.merged = true
  return [mesh]
}

/** Redraws a tile's buildings without those in `destroyed` (indices into `tile.buildings`). */
export function rebuildBuildings(group: Group, tile: TileData, salt: number, destroyed: ReadonlySet<number>) {
  const stale = group.children.filter((child): child is Mesh => child.userData.merged === true && child instanceof Mesh)
  for (const mesh of stale) {
    mesh.geometry.dispose()
    group.remove(mesh)
  }
  group.add(...buildMerged(tile, salt, destroyed))
}

/**
 * Building `index` as a mesh of its own, for it to be seen collapsing: its walls and roof, with
 * the floor at the group's origin so scaling `y` sinks it into the ground.
 */
export function collapsingBuilding(tile: TileData, index: number, salt: number): Group {
  const building = tile.buildings[index]!
  const { wall, roof } = buildingColors(tile, index, salt)
  const group = new Group()
  group.position.y = building.base
  const faces = buildingFaces(tile, index)
  if (faces) {
    const soup = new Soup(faces.roof.length)
    soup.faces(faces, ...linearColors(tile, index, salt), [0, building.base, 0])
    const mesh = new Mesh(soup.geometry(), buildingMaterial)
    mesh.castShadow = true
    group.add(mesh)
    return group
  }
  for (const [color, from, count] of [[wall, building.first, building.walls], [roof, building.first + building.walls, building.roof]] as const) {
    const mesh = new Mesh(flatGeometry(trianglePositions(tile, from, count, [], building.base)), material('building', color))
    mesh.castShadow = true
    group.add(mesh)
  }
  return group
}

/** Frees a tile's geometry and its terrain texture. The materials and tree prototypes are shared and stay. */
export function disposeTile(group: Group) {
  group.traverse((object: Node) => {
    if (!(object instanceof Mesh)) return
    // Tree prototypes are shared by every tile; an instanced mesh has no buffers of its own.
    if (!object.userData.sharedGeometry) object.geometry.dispose()
    if (object.userData.ownsMaterial) {
      const material = object.material
      material.map?.dispose()
      material.normalMap?.dispose()
      material.dispose()
    }
  })
  group.removeFromParent()
}

/** Flat-shaded triangle soup (metres, nine numbers a triangle) with an optional colour per vertex. */
export function flatGeometry(positions: readonly number[], colors?: readonly number[]): Geometry {
  const array = Float32Array.from(positions)
  const normals = new Float32Array(array.length)
  for (let i = 0; i < array.length; i += 9) {
    // (c - b) × (a - b), normalised.
    const [ex, ey, ez] = [array[i + 6]! - array[i + 3]!, array[i + 7]! - array[i + 4]!, array[i + 8]! - array[i + 5]!]
    const [fx, fy, fz] = [array[i]! - array[i + 3]!, array[i + 1]! - array[i + 4]!, array[i + 2]! - array[i + 5]!]
    let [nx, ny, nz] = [ey * fz - ez * fy, ez * fx - ex * fz, ex * fy - ey * fx]
    const length = Math.sqrt(nx * nx + ny * ny + nz * nz) || 1
    ;[nx, ny, nz] = [nx / length, ny / length, nz / length]
    for (let k = 0; k < 9; k += 3) {
      normals[i + k] = nx
      normals[i + k + 1] = ny
      normals[i + k + 2] = nz
    }
  }
  return new Geometry({ positions: array, normals, colors: colors && Float32Array.from(colors) })
}
