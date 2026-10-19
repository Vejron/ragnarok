import { Geometry, Group, Material, Mesh, Ramp, planeGeometry, type Node } from '@ragnarok/engine'
import * as THREE from 'three'
import { toonMaterial } from './toon'
import type { TileGround } from '@/sim/terrain'
import { ROOF_FLAT, type TileData } from '@/world'
import { shellOf } from '@/sim/carve'
import { GRASS_COLOR, bakeNormalMap, bakeTerrainTexture } from './cover'
import { buildRails } from './rails'
import { buildRoads } from './roads'
import { heightSampler, normalAt, toneAt, type HeightSource } from './shade'
import { buildTrees } from './trees'
import { voxelFaces, type Soup } from './voxels'


export type { HeightSource }

/** Candy-coloured buildings. Picked per building so the palette stays stable between reloads. */
const BRIDGE_DECK_COLOR = 0xb8b6ae
const BRIDGE_STEEL_COLOR = 0x6d7a86
const BUILDING_COLORS = [0xffd6a5, 0xfdffb6, 0xcaffbf, 0x9bf6ff, 0xa0c4ff, 0xbdb2ff, 0xffc6ff, 0xffadad]

/** Roofs: slate, tile and timber for pitched ones, concrete grey for flat ones. */
const ROOF_COLORS = [0xb5543f, 0x6d7a8c, 0x8d6e63, 0x4f5d6b, 0x9c4a3c]
const FLAT_ROOF_COLORS = [0xb0b4ba, 0x9aa0a6, 0xc4c7cc]

/**
 * The terrain's toon ramp: eight steps over three's half-Lambert coordinate (`N·L * 0.5 + 0.5`),
 * dark and coarse in shade and fine near the light. Flat ground under the sun (about 0.83) sits
 * in the middle of step 6, so a few degrees of slope tip it one step either way instead of flickering.
 */
const TERRAIN_RAMP = new Ramp(new Uint8Array([90, 100, 115, 140, 180, 208, 235, 255]))

/** Every tile's buildings share this one; each vertex carries its colour. */
const buildingMaterial = toonMaterial({ vertexColors: true })

/** Materials are shared by every tile, so streaming one in or out never compiles a shader. */
const materials = new Map<string, Material>()
function material(kind: 'terrain' | 'building', color: number) {
  const key = `${kind}:${color}`
  let found = materials.get(key)
  if (!found) {
    found =
      kind === 'terrain'
        ? toonMaterial({ color, ramp: TERRAIN_RAMP, vertexColors: true })
        : toonMaterial({ color })
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
 * One tile's terrain, as indexed triangles split the way the collider's are (each cell along its
 * `(x0, z1)`–`(x1, z0)` diagonal). UVs run 0..1 over the tile, for a texture baked per tile later.
 */
function buildTerrain(tile: Pick<TileData, 'cover' | 'detail'>, ground: TileGround, around?: HeightSource): Mesh {
  const { x0, z0, cell, side, heights } = ground
  const positions = new Float32Array(side * side * 3)
  const uvs = new Float32Array(side * side * 2)
  for (let row = 0; row < side; row++) {
    for (let column = 0; column < side; column++) {
      const at = row * side + column
      positions.set([x0 + column * cell, heights[at]!, z0 + row * cell], at * 3)
      uvs.set([column / (side - 1), row / (side - 1)], at * 2)
    }
  }
  const indices = new Uint32Array((side - 1) * (side - 1) * 6)
  let n = 0
  for (let row = 0; row < side - 1; row++) {
    for (let column = 0; column < side - 1; column++) {
      const p00 = row * side + column
      const p01 = p00 + 1 // (x1, z0)
      const p10 = p00 + side // (x0, z1)
      const p11 = p10 + 1
      // Counter-clockwise seen from above (+Y), so the faces point up.
      indices.set([p00, p10, p01, p10, p11, p01], n)
      n += 6
    }
  }
  const shading = terrainShading(ground, around)
  const geometry = new Geometry({ positions, uvs, normals: shading.normals, colors: shading.colors, indices })
  // The cover is painted into this tile's own texture; the material is the tile's too.
  const map = bakeTerrainTexture(tile, x0, z0, cell * (side - 1))
  // Finer relief than the mesh has, as lighting only.
  const normalMap = bakeNormalMap(tile.detail)
  const owned = map !== null || normalMap !== null
  const mesh = new Mesh(
    geometry,
    owned
      ? toonMaterial({
          ...(map ? { map } : { color: GRASS_COLOR }),
          ...(normalMap ? { normalMap } : {}),
          ramp: TERRAIN_RAMP,
          vertexColors: true,
        })
      : material('terrain', GRASS_COLOR),
  )
  mesh.userData.ownsMaterial = owned
  mesh.userData.ground = ground
  mesh.receiveShadow = true
  return mesh
}

/**
 * Smooth per-vertex normals and the baked tone (`toneAt`). Past the tile's edge the height comes
 * from `around` (the neighbouring tile), so both sides of a seam agree; where there is no
 * neighbour the edge height stands in, which leans the normal one-sidedly.
 */
function terrainShading(ground: TileGround, around?: HeightSource) {
  const { x0, z0, cell, side } = ground
  const height = heightSampler(ground, around)
  const normals = new Float32Array(side * side * 3)
  const colors = new Float32Array(side * side * 3)
  for (let row = 0; row < side; row++) {
    for (let column = 0; column < side; column++) {
      const [x, z] = [x0 + column * cell, z0 + row * cell]
      const normal = normalAt(height, x, z, cell)
      const tone = toneAt(height, x, z, normal)
      const at = (row * side + column) * 3
      normals.set(normal, at)
      colors.set([tone, tone, tone], at)
    }
  }
  return { normals, colors }
}

/** Recomputes a terrain mesh's normals once a neighbouring tile has arrived, so the seam closes. */
export function restitchTerrain(group: Group, around: HeightSource) {
  for (const child of group.children) {
    const ground = child.userData.ground as TileGround | undefined
    if (!ground || !(child instanceof Mesh)) continue
    const { normals, colors } = terrainShading(ground, around)
    child.geometry.normals!.set(normals)
    child.geometry.colors!.set(colors)
    // Only the shading changed, not the shape, so the vertices go up again but the bounds stand.
    child.geometry.needsUpdateRange(0, child.geometry.vertexCount)
  }
}

/**
 * One tile's terrain and buildings (extruded footprints), merged by colour so a tile is a handful
 * of draw calls. `salt` keeps colours stable between reloads and varied across tiles. Roads and
 * tracks are ribbons draped on the terrain; ground cover is painted into the terrain's texture.
 */
export function buildTile(tile: TileData, salt: number, ground: TileGround | null = null, around?: HeightSource, destroyed: ReadonlySet<number> = NONE): Group {
  const group = new Group()
  if (ground) {
    group.add(buildTerrain(tile, ground, around))
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

/**
 * Triangle soup for `count` triangles of the tile's collision mesh from `from`: the importer's
 * own triangles, so what is drawn is what is collided with. The importer winds footprints
 * counter-clockwise in (x, z); seen from +Y that is clockwise, so they are reversed to face
 * outwards and up. `lower` is taken off every height.
 */
function trianglePositions(tile: TileData, from: number, count: number, out: number[], lower = 0): number[] {
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
function buildingVoxels(tile: TileData, index: number, salt: number, out: Soup, offset?: readonly [number, number, number]): boolean {
  const shell = shellOf(tile, index)
  if (!shell) return false
  const { wall, roof } = buildingColors(tile, index, salt)
  voxelFaces(shell, (id) => shell.isSolid(id), (id) => (shell.isRoof(id) ? roof : wall), out, offset)
  return true
}

/**
 * The tile's buildings (as cubes) and bridges as one mesh with a colour per vertex, leaving out the
 * buildings in `destroyed`. One draw per tile (and one shadow draw) however many colours it holds.
 */
function buildMerged(tile: TileData, salt: number, destroyed: ReadonlySet<number>): Mesh[] {
  const positions: number[] = []
  const colors: number[] = []
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
  const soup: Soup = { positions: [], colors: [] }
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
function flatGeometry(positions: number[], colors?: number[]): Geometry {
  const array = Float32Array.from(positions)
  const normals = new Float32Array(array.length)
  const [a, b, c, n] = [new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3()]
  for (let i = 0; i < array.length; i += 9) {
    a.fromArray(array, i)
    b.fromArray(array, i + 3)
    c.fromArray(array, i + 6)
    n.subVectors(c, b).cross(a.sub(b)).normalize()
    for (let k = 0; k < 3; k++) n.toArray(normals, i + k * 3)
  }
  return new Geometry({ positions: array, normals, colors: colors ? Float32Array.from(colors) : undefined })
}
