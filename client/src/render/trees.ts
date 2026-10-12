import * as THREE from 'three'
import { mergeGeometries, mergeVertices } from 'three/addons/utils/BufferGeometryUtils.js'
import {
  TREE_BIRCH,
  TREE_BROADLEAF,
  TREE_BUSH,
  TREE_COLUMNAR,
  TREE_LOWWIDE,
  TREE_ORNAMENTAL,
  TREE_PINE,
  TREE_ROCK_BOULDER,
  TREE_ROCK_SLAB,
  TREE_SPRUCE,
  type Tree,
} from '@/world'

/**
 * The tree archetypes. Each prototype is built in unit space (foot at the origin, `y` 0..1 over the
 * tree's height, crown radius 1) and placed per tree by scaling `(crown, height, crown)`, so the
 * trunk radii here are the real ones divided by the archetype's crown radius.
 */
const TRUNK_BARK = 0x8a5a3c
const TRUNK_BIRCH = 0xe9e5d9

interface Part {
  /** `ellipsoid`: a low-poly sphere scaled by `(rx, ry, rz)`; `cone` and `trunk` span `y0..y1`. */
  shape: 'ellipsoid' | 'cone' | 'trunk'
  color: number
  y0: number
  y1?: number
  /** Ellipsoid radii in unit space; radius of the cone's base or the trunk. */
  rx?: number
  ry?: number
  x?: number
  z?: number
}

const PROTOTYPES: Record<number, Part[]> = {
  [TREE_BIRCH]: [
    { shape: 'trunk', color: TRUNK_BIRCH, y0: 0, y1: 0.7, rx: 0.07 },
    { shape: 'ellipsoid', color: 0x9bd36b, y0: 0.64, rx: 1, ry: 0.38 },
  ],
  [TREE_PINE]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.85, rx: 0.08 },
    { shape: 'ellipsoid', color: 0x4f8f4a, y0: 0.74, rx: 0.9, ry: 0.12 },
    { shape: 'ellipsoid', color: 0x5a9a50, y0: 0.9, rx: 0.6, ry: 0.1, x: 0.15 },
  ],
  [TREE_SPRUCE]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.2, rx: 0.09 },
    { shape: 'cone', color: 0x2f6b4a, y0: 0.12, y1: 0.5, rx: 1 },
    { shape: 'cone', color: 0x2f6b4a, y0: 0.4, y1: 0.75, rx: 0.75 },
    { shape: 'cone', color: 0x35764f, y0: 0.65, y1: 1, rx: 0.5 },
  ],
  [TREE_ORNAMENTAL]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.55, rx: 0.05 },
    { shape: 'ellipsoid', color: 0xa7d65c, y0: 0.68, rx: 1, ry: 0.32 },
  ],
  [TREE_BROADLEAF]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.55, rx: 0.06 },
    { shape: 'ellipsoid', color: 0x4fae4a, y0: 0.68, rx: 1, ry: 0.32 },
    { shape: 'ellipsoid', color: 0x5cbc54, y0: 0.82, rx: 0.65, ry: 0.2, x: 0.2, z: 0.1 },
  ],
  [TREE_COLUMNAR]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.3, rx: 0.13 },
    { shape: 'ellipsoid', color: 0x6fbf5a, y0: 0.6, rx: 1, ry: 0.4 },
  ],
  [TREE_LOWWIDE]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.35, rx: 0.045 },
    { shape: 'ellipsoid', color: 0x9bbf7a, y0: 0.58, rx: 1, ry: 0.45 },
  ],
  // Rocks are scaled by (radius, height, radius) like a crown, and sunk a little into the ground.
  [TREE_ROCK_BOULDER]: [
    { shape: 'ellipsoid', color: 0x8d8f91, y0: 0.4, rx: 1, ry: 0.62 },
    { shape: 'ellipsoid', color: 0x7b7d80, y0: 0.22, rx: 0.55, ry: 0.4, x: 0.62, z: 0.25 },
  ],
  [TREE_ROCK_SLAB]: [
    { shape: 'ellipsoid', color: 0x9a9b97, y0: 0.3, rx: 1, ry: 0.42 },
    { shape: 'ellipsoid', color: 0x85878a, y0: 0.5, rx: 0.6, ry: 0.4, x: -0.2, z: -0.1 },
  ],
  [TREE_BUSH]: [
    { shape: 'ellipsoid', color: 0x3f8f45, y0: 0.4, rx: 1, ry: 0.45 },
    { shape: 'ellipsoid', color: 0x4fa352, y0: 0.62, rx: 0.65, ry: 0.35, x: 0.15, z: -0.1 },
  ],
}

function partGeometry(part: Part): THREE.BufferGeometry {
  let geometry: THREE.BufferGeometry
  if (part.shape === 'ellipsoid') {
    // Fine enough that the silhouette reads round; welded so the normals blend across faces.
    geometry = new THREE.IcosahedronGeometry(1, 3)
    geometry.deleteAttribute('uv')
    geometry.deleteAttribute('normal')
    geometry = mergeVertices(geometry, 1e-4)
    geometry.scale(part.rx!, part.ry!, part.rx!)
    geometry.translate(part.x ?? 0, part.y0, part.z ?? 0)
    geometry.computeVertexNormals()
  } else {
    const height = part.y1! - part.y0
    geometry =
      part.shape === 'cone'
        ? new THREE.ConeGeometry(part.rx!, height, 20, 1)
        : new THREE.CylinderGeometry(part.rx! * 0.8, part.rx!, height, 10)
    geometry.translate(0, part.y0 + height / 2, 0)
    geometry.deleteAttribute('uv')
  }
  // Merged parts must share one attribute layout: indexed, position + normal + colour.
  const color = new THREE.Color(part.color)
  const count = geometry.getAttribute('position').count
  const colors = new Float32Array(count * 3)
  for (let i = 0; i < count; i++) color.toArray(colors, i * 3)
  geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3))
  return geometry
}

/** A soft stepped ramp (shade is N·L*0.5+0.5): a few bands with a lit crown, not a hard two-tone split. */
const CROWN_RAMP = (() => {
  const steps = [120, 150, 185, 220, 245, 255]
  const texture = new THREE.DataTexture(new Uint8Array(steps), steps.length, 1, THREE.RedFormat)
  texture.minFilter = texture.magFilter = THREE.LinearFilter
  texture.needsUpdate = true
  return texture
})()

let shared: { geometries: Map<number, THREE.BufferGeometry>; material: THREE.Material } | undefined

function prototypes() {
  if (!shared) {
    const geometries = new Map<number, THREE.BufferGeometry>()
    for (const [kind, parts] of Object.entries(PROTOTYPES)) {
      geometries.set(Number(kind), mergeGeometries(parts.map(partGeometry))!)
    }
    shared = { geometries, material: new THREE.MeshToonMaterial({ vertexColors: true, gradientMap: CROWN_RAMP }) }
  }
  return shared
}

/** A turn about the vertical, from the position, so neighbours of one kind do not all look alike. */
function spin(tree: Tree): number {
  return (Math.abs(Math.sin(tree.x * 12.9898 + tree.z * 78.233)) * 43758.5453) % (Math.PI * 2)
}

/**
 * One standalone mesh of `tree`, in a pivot at its foot, for a tree that is no longer an
 * instance: the pivot is where it turns as it falls. Geometry and material are the shared ones.
 */
export function standaloneTree(tree: Tree): { pivot: THREE.Group; mesh: THREE.Mesh } {
  const { geometries, material } = prototypes()
  const mesh = new THREE.Mesh(geometries.get(tree.kind) ?? geometries.get(TREE_BIRCH)!, material)
  mesh.rotation.y = spin(tree)
  mesh.scale.set(tree.crown, tree.height, tree.crown)
  mesh.castShadow = true
  mesh.userData.sharedGeometry = true
  const pivot = new THREE.Group()
  pivot.position.set(tree.x, tree.base, tree.z)
  pivot.add(mesh)
  return { pivot, mesh }
}

/**
 * Takes tree number `index` of its tile (the `Destroyed` index) out of the tile's instanced
 * meshes, by shrinking it to nothing. False if it is not there, or already gone.
 */
export function removeTreeInstance(group: THREE.Object3D, index: number): boolean {
  for (const child of group.children) {
    const slots = child.userData.treeSlots as Map<number, number> | undefined
    const slot = slots?.get(index)
    if (slot === undefined || !(child instanceof THREE.InstancedMesh)) continue
    slots!.delete(index)
    child.setMatrixAt(slot, new THREE.Matrix4().makeScale(0, 0, 0))
    child.instanceMatrix.needsUpdate = true
    return true
  }
  return false
}

/**
 * One tile's trees: one instanced mesh per archetype present, so at most ten draw calls however
 * many trees there are. The geometry and material are shared by all tiles (`disposeTile` leaves
 * them), only the instance buffers belong to the tile.
 */
export function buildTrees(trees: readonly Tree[]): THREE.InstancedMesh[] {
  if (trees.length === 0) return []
  const { geometries, material } = prototypes()
  const byKind = new Map<number, { tree: Tree; index: number }[]>()
  trees.forEach((tree, index) => {
    const list = byKind.get(tree.kind) ?? []
    list.push({ tree, index })
    byKind.set(tree.kind, list)
  })
  const matrix = new THREE.Matrix4()
  const position = new THREE.Vector3()
  const scale = new THREE.Vector3()
  const rotation = new THREE.Quaternion()
  const meshes: THREE.InstancedMesh[] = []
  for (const [kind, list] of byKind) {
    const geometry = geometries.get(kind) ?? geometries.get(TREE_BIRCH)!
    const mesh = new THREE.InstancedMesh(geometry, material, list.length)
    // Which instance is which tree of the tile, so one can be felled.
    const slots = new Map<number, number>()
    mesh.userData.treeSlots = slots
    list.forEach(({ tree, index }, i) => {
      slots.set(index, i)
      rotation.setFromAxisAngle(THREE.Object3D.DEFAULT_UP, spin(tree))
      position.set(tree.x, tree.base, tree.z)
      scale.set(tree.crown, tree.height, tree.crown)
      mesh.setMatrixAt(i, matrix.compose(position, rotation, scale))
    })
    mesh.instanceMatrix.needsUpdate = true
    mesh.castShadow = true
    mesh.receiveShadow = false
    mesh.userData.sharedGeometry = true
    meshes.push(mesh)
  }
  return meshes
}
