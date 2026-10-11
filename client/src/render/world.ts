import * as THREE from 'three'
import type { TileGround } from '@/sim/terrain'
import { ROOF_FLAT, type TileData } from '@/world'
import { GRASS_COLOR, bakeTerrainTexture } from './cover'
import { buildRoads } from './roads'
import { heightSampler, normalAt, toneAt, type HeightSource } from './shade'
import { buildTrees } from './trees'

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
const TERRAIN_RAMP = (() => {
  const texture = new THREE.DataTexture(new Uint8Array([90, 100, 115, 140, 180, 208, 235, 255]), 8, 1, THREE.RedFormat)
  texture.minFilter = texture.magFilter = THREE.NearestFilter
  texture.needsUpdate = true
  return texture
})()

/** Materials are shared by every tile, so streaming one in or out never compiles a shader. */
const materials = new Map<string, THREE.Material>()
function material(kind: 'terrain' | 'building', color: number) {
  const key = `${kind}:${color}`
  let found = materials.get(key)
  if (!found) {
    found =
      kind === 'terrain'
        ? new THREE.MeshToonMaterial({ color, gradientMap: TERRAIN_RAMP, vertexColors: true })
        : new THREE.MeshToonMaterial({ color })
    materials.set(key, found)
  }
  return found
}

/**
 * The grass, as one plane under the whole world: the ground of a flat world, and on a hilly one
 * what shows through where a tile has not streamed in yet. `y` sits it just under the terrain.
 */
export function buildGround(halfExtent: number, y = 0): THREE.Mesh {
  const ground = new THREE.Mesh(new THREE.PlaneGeometry(halfExtent * 2, halfExtent * 2), new THREE.MeshLambertMaterial({ color: GRASS_COLOR }))
  ground.rotation.x = -Math.PI / 2
  ground.position.y = y
  ground.receiveShadow = true
  return ground
}

/**
 * One tile's terrain, as indexed triangles split the way the collider's are (each cell along its
 * `(x0, z1)`–`(x1, z0)` diagonal). UVs run 0..1 over the tile, for a texture baked per tile later.
 */
function buildTerrain(tile: Pick<TileData, 'cover'>, ground: TileGround, around?: HeightSource): THREE.Mesh {
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
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3))
  geometry.setAttribute('uv', new THREE.BufferAttribute(uvs, 2))
  geometry.setIndex(new THREE.BufferAttribute(indices, 1))
  const shading = terrainShading(ground, around)
  geometry.setAttribute('normal', new THREE.BufferAttribute(shading.normals, 3))
  geometry.setAttribute('color', new THREE.BufferAttribute(shading.colors, 3))
  geometry.computeBoundingSphere()
  // The cover is painted into this tile's own texture; the material is the tile's too.
  const map = bakeTerrainTexture(tile, x0, z0, cell * (side - 1))
  const mesh = new THREE.Mesh(geometry, map ? new THREE.MeshToonMaterial({ map, gradientMap: TERRAIN_RAMP, vertexColors: true }) : material('terrain', GRASS_COLOR))
  mesh.userData.ownsMaterial = map !== null
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
export function restitchTerrain(group: THREE.Group, around: HeightSource) {
  for (const child of group.children) {
    const ground = child.userData.ground as TileGround | undefined
    if (!ground || !(child instanceof THREE.Mesh)) continue
    const { normals, colors } = terrainShading(ground, around)
    for (const [name, values] of [['normal', normals], ['color', colors]] as const) {
      const attribute = child.geometry.getAttribute(name) as THREE.BufferAttribute
      attribute.array.set(values)
      attribute.needsUpdate = true
    }
  }
}

/**
 * One tile's terrain and buildings (extruded footprints), merged by colour so a tile is a handful
 * of draw calls. `salt` keeps colours stable between reloads and varied across tiles. Roads are
 * ribbons draped on the terrain; ground cover is painted into the terrain's texture.
 */
export function buildTile(tile: TileData, salt: number, ground: TileGround | null = null, around?: HeightSource, destroyed: ReadonlySet<number> = NONE): THREE.Group {
  const group = new THREE.Group()
  if (ground) {
    group.add(buildTerrain(tile, ground, around))
    const size = (ground.side - 1) * ground.cell
    group.add(...buildRoads(tile.roads, heightSampler(ground, around), ground.cell, [ground.x0, ground.z0, ground.x0 + size, ground.z0 + size], TERRAIN_RAMP))
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

/** The tile's buildings and bridges, merged by colour, leaving out the buildings in `destroyed`. */
function buildMerged(tile: TileData, salt: number, destroyed: ReadonlySet<number>): THREE.Mesh[] {
  const byColor = new Map<number, number[]>()
  const add = (color: number, from: number, count: number) => {
    const list = byColor.get(color) ?? []
    trianglePositions(tile, from, count, list)
    byColor.set(color, list)
  }
  tile.buildings.forEach((building, index) => {
    if (destroyed.has(index)) return
    const { wall, roof } = buildingColors(tile, index, salt)
    add(wall, building.first, building.walls)
    add(roof, building.first + building.walls, building.roof)
  })
  // Bridges: concrete deck, piers and parapets, then steel pylon and cables.
  for (const bridge of tile.bridges) {
    add(BRIDGE_DECK_COLOR, bridge.first, bridge.deck)
    add(BRIDGE_STEEL_COLOR, bridge.first + bridge.deck, bridge.steel)
  }
  const meshes: THREE.Mesh[] = []
  for (const [color, positions] of byColor) {
    const mesh = new THREE.Mesh(flatGeometry(positions), material('building', color))
    mesh.castShadow = true
    mesh.receiveShadow = true
    mesh.userData.merged = true
    meshes.push(mesh)
  }
  return meshes
}

/** Redraws a tile's buildings without those in `destroyed` (indices into `tile.buildings`). */
export function rebuildBuildings(group: THREE.Group, tile: TileData, salt: number, destroyed: ReadonlySet<number>) {
  const stale = group.children.filter((child): child is THREE.Mesh => child.userData.merged === true && child instanceof THREE.Mesh)
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
export function collapsingBuilding(tile: TileData, index: number, salt: number): THREE.Group {
  const building = tile.buildings[index]!
  const { wall, roof } = buildingColors(tile, index, salt)
  const group = new THREE.Group()
  group.position.y = building.base
  for (const [color, from, count] of [[wall, building.first, building.walls], [roof, building.first + building.walls, building.roof]] as const) {
    const mesh = new THREE.Mesh(flatGeometry(trianglePositions(tile, from, count, [], building.base)), material('building', color))
    mesh.castShadow = true
    group.add(mesh)
  }
  return group
}

/** Frees a tile's geometry and its terrain texture. The materials and tree prototypes are shared and stay. */
export function disposeTile(group: THREE.Group) {
  group.traverse((object) => {
    if (!(object instanceof THREE.Mesh)) return
    // Tree prototypes are shared by every tile; an instanced mesh frees only its own buffers.
    if (!object.userData.sharedGeometry) object.geometry.dispose()
    if (object instanceof THREE.InstancedMesh) object.dispose()
    if (object.userData.ownsMaterial) {
      const material = object.material as THREE.MeshLambertMaterial
      material.map?.dispose()
      material.dispose()
    }
  })
  group.removeFromParent()
}

/** A non-indexed geometry from triangle-soup positions, with one flat normal per face. */
function flatGeometry(positions: number[]): THREE.BufferGeometry {
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
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.BufferAttribute(array, 3))
  geometry.setAttribute('normal', new THREE.BufferAttribute(normals, 3))
  geometry.computeBoundingSphere()
  return geometry
}
