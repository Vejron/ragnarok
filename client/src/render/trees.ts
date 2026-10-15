import { Color, Geometry, Group, InstancedMesh, Mesh, type Material, type Node, cylinderGeometry, icosphereGeometry, mat4 } from '@ragnarok/engine'
import { toonMaterial } from './toon'
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
  /**
   * `ellipsoid`: a low-poly sphere scaled by `(rx, ry, rz)`; `cone` and `trunk` span `y0..y1`;
   * `clumps`: `count` small ellipsoids scattered through the ellipsoid `(rx, ry)` about `y0`, which
   * leaves gaps to see through; `limb`: a slim branch of `length` from `(x, y0, z)`, `tilt` off
   * vertical, towards `yaw`.
   */
  shape: 'ellipsoid' | 'cone' | 'trunk' | 'clumps' | 'limb'
  color: number
  y0: number
  y1?: number
  /** Ellipsoid radii in unit space; radius of the cone's base or the trunk. */
  rx?: number
  ry?: number
  x?: number
  z?: number
  count?: number
  /** Clump radius in crown radii. */
  clump?: number
  /** A second shade the clumps alternate to. */
  color2?: number
  seed?: number
  length?: number
  tilt?: number
  yaw?: number
  /** Sides of a cone. */
  sides?: number
}

/** A crown of loose clumps with a limb or two under it: see-through enough to spot someone in the gaps. */
function crown(y0: number, rx: number, ry: number, count: number, clump: number, color: number, color2: number, seed: number): Part[] {
  return [{ shape: 'clumps', color, color2, y0, rx, ry, count, clump, seed }]
}

/** `n` limbs fanning out of the trunk from `y0`, each reaching `reach` crown radii out. */
function limbs(n: number, y0: number, length: number, tilt: number, radius: number, color: number, seed = 1): Part[] {
  return Array.from({ length: n }, (_, i) => ({
    shape: 'limb' as const,
    color,
    y0: y0 + (i % 3) * length * 0.12,
    rx: radius,
    length: length * (0.8 + 0.2 * ((i * 7 + seed) % 3) / 2),
    tilt: tilt + ((i * 5 + seed) % 3) * 0.12,
    yaw: (i / n) * Math.PI * 2 + seed,
  }))
}

/** A tier of conifer: a cone of many sides, so its rim is rounder and the tiers read apart. */
function tier(y0: number, y1: number, rx: number, color: number): Part {
  return { shape: 'cone', color, y0, y1, rx, sides: 9 }
}

const PROTOTYPES: Record<number, Part[]> = {
  [TREE_BIRCH]: [
    { shape: 'trunk', color: TRUNK_BIRCH, y0: 0, y1: 0.78, rx: 0.07 },
    ...limbs(4, 0.48, 0.3, 0.7, 0.03, TRUNK_BIRCH, 1),
    ...crown(0.62, 1, 0.4, 9, 0.46, 0x9bd36b, 0x86c45a, 11),
  ],
  [TREE_PINE]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.95, rx: 0.07 },
    ...limbs(5, 0.55, 0.38, 1.0, 0.025, TRUNK_BARK, 2),
    ...crown(0.82, 0.95, 0.2, 7, 0.4, 0x4f8f4a, 0x5a9a50, 23),
  ],
  [TREE_SPRUCE]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.4, rx: 0.07 },
    tier(0.1, 0.34, 1, 0x2f6b4a),
    tier(0.26, 0.5, 0.82, 0x2f6b4a),
    tier(0.42, 0.66, 0.64, 0x35764f),
    tier(0.58, 0.82, 0.44, 0x35764f),
    tier(0.74, 1, 0.26, 0x3a7d54),
  ],
  [TREE_ORNAMENTAL]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.6, rx: 0.05 },
    ...limbs(4, 0.4, 0.3, 0.8, 0.025, TRUNK_BARK, 3),
    ...crown(0.7, 1, 0.34, 8, 0.48, 0xa7d65c, 0x92c44c, 37),
  ],
  [TREE_BROADLEAF]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.6, rx: 0.06 },
    ...limbs(5, 0.4, 0.34, 0.8, 0.03, TRUNK_BARK, 4),
    ...crown(0.72, 1, 0.34, 11, 0.5, 0x4fae4a, 0x5cbc54, 41),
  ],
  [TREE_COLUMNAR]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.35, rx: 0.12 },
    ...crown(0.62, 0.9, 0.42, 9, 0.5, 0x6fbf5a, 0x5fae4e, 53),
  ],
  [TREE_LOWWIDE]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.4, rx: 0.045 },
    ...limbs(5, 0.22, 0.3, 1.0, 0.025, TRUNK_BARK, 5),
    ...crown(0.58, 1, 0.45, 10, 0.5, 0x9bbf7a, 0x88ad69, 61),
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
  [TREE_BUSH]: crown(0.42, 0.85, 0.35, 6, 0.5, 0x3f8f45, 0x4fa352, 71),
}

/** A deterministic 0..1 sequence, so a re-import or a reload draws the same tree. */
function sequence(seed: number) {
  let state = (seed * 2654435761) >>> 0
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0
    return state / 4294967296
  }
}

function paint(geometry: Geometry, color: number): Geometry {
  // Indexed and welded, so the parts merge and each vertex is shared: position + normal + colour.
  geometry.uvs = undefined
  geometry.computeNormals()
  const c = Color.hex(color)
  const count = geometry.vertexCount
  const colors = new Float32Array(count * 3)
  for (let i = 0; i < count; i++) colors.set([c.r, c.g, c.b], i * 3)
  geometry.colors = colors
  return geometry
}

function partGeometries(part: Part): Geometry[] {
  if (part.shape === 'clumps') {
    const next = sequence(part.seed ?? 1)
    const out: Geometry[] = []
    const r = part.clump ?? 0.5
    const flat = part.ry! / part.rx!
    for (let i = 0; i < part.count!; i++) {
      // Inside the ellipsoid, a little biased to the rim so the middle is hollow enough to see into.
      const angle = next() * Math.PI * 2
      const distance = Math.sqrt(0.25 + 0.75 * next()) * (1 - r * 0.5)
      const rise = (next() * 2 - 1) * (1 - distance * distance) ** 0.5
      const size = r * (0.7 + 0.5 * next())
      const clump = icosphereGeometry(1, 1)
        .scale(size, size * flat * 1.3, size)
        .translate(Math.cos(angle) * distance * part.rx!, part.y0 + rise * part.ry!, Math.sin(angle) * distance * part.rx!)
      out.push(paint(clump, i % 2 ? (part.color2 ?? part.color) : part.color))
    }
    return out
  }
  if (part.shape === 'limb') {
    const length = part.length!
    const limb = cylinderGeometry(part.rx! * 0.5, part.rx!, length, 5)
      .translate(0, length / 2, 0)
      .rotateZ(-(part.tilt ?? 0.8))
      .rotateY(part.yaw ?? 0)
      .translate(part.x ?? 0, part.y0, part.z ?? 0)
    return [paint(limb, part.color)]
  }
  let geometry: Geometry
  if (part.shape === 'ellipsoid') {
    // A low-poly sphere: 42 welded vertices and 80 triangles, so the crown reads as a few bold shapes.
    geometry = icosphereGeometry(1, 1).scale(part.rx!, part.ry!, part.rx!).translate(part.x ?? 0, part.y0, part.z ?? 0)
  } else {
    const height = part.y1! - part.y0
    // A cone is a cylinder that closes to a point at the top.
    geometry = (part.shape === 'cone' ? cylinderGeometry(0, part.rx!, height, part.sides ?? 7) : cylinderGeometry(part.rx! * 0.8, part.rx!, height, 6)).translate(0, part.y0 + height / 2, 0)
  }
  return [paint(geometry, part.color)]
}

let shared: { geometries: Map<number, Geometry>; material: Material } | undefined

function prototypes() {
  if (!shared) {
    const geometries = new Map<number, Geometry>()
    for (const [kind, parts] of Object.entries(PROTOTYPES)) {
      geometries.set(Number(kind), Geometry.merge(parts.flatMap(partGeometries)))
    }
    shared = { geometries, material: toonMaterial({ vertexColors: true, nearFade: 2.5 }) }
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
export function standaloneTree(tree: Tree): { pivot: Group; mesh: Mesh } {
  const { geometries, material } = prototypes()
  const mesh = new Mesh(geometries.get(tree.kind) ?? geometries.get(TREE_BIRCH)!, material)
  mesh.quaternion.setFromAxisAngle(0, 1, 0, spin(tree))
  mesh.scale.set(tree.crown, tree.height, tree.crown)
  mesh.castShadow = true
  mesh.userData.sharedGeometry = true
  const pivot = new Group()
  pivot.position.set(tree.x, tree.base, tree.z)
  pivot.add(mesh)
  return { pivot, mesh }
}

/**
 * Takes tree number `index` of its tile (the `Destroyed` index) out of the tile's instanced
 * meshes, by shrinking it to nothing. False if it is not there, or already gone.
 */
export function removeTreeInstance(group: Node, index: number): boolean {
  for (const child of group.children) {
    const slots = child.userData.treeSlots as Map<number, number> | undefined
    const slot = slots?.get(index)
    if (slot === undefined || !(child instanceof InstancedMesh)) continue
    slots!.delete(index)
    // Scaled to nothing where it stood, so the mesh's bounds do not stretch to the origin.
    const at = child.matrices
    child.setMatrixAt(slot, mat4.compose(mat4.create(), at[slot * 16 + 12]!, at[slot * 16 + 13]!, at[slot * 16 + 14]!, 0, 0, 0, 1, 0, 0, 0))
    child.needsUpdate()
    return true
  }
  return false
}

/**
 * One tile's trees: one instanced mesh per archetype present, so at most ten draw calls however
 * many trees there are. The geometry and material are shared by all tiles (`disposeTile` leaves
 * them), only the instance buffers belong to the tile.
 */
export function buildTrees(trees: readonly Tree[]): InstancedMesh[] {
  if (trees.length === 0) return []
  const { geometries, material } = prototypes()
  const byKind = new Map<number, { tree: Tree; index: number }[]>()
  trees.forEach((tree, index) => {
    const list = byKind.get(tree.kind) ?? []
    list.push({ tree, index })
    byKind.set(tree.kind, list)
  })
  const matrix = mat4.create()
  const meshes: InstancedMesh[] = []
  for (const [kind, list] of byKind) {
    const geometry = geometries.get(kind) ?? geometries.get(TREE_BIRCH)!
    const mesh = new InstancedMesh(geometry, material, list.length)
    // Which instance is which tree of the tile, so one can be felled.
    const slots = new Map<number, number>()
    mesh.userData.treeSlots = slots
    list.forEach(({ tree, index }, i) => {
      slots.set(index, i)
      const half = spin(tree) / 2
      mesh.setMatrixAt(i, mat4.compose(matrix, tree.x, tree.base, tree.z, 0, Math.sin(half), 0, Math.cos(half), tree.crown, tree.height, tree.crown))
    })
    mesh.needsUpdate()
    mesh.castShadow = true
    mesh.receiveShadow = false
    mesh.userData.sharedGeometry = true
    meshes.push(mesh)
  }
  return meshes
}
