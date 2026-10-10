import * as THREE from 'three'
import type { TileGround } from '@/sim/terrain'
import type { TileData } from '@/world'
import { GRASS_COLOR, bakeTerrainTexture } from './cover'

/** Candy-coloured buildings. Picked per building so the palette stays stable between reloads. */
const BUILDING_COLORS = [0xffd6a5, 0xfdffb6, 0xcaffbf, 0x9bf6ff, 0xa0c4ff, 0xbdb2ff, 0xffc6ff, 0xffadad]

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

  // Buildings: extrude each footprint upwards. Shape space is (x, -z) so the extrusion along +Z
  // becomes +Y after rotating the geometry -90° about X.
  const byColor = new Map<number, THREE.BufferGeometry[]>()
  tile.buildings.forEach((building, index) => {
    const shape = new THREE.Shape(building.footprint.map(([x, z]) => new THREE.Vector2(x, -z)))
    const geometry = new THREE.ExtrudeGeometry(shape, { depth: building.height, bevelEnabled: false })
    geometry.rotateX(-Math.PI / 2)
    geometry.translate(0, building.base, 0)
    const color = BUILDING_COLORS[(index + salt) % BUILDING_COLORS.length]!
    const list = byColor.get(color) ?? []
    list.push(geometry)
    byColor.set(color, list)
  })
  for (const [color, geometries] of byColor) {
    const mesh = new THREE.Mesh(mergeGeometries(geometries), material('building', color))
    mesh.castShadow = true
    mesh.receiveShadow = true
    group.add(mesh)
  }
  return group
}

/** Frees a tile's geometry and its terrain texture. The other materials are shared and stay. */
export function disposeTile(group: THREE.Group) {
  group.traverse((object) => {
    if (!(object instanceof THREE.Mesh)) return
    object.geometry.dispose()
    if (object.userData.ownsMaterial) {
      const material = object.material as THREE.MeshLambertMaterial
      material.map?.dispose()
      material.dispose()
    }
  })
  group.removeFromParent()
}

/** Concatenates non-indexed copies of geometries that share attributes (position, normal, uv). */
function mergeGeometries(geometries: THREE.BufferGeometry[]): THREE.BufferGeometry {
  const parts = geometries.map((g) => (g.index ? g.toNonIndexed() : g))
  const merged = new THREE.BufferGeometry()
  for (const name of ['position', 'normal'] as const) {
    const total = parts.reduce((sum, g) => sum + g.getAttribute(name).array.length, 0)
    const array = new Float32Array(total)
    let offset = 0
    for (const g of parts) {
      const source = g.getAttribute(name).array
      array.set(source, offset)
      offset += source.length
    }
    merged.setAttribute(name, new THREE.BufferAttribute(array, 3))
  }
  for (const g of geometries) g.dispose()
  for (const g of parts) g.dispose()
  merged.computeBoundingSphere()
  return merged
}
