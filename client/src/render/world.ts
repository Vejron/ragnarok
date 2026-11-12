import { winter } from './season'
import { Geometry, Group, Material, Mesh, planeGeometry, type Node, type Texture } from '@ragnarok/engine'
import { toonMaterial } from './toon'
import type { TileGround } from '@/sim/terrain'
import type { TileData } from '@/world'
import { buildingBuffers, type BuildingBuffers } from './buildingmesh'
import type { GlassBuffers } from './glass'
import { GRASS_COLOR, bakeTerrainTexture, coverTexture, reliefTexture } from './cover'
import type { Ribbon } from './groundmesh'
import type { HeightSource } from './shade'
import { packMarsh, wetnessPixels } from './marshnoise'
import { reliefPixels, type Pixels, type TerrainBuffers } from './terrainmesh'
import { prepareTile, type PreparedTile } from './tileprep'
import { buildTrees } from './trees'
import { buildLines } from './lines'
import { stonesTexture } from './stones'
import { shoreOf, type Shore } from './shore'
import { TERRAIN_RAMP } from './groundlight'

export type { HeightSource }

/** Ribbon materials drawn whole are shared by every tile too, by colour and depth bias. */
const ribbonMaterials = new Map<string, Material>()

/**
 * Roads' and rails' ribbons (`roads.ts`, `rails.ts`) as meshes, toned like the terrain under them.
 * Each layer pulls further towards the camera (`offset`), so overlapping ribbons settle in a fixed
 * order and clear the terrain at any distance; the lift alone covers the near field, where depth
 * is fine. A flat quad between samples cuts the terrain's kinks, so the lift must exceed that chord error.
 * A worn path's or asphalt road's ribbon is the tile's own: it reads `ground`, the tile's cover map,
 * so its edges fade into the ground beside it (`Material.ground`).
 */
function ribbonMeshes(ribbons: readonly Ribbon[], ground: Texture | null): Mesh[] {
  const own = new Map<string, Material>()
  return ribbons.map(({ buffers, color, look, offset }) => {
    const blends = ground !== null && look !== 'plain'
    const materials = blends ? own : ribbonMaterials
    const key = `${color}:${offset}:${look}`
    let material = materials.get(key)
    if (!material) {
      material = toonMaterial({ color, ramp: TERRAIN_RAMP, vertexColors: true, depthBias: offset, depthBiasSlope: 2, craters: true, worn: look === 'worn', asphalt: look === 'asphalt', ...(blends ? { map: ground, ground: true } : {}) })
      materials.set(key, material)
    }
    const mesh = new Mesh(new Geometry(buffers), material)
    mesh.receiveShadow = true
    mesh.userData.ribbon = true
    // The tile's own: freed with it, but not the cover map, which is the terrain's.
    mesh.userData.groundMaterial = blends
    return mesh
  })
}

/** The cover map a tile's terrain is drawn with, if it has one. */
function groundMap(group: Group): Texture | null {
  return terrainMesh(group)?.material.map ?? null
}

/** Draws a tile's roads and rails again, from ribbons made on its new ground (`ribbonsOf`). */
export function replaceRibbons(group: Group, ribbons: readonly Ribbon[]) {
  const stale = group.children.filter((child): child is Mesh => child.userData.ribbon === true && child instanceof Mesh)
  const owned = new Set<Material>()
  for (const mesh of stale) {
    mesh.geometry.dispose()
    if (mesh.userData.groundMaterial) owned.add(mesh.material)
    group.remove(mesh)
  }
  for (const material of owned) material.dispose()
  group.add(...ribbonMeshes(ribbons, groundMap(group)))
}

/** Every tile's buildings share this one; each vertex carries its colour. */
const buildingMaterial = toonMaterial({ vertexColors: true, walls: true })
/** Window glass: a pale translucent sheet, lit, drawn after everything solid and after water. */
export const glassMaterial = toonMaterial({ color: 0xbcd6ea, opacity: 0.45, blend: 'alpha', depthWrite: false, side: 'double' })
/** Where the glass is drawn among what is translucent: over water (1), so a pane seen over a lake is blended on top of it. */
const GLASS_ORDER = 2

/** Puts snow on the roofs. Done once, when the manifest says it is winter, before any tile is drawn. */
export function snowOnBuildings() {
  buildingMaterial.snow = true
  buildingMaterial.needsUpdate()
}

/**
 * The grass, as one plane under the whole world: the ground of a flat world, and on a hilly one
 * what shows through where a tile has not streamed in yet. `y` sits it just under the terrain.
 */
export function buildGround(halfExtent: number, y = 0): Mesh {
  const ground = new Mesh(planeGeometry(halfExtent * 2, halfExtent * 2), toonMaterial({ color: GRASS_COLOR, craters: true, snow: winter }))
  ground.position.y = y
  ground.receiveShadow = true
  return ground
}

/**
 * One tile's terrain mesh, from what `prepareTile` made of it. Its colour is the painted cover and
 * its shading the relief map, both the tile's own, so the mesh can be simplified without losing
 * either. A cover nothing painted yet is baked here.
 */
function buildTerrain(tile: Pick<TileData, 'cover' | 'detail' | 'stones'>, ground: TileGround, terrain: NonNullable<PreparedTile['terrain']>, cover: PreparedTile['cover'], shore: Shore | null): Mesh {
  const { x0, z0, cell, side } = ground
  const map = cover === undefined ? bakeTerrainTexture(tile, x0, z0, cell * (side - 1)) : cover ? coverTexture(cover) : null
  const size = cell * (side - 1)
  const wet = terrain.wet === undefined ? wetnessPixels(tile, x0, z0, size, terrain.relief.width) : terrain.wet
  const normalMap = reliefTexture(wet ? packMarsh(terrain.relief, wet) : terrain.relief)
  const stones = stonesTexture(tile.stones, shore, ground)
  const mesh = new Mesh(new Geometry(terrain.buffers), toonMaterial({ ...(map ? { map } : { color: GRASS_COLOR }), normalMap, normalMapTone: true, ramp: TERRAIN_RAMP, craters: true, snow: winter, marsh: wet !== null, stones }))
  mesh.userData.ownsMaterial = true
  mesh.userData.ground = ground
  mesh.userData.detail = tile.detail
  // The relief as `reliefPixels` made it, for the next restitch, and the marsh packed into what is drawn.
  mesh.userData.relief = terrain.relief
  mesh.userData.wet = wet
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
  if (!mesh?.material.normalMap) return
  shadeTerrain(group, reliefPixels(mesh.userData.ground as TileGround, around, mesh.userData.detail as Int8Array | undefined, mesh.userData.relief as Pixels))
}

/** Gives a tile's terrain a new relief map, as `restitchTerrain` or the tile worker made it. */
export function shadeTerrain(group: Group, relief: Pixels) {
  const mesh = terrainMesh(group)
  const texture = mesh?.material.normalMap
  if (!mesh || !texture) return
  const wet = mesh.userData.wet as Uint8Array | null
  mesh.userData.relief = relief
  texture.source = wet && wet.length === relief.width * relief.height ? packMarsh(relief, wet) : relief
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
 * buildings and bridges merged into one mesh with a colour a vertex, its trees, and its fences and
 * power lines. Water is drawn
 * apart (`buildWater`). Only wrapping arrays, so it is cheap enough for the frame.
 */
export function tileGroup(tile: Pick<TileData, 'cover' | 'detail' | 'trees' | 'lines' | 'stones' | 'water'>, ground: TileGround | null, prepared: PreparedTile): Group {
  const group = new Group()
  // Where the ground meets the water, for what is wet there.
  const shore = ground ? shoreOf(ground, tile.water) : null
  if (ground && prepared.terrain) group.add(buildTerrain(tile, ground, prepared.terrain, prepared.cover, shore))
  if (prepared.draped) group.add(...ribbonMeshes(prepared.draped.ribbons, groundMap(group)))
  group.add(...mergedMesh(prepared.buildings, new Set(prepared.undrawn)))
  group.add(...buildTrees(tile.trees, shore))
  group.add(...buildLines(tile.lines, ground))
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

/**
 * The tile's buildings (as cubes) and bridges as one mesh with a colour per vertex
 * (`buildingBuffers`). One draw per tile (and one shadow draw) however many colours it holds.
 */
function mergedMesh(buffers: BuildingBuffers | null, destroyed: ReadonlySet<number>): Mesh[] {
  if (!buffers) return []
  const out: Mesh[] = []
  if (buffers.positions.length > 0) {
    const { positions, normals, colors, custom, bounds } = buffers
    const mesh = new Mesh(new Geometry({ positions, normals, colors, custom, bounds }), buildingMaterial)
    mesh.castShadow = true
    mesh.receiveShadow = true
    mesh.userData.merged = true
    const merged: Merged = { ranges: buffers.ranges ?? null, left: new Set(destroyed) }
    mesh.userData.buildings = merged
    out.push(mesh)
  }
  if (buffers.glass) out.push(glassMesh(buffers.glass))
  return out
}

/** The windows' glass of a tile's buildings as one translucent mesh; `Glazing` breaks its panes in place. */
function glassMesh(glass: GlassBuffers): Mesh {
  const mesh = new Mesh(new Geometry({ positions: glass.positions, normals: glass.normals, bounds: glass.bounds }), glassMaterial)
  mesh.castShadow = false
  mesh.receiveShadow = true
  mesh.renderOrder = GLASS_ORDER
  mesh.userData.glass = true
  mesh.userData.panes = glass.panes
  return mesh
}

/** The glass mesh of a tile's group, if it has one. */
export function glassOf(group: Group): Mesh | null {
  return group.children.find((child): child is Mesh => child.userData.glass === true && child instanceof Mesh) ?? null
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
  // The glass stays as it is: its panes do not depend on which buildings are drawn here, and some are broken.
  const glass = glassOf(group)
  for (const mesh of mergedMesh(buildingBuffers(tile, salt, destroyed), destroyed)) {
    if (mesh.userData.glass && glass) mesh.geometry.dispose()
    else group.add(mesh)
  }
}

/** Frees a tile's geometry, its terrain texture and its own ribbon materials. Shared materials and tree prototypes stay. */
export function disposeTile(group: Group) {
  // A tile's own ribbon materials, freed once each though several meshes share one.
  const disposed = new Set<Material>()
  group.traverse((object: Node) => {
    if (!(object instanceof Mesh)) return
    // Tree prototypes are shared by every tile; an instanced mesh has no buffers of its own.
    if (!object.userData.sharedGeometry) object.geometry.dispose()
    if (object.userData.groundMaterial) disposed.add(object.material)
    if (object.userData.ownsMaterial) {
      const material = object.material
      material.map?.dispose()
      material.normalMap?.dispose()
      material.stones?.dispose()
      material.dispose()
    }
  })
  for (const material of disposed) material.dispose()
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
