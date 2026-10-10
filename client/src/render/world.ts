import * as THREE from 'three'
import type { TileGround } from '@/sim/terrain'
import { ROOF_FLAT, type TileData } from '@/world'
import { GRASS_COLOR, bakeTerrainTexture } from './cover'
import { buildTrees } from './trees'

/** Candy-coloured buildings. Picked per building so the palette stays stable between reloads. */
const BUILDING_COLORS = [0xffd6a5, 0xfdffb6, 0xcaffbf, 0x9bf6ff, 0xa0c4ff, 0xbdb2ff, 0xffc6ff, 0xffadad]

/** Roofs: slate, tile and timber for pitched ones, concrete grey for flat ones. */
const ROOF_COLORS = [0xb5543f, 0x6d7a8c, 0x8d6e63, 0x4f5d6b, 0x9c4a3c]
const FLAT_ROOF_COLORS = [0xb0b4ba, 0x9aa0a6, 0xc4c7cc]

/** Materials are shared by every tile, so streaming one in or out never compiles a shader. */
const materials = new Map<string, THREE.Material>()
function material(kind: 'terrain' | 'building', color: number) {
  const key = `${kind}:${color}`
  let found = materials.get(key)
  if (!found) {
    found =
      kind === 'terrain'
        ? new THREE.MeshLambertMaterial({ color, flatShading: true })
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
function buildTerrain(tile: Pick<TileData, 'cover' | 'roads'>, { x0, z0, cell, side, heights }: TileGround): THREE.Mesh {
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
  geometry.computeVertexNormals()
  geometry.computeBoundingSphere()
  // The cover and roads are painted into this tile's own texture; the material is the tile's too.
  const map = bakeTerrainTexture(tile, x0, z0, cell * (side - 1))
  const mesh = new THREE.Mesh(geometry, map ? new THREE.MeshLambertMaterial({ map, flatShading: true }) : material('terrain', GRASS_COLOR))
  mesh.userData.ownsMaterial = map !== null
  mesh.receiveShadow = true
  return mesh
}

/**
 * One tile's terrain and buildings (extruded footprints), merged by colour so a tile is a handful
 * of draw calls. `salt` keeps colours stable between reloads and varied across tiles. Roads and
 * ground cover are not meshes: they are painted into the terrain's texture.
 */
export function buildTile(tile: TileData, salt: number, ground: TileGround | null = null): THREE.Group {
  const group = new THREE.Group()
  if (ground) group.add(buildTerrain(tile, ground))

  // Buildings: the importer's own triangles (walls, then roof) out of the collision mesh, so what
  // is drawn is what is collided with. Walls and roofs are merged by colour.
  const byColor = new Map<number, number[]>()
  const { vertices, triangles } = tile.collision
  const add = (color: number, from: number, count: number) => {
    const list = byColor.get(color) ?? []
    for (let t = from; t < from + count; t++) {
      // The importer winds footprints counter-clockwise in (x, z); seen from +Y that is clockwise, so
      // its triangles are reversed to face outwards and up.
      for (const k of [0, 2, 1]) {
        const v = triangles[t * 3 + k]! * 3
        list.push(vertices[v]!, vertices[v + 1]!, vertices[v + 2]!)
      }
    }
    byColor.set(color, list)
  }
  tile.buildings.forEach((building, index) => {
    add(BUILDING_COLORS[(index + salt) % BUILDING_COLORS.length]!, building.first, building.walls)
    const palette = building.roofKind === ROOF_FLAT ? FLAT_ROOF_COLORS : ROOF_COLORS
    add(palette[(index + salt) % palette.length]!, building.first + building.walls, building.roof)
  })
  for (const [color, positions] of byColor) {
    const mesh = new THREE.Mesh(flatGeometry(positions), material('building', color))
    mesh.castShadow = true
    mesh.receiveShadow = true
    group.add(mesh)
  }
  group.add(...buildTrees(tile.trees))
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
