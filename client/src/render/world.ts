import { Geometry, Group, Material, Mesh, planeGeometry, type Node } from '@ragnarok/engine'
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

/** Numbers appended three at a time: a `number[]`, or `Floats`. */
interface Triples {
  readonly length: number
  push(a: number, b: number, c: number): unknown
}

/** Numbers appended three at a time into a typed array that grows, for big meshes. */
class Floats implements Triples {
  private data = new Float32Array(3 * 4096)
  length = 0

  push(a: number, b: number, c: number) {
    if (this.length + 3 > this.data.length) {
      const grown = new Float32Array(this.data.length * 2)
      grown.set(this.data)
      this.data = grown
    }
    this.data[this.length] = a
    this.data[this.length + 1] = b
    this.data[this.length + 2] = c
    this.length += 3
  }

  /** What was appended. */
  array(): Float32Array {
    return this.data.subarray(0, this.length)
  }
}

/** Triangle soup, metres, with a colour per vertex. */
interface Soup {
  positions: Triples
  colors: Triples
}

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
 * Building `index` as cubes (`voxels.ts`), the roof's cells in the roof colour, appended to `out`.
 * False when it has no cells to draw (the importer left it no walls, or it is too big to cut).
 */
function buildingVoxels(tile: TileData, index: number, salt: number, out: Soup, offset: readonly [number, number, number] = [0, 0, 0]): boolean {
  const shell = shellOf(tile, index)
  if (!shell) return false
  // The outer faces of its cubes (`Shell.facesWithRoof`); it is collided with as boxes.
  const { tris, roof } = shell.facesWithRoof(new Set())
  const colors = buildingColors(tile, index, salt)
  const [wall, top] = [new THREE.Color(colors.wall), new THREE.Color(colors.roof)]
  for (let t = 0; t < roof.length; t++) {
    const c = roof[t] ? top : wall
    for (let k = 0; k < 9; k += 3) {
      out.positions.push(tris[t * 9 + k]! / 100 - offset[0], tris[t * 9 + k + 1]! / 100 - offset[1], tris[t * 9 + k + 2]! / 100 - offset[2])
      out.colors.push(c.r, c.g, c.b)
    }
  }
  return true
}

/**
 * The tile's buildings (as cubes) and bridges as one mesh with a colour per vertex, leaving out the
 * buildings in `destroyed`. One draw per tile (and one shadow draw) however many colours it holds.
 */
function buildMerged(tile: TileData, salt: number, destroyed: ReadonlySet<number>): Mesh[] {
  const positions = new Floats()
  const colors = new Floats()
  const soup = { positions, colors }
  const tint = new THREE.Color()
  const add = (color: number, from: number, count: number) => {
    const before = positions.length
    trianglePositions(tile, from, count, positions)
    tint.set(color)
    for (let i = before; i < positions.length; i += 3) colors.push(tint.r, tint.g, tint.b)
  }
  tile.buildings.forEach((building, index) => {
    if (destroyed.has(index)) return
    if (buildingVoxels(tile, index, salt, soup)) return
    const { wall, roof } = buildingColors(tile, index, salt)
    add(wall, building.first, building.walls)
    add(roof, building.first + building.walls, building.roof)
  })
  // Bridges: concrete deck, piers and parapets, then steel pylon and cables.
  for (const bridge of tile.bridges) {
    add(BRIDGE_DECK_COLOR, bridge.first, bridge.deck)
    add(BRIDGE_STEEL_COLOR, bridge.first + bridge.deck, bridge.steel)
  }
  if (positions.length === 0) return []
  const mesh = new Mesh(flatGeometry(positions, colors), buildingMaterial)
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
  const soup = { positions: new Floats(), colors: new Floats() }
  if (buildingVoxels(tile, index, salt, soup, [0, building.base, 0])) {
    const mesh = new Mesh(flatGeometry(soup.positions, soup.colors), buildingMaterial)
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

/** A non-indexed geometry from triangle-soup positions, with one flat normal per face. */
/** Flat-shaded triangle soup (metres, nine numbers a triangle) with an optional colour per vertex. */
export function flatGeometry(positions: number[] | Floats, colors?: Floats | number[]): Geometry {
  const array = positions instanceof Floats ? positions.array() : Float32Array.from(positions)
  const normals = new Float32Array(array.length)
  for (let i = 0; i < array.length; i += 9) {
    // (c - b) × (a - b), normalised.
    const [ex, ey, ez] = [array[i + 6]! - array[i + 3]!, array[i + 7]! - array[i + 4]!, array[i + 8]! - array[i + 5]!]
    const [fx, fy, fz] = [array[i]! - array[i + 3]!, array[i + 1]! - array[i + 4]!, array[i + 2]! - array[i + 5]!]
    let [nx, ny, nz] = [ey * fz - ez * fy, ez * fx - ex * fz, ex * fy - ey * fx]
    const length = Math.hypot(nx, ny, nz) || 1
    ;[nx, ny, nz] = [nx / length, ny / length, nz / length]
    for (let k = 0; k < 9; k += 3) {
      normals[i + k] = nx
      normals[i + k + 1] = ny
      normals[i + k + 2] = nz
    }
  }
  return new Geometry({ positions: array, normals, colors: colors instanceof Floats ? colors.array() : colors && Float32Array.from(colors) })
}
