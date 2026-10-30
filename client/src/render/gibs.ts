import { Geometry, Group, Mesh, SkinnedMesh, Matrix4, Quaternion, Vector3 } from '@ragnarok/engine'

/** One rigid piece of the character, as it is in the bind pose. */
export interface Limb {
  key: string
  /** The bone it rides, by name. */
  bone: string
  /** Vertices centred on `centre`, which is in the mesh's bind-pose space. */
  geometry: Geometry
  centre: Vector3
}

/** Which limb a bone belongs to: arms and legs split at the elbow and knee, the rest is the torso. */
export function limbOf(bone: string): { key: string; bone: string } {
  const side = /_([LR])$/.exec(bone)?.[1]
  const base = side ? bone.slice(0, -2) : bone
  switch (base) {
    case 'head':
    case 'neck':
      return { key: 'head', bone: 'head' }
    case 'upper_arm':
      return { key: `upper_arm_${side}`, bone: bone }
    case 'forearm':
    case 'hand':
      return { key: `forearm_${side}`, bone: `forearm_${side}` }
    case 'thigh':
      return { key: `thigh_${side}`, bone: bone }
    case 'shin':
    case 'foot':
      return { key: `shin_${side}`, bone: `shin_${side}` }
    default:
      return { key: 'torso', bone: 'spine' }
  }
}

/**
 * Cuts a skinned mesh into one rigid piece per limb: every triangle goes to the limb of the bone that
 * moves it most. The model is built from parts each bound whole to one bone, so the cuts fall on the
 * joints. Triangles of a mesh with no skin make no limbs.
 */
export function splitLimbs(mesh: SkinnedMesh): Limb[] {
  const geometry = mesh.geometry
  const { joints, weights, indices } = geometry
  if (!joints || !weights) return []
  const names = mesh.skeleton.bones.map((bone) => bone.name)
  const triangles = geometry.indexCount / 3
  const vertexAt = (corner: number) => (indices ? indices[corner]! : corner)

  /** The limb a vertex belongs to: the joint with the most weight. */
  const limbOfVertex = (vertex: number) => {
    let best = 0
    let weight = -1
    for (let slot = 0; slot < 4; slot++) {
      const w = weights[vertex * 4 + slot]!
      if (w > weight) {
        weight = w
        best = joints[vertex * 4 + slot]!
      }
    }
    return limbOf(names[best] ?? 'spine')
  }

  const groups = new Map<string, { bone: string; vertices: number[] }>()
  for (let t = 0; t < triangles; t++) {
    const corners = [vertexAt(t * 3), vertexAt(t * 3 + 1), vertexAt(t * 3 + 2)]
    const limb = limbOfVertex(corners[0]!)
    let group = groups.get(limb.key)
    if (!group) groups.set(limb.key, (group = { bone: limb.bone, vertices: [] }))
    group.vertices.push(...corners)
  }

  const limbs: Limb[] = []
  for (const [key, group] of groups) {
    const take = (source: Float32Array | undefined, size: number) => {
      if (!source) return undefined
      const array = new Float32Array(group.vertices.length * size)
      group.vertices.forEach((vertex, i) => {
        for (let c = 0; c < size; c++) array[i * size + c] = source[vertex * size + c]!
      })
      return array
    }
    const colorSize = geometry.colors ? geometry.colors.length / geometry.vertexCount : 0
    const out = new Geometry({ positions: take(geometry.positions, 3)!, normals: take(geometry.normals, 3), colors: take(geometry.colors, colorSize) })
    // Centred on the middle of its box, so it tumbles about itself.
    const p = out.positions
    const min = [Infinity, Infinity, Infinity]
    const max = [-Infinity, -Infinity, -Infinity]
    for (let i = 0; i < p.length; i += 3) {
      for (let c = 0; c < 3; c++) {
        min[c] = Math.min(min[c]!, p[i + c]!)
        max[c] = Math.max(max[c]!, p[i + c]!)
      }
    }
    const centre = new Vector3((min[0]! + max[0]!) / 2, (min[1]! + max[1]!) / 2, (min[2]! + max[2]!) / 2)
    out.translate(-centre.x, -centre.y, -centre.z)
    limbs.push({ key, bone: group.bone, geometry: out, centre })
  }
  return limbs
}

const SCRATCH = {
  matrix: new Matrix4(),
  inverseBind: new Matrix4(),
  position: new Vector3(),
  quaternion: new Quaternion(),
  scale: new Vector3(),
}

/**
 * Places `limb` of `mesh` where its bone has it right now: a group positioned at the limb's centre,
 * holding the limb's mesh, ready to be thrown. Null when the skeleton has no such bone.
 */
export function poseLimb(mesh: SkinnedMesh, limb: Limb): Group | null {
  const joint = mesh.skeleton.bones.findIndex((bone) => bone.name === limb.bone)
  if (joint < 0) return null
  const bone = mesh.skeleton.bones[joint]!
  // The skinning transform of a rigidly bound vertex, as the shader builds it: the bone's world matrix
  // times its inverse bind (a skinned mesh's own node transform does not count).
  const matrix = SCRATCH.matrix.fromArray(bone.matrixWorld).multiply(SCRATCH.inverseBind.fromArray(mesh.skeleton.inverseBind, joint * 16))
  const group = new Group()
  group.position.copy(limb.centre.clone().applyMatrix4(matrix))
  matrix.decompose(SCRATCH.position, SCRATCH.quaternion, SCRATCH.scale)
  group.quaternion.copy(SCRATCH.quaternion)
  group.scale.copy(SCRATCH.scale)
  const piece = new Mesh(limb.geometry, mesh.material)
  piece.castShadow = true
  group.add(piece)
  return group
}
