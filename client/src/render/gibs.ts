import * as THREE from 'three'

/** One rigid piece of the character, as it is in the bind pose. */
export interface Limb {
  key: string
  /** The bone it rides, by name. */
  bone: string
  /** Vertices centred on `centre`, which is in the mesh's bind-pose space. */
  geometry: THREE.BufferGeometry
  centre: THREE.Vector3
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
export function splitLimbs(mesh: THREE.SkinnedMesh): Limb[] {
  const geometry = mesh.geometry
  const position = geometry.getAttribute('position')
  const skinIndex = geometry.getAttribute('skinIndex')
  const skinWeight = geometry.getAttribute('skinWeight')
  if (!position || !skinIndex || !skinWeight) return []
  const names = mesh.skeleton.bones.map((bone) => bone.name)
  const index = geometry.getIndex()
  const triangles = index ? index.count / 3 : position.count / 3
  const vertexAt = (corner: number) => (index ? index.getX(corner) : corner)

  /** The limb a vertex belongs to: the joint with the most weight. */
  const limbOfVertex = (vertex: number) => {
    let best = 0
    let weight = -1
    for (let slot = 0; slot < 4; slot++) {
      const w = skinWeight.getComponent(vertex, slot)
      if (w > weight) {
        weight = w
        best = skinIndex.getComponent(vertex, slot)
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
    const out = new THREE.BufferGeometry()
    for (const name of ['position', 'normal', 'color'] as const) {
      const source = geometry.getAttribute(name)
      if (!source) continue
      const size = source.itemSize
      const array = new Float32Array(group.vertices.length * size)
      group.vertices.forEach((vertex, i) => {
        for (let c = 0; c < size; c++) array[i * size + c] = source.getComponent(vertex, c)
      })
      out.setAttribute(name, new THREE.BufferAttribute(array, size))
    }
    out.computeBoundingBox()
    const centre = out.boundingBox!.getCenter(new THREE.Vector3())
    out.translate(-centre.x, -centre.y, -centre.z)
    out.computeBoundingSphere()
    limbs.push({ key, bone: group.bone, geometry: out, centre })
  }
  return limbs
}

const SCRATCH = {
  matrix: new THREE.Matrix4(),
  scale: new THREE.Vector3(),
}

/**
 * Places `limb` of `mesh` where its bone has it right now: a group positioned at the limb's centre,
 * holding the limb's mesh, ready to be thrown. Null when the skeleton has no such bone.
 */
export function poseLimb(mesh: THREE.SkinnedMesh, limb: Limb): THREE.Group | null {
  const joint = mesh.skeleton.bones.findIndex((bone) => bone.name === limb.bone)
  if (joint < 0) return null
  const bone = mesh.skeleton.bones[joint]!
  // The skinning transform of a rigidly bound vertex, as the shader builds it.
  const matrix = SCRATCH.matrix
    .copy(mesh.matrixWorld)
    .multiply(mesh.bindMatrixInverse)
    .multiply(bone.matrixWorld)
    .multiply(mesh.skeleton.boneInverses[joint]!)
    .multiply(mesh.bindMatrix)
  const group = new THREE.Group()
  group.position.copy(limb.centre).applyMatrix4(matrix)
  matrix.decompose(new THREE.Vector3(), group.quaternion, SCRATCH.scale)
  group.scale.copy(SCRATCH.scale)
  const piece = new THREE.Mesh(limb.geometry, mesh.material)
  piece.castShadow = true
  group.add(piece)
  return group
}
