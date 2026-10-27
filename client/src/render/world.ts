import { Geometry, Group, Material, Mesh, planeGeometry, type Node } from '@ragnarok/engine'
import { toonMaterial } from './toon'
import type { TileGround } from '@/sim/terrain'
import type { TileData } from '@/world'
import { buildingBuffers, buildingColors, oneBuildingBuffers, type BuildingBuffers } from './buildingmesh'
import { GRASS_COLOR, bakeTerrainTexture, coverTexture, reliefTexture } from './cover'
import type { Ribbon } from './groundmesh'
import type { HeightSource } from './shade'
import { reliefPixels, type Pixels, type TerrainBuffers } from './terrainmesh'
import { prepareTile, type PreparedTile } from './tileprep'
import { buildTrees } from './trees'
import { TERRAIN_RAMP } from './groundlight'

export type { HeightSource }

/** Ribbon materials are shared by every tile too, by colour and depth bias. */
const ribbonMaterials = new Map<string, Material>()

/**
 * Roads' and rails' ribbons (`roads.ts`, `rails.ts`) as meshes, toned like the terrain under them.
 * Each layer pulls further towards the camera (`offset`), so overlapping ribbons settle in a fixed
 * order and clear the terrain at any distance; the lift alone covers the near field, where depth
 * is fine. A flat quad between samples cuts the terrain's kinks, so the lift must exceed that chord error.
 */
function ribbonMeshes(ribbons: readonly Ribbon[]): Mesh[] {
  return ribbons.map(({ buffers, color, offset }) => {
    const key = `${color}:${offset}`
    let material = ribbonMaterials.get(key)
    if (!material) {
      material = toonMaterial({ color, ramp: TERRAIN_RAMP, vertexColors: true, depthBias: offset, depthBiasSlope: 2, craters: true })
      ribbonMaterials.set(key, material)
    }
    const mesh = new Mesh(new Geometry(buffers), material)
    mesh.receiveShadow = true
    mesh.userData.ribbon = true
    return mesh
  })
}

/** Draws a tile's roads and rails again, from ribbons made on its new ground (`ribbonsOf`). */
export function replaceRibbons(group: Group, ribbons: readonly Ribbon[]) {
  const stale = group.children.filter((child): child is Mesh => child.userData.ribbon === true && child instanceof Mesh)
  for (const mesh of stale) {
    mesh.geometry.dispose()
    group.remove(mesh)
  }
  group.add(...ribbonMeshes(ribbons))
}

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
  const ground = new Mesh(planeGeometry(halfExtent * 2, halfExtent * 2), toonMaterial({ color: GRASS_COLOR, craters: true }))
  ground.position.y = y
  ground.receiveShadow = true
  return ground
}

/**
 * One tile's terrain mesh, from what `prepareTile` made of it. Its colour is the painted cover and
 * its shading the relief map, both the tile's own, so the mesh can be simplified without losing
 * either. A cover nothing painted yet is baked here.
 */
function buildTerrain(tile: Pick<TileData, 'cover' | 'detail'>, ground: TileGround, terrain: NonNullable<PreparedTile['terrain']>, cover: PreparedTile['cover']): Mesh {
  const { x0, z0, cell, side } = ground
  const map = cover === undefined ? bakeTerrainTexture(tile, x0, z0, cell * (side - 1)) : cover ? coverTexture(cover) : null
  const normalMap = reliefTexture(terrain.relief)
  const mesh = new Mesh(new Geometry(terrain.buffers), toonMaterial({ ...(map ? { map } : { color: GRASS_COLOR }), normalMap, normalMapTone: true, ramp: TERRAIN_RAMP, craters: true }))
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

/** Gives a tile's terrain a new relief map, as `restitchTerrain` or the tile worker made it. */
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
 * A prepared tile (`prepareTile`) as meshes: its terrain, roads and rails draped on it, its
 * buildings and bridges merged into one mesh with a colour a vertex, and its trees. Water is drawn
 * apart (`buildWater`). Only wrapping arrays, so it is cheap enough for the frame.
 */
export function tileGroup(tile: Pick<TileData, 'cover' | 'detail' | 'trees'>, ground: TileGround | null, prepared: PreparedTile): Group {
  const group = new Group()
  if (ground && prepared.terrain) group.add(buildTerrain(tile, ground, prepared.terrain, prepared.cover))
  if (prepared.draped) group.add(...ribbonMeshes(prepared.draped.ribbons))
  group.add(...mergedMesh(prepared.buildings, new Set(prepared.undrawn)))
  group.add(...buildTrees(tile.trees))
  return group
}

/**
 * One tile's meshes, prepared here and now at the finest level (`prepareTile`, then `tileGroup`),
 * leaving out the buildings in `destroyed`. `salt` keeps colours stable between reloads and varied
 * across tiles.
 */
export function buildTile(tile: TileData, salt: number, ground: TileGround | null = null, around?: HeightSource, destroyed: ReadonlySet<number> = NONE): Group {
  return tileGroup(tile, ground, prepareTile({ tile, ground, salt, level: 0, undrawn: [...destroyed] }, around))
}

const NONE: ReadonlySet<number> = new Set()

/** Numbers appended three at a time. */
interface Triples {
  readonly length: number
  push(a: number, b: number, c: number): unknown
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
 * The tile's buildings (as cubes) and bridges as one mesh with a colour per vertex
 * (`buildingBuffers`). One draw per tile (and one shadow draw) however many colours it holds.
 */
function mergedMesh(buffers: BuildingBuffers | null, destroyed: ReadonlySet<number>): Mesh[] {
  if (!buffers) return []
  const { positions, normals, colors, bounds } = buffers
  const mesh = new Mesh(new Geometry({ positions, normals, colors, bounds }), buildingMaterial)
  mesh.castShadow = true
  mesh.receiveShadow = true
  mesh.userData.merged = true
  const merged: Merged = { ranges: buffers.ranges ?? null, left: new Set(destroyed) }
  mesh.userData.buildings = merged
  return [mesh]
}

/** What a tile's merged mesh knows of its buildings: where each one's vertices are, and those it leaves out. */
interface Merged {
  ranges: Uint32Array | null
  left: Set<number>
}

/**
 * Redraws a tile's buildings without those in `destroyed` (indices into `tile.buildings`). When
 * that only leaves out more of them, as when one falls or starts to be drawn as shards, their
 * vertices are collapsed to a point in place and only they are sent again; otherwise the mesh is
 * built anew.
 */
export function rebuildBuildings(group: Group, tile: TileData, salt: number, destroyed: ReadonlySet<number>) {
  const drawn = group.children.find((child): child is Mesh => child.userData.merged === true && child instanceof Mesh)
  const merged = drawn?.userData.buildings as Merged | undefined
  if (drawn && merged?.ranges && [...merged.left].every((index) => destroyed.has(index))) {
    const { positions } = drawn.geometry
    for (const index of destroyed) {
      if (merged.left.has(index)) continue
      merged.left.add(index)
      const [first, count] = [merged.ranges[index * 2]!, merged.ranges[index * 2 + 1]!]
      if (count === 0) continue
      // Every corner on the first: no triangle left to draw, and no shadow.
      const at = first * 3
      for (let k = at + 3; k < (first + count) * 3; k += 3) positions.copyWithin(k, at, at + 3)
      drawn.geometry.needsUpdateRange(first, count)
    }
    return
  }
  const stale = group.children.filter((child): child is Mesh => child.userData.merged === true && child instanceof Mesh)
  for (const mesh of stale) {
    mesh.geometry.dispose()
    group.remove(mesh)
  }
  group.add(...mergedMesh(buildingBuffers(tile, salt, destroyed), destroyed))
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
  const cubes = oneBuildingBuffers(tile, index, salt, [0, building.base, 0])
  if (cubes) {
    const mesh = new Mesh(new Geometry(cubes), buildingMaterial)
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
