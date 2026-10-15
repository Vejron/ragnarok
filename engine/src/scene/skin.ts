import type { Geometry } from './geometry'
import type { Material } from './material'
import { Mesh } from './mesh'
import type { Node } from './node'

/** The bones a skinned mesh follows, and each bone's inverse bind matrix (column-major, 16 floats each). */
export class Skeleton {
  constructor(public bones: Node[], readonly inverseBind: Float32Array) {}

  /** The same skeleton on other bones: those `map` takes these bones to. */
  remap(map: Map<Node, Node>): Skeleton {
    return new Skeleton(this.bones.map((bone) => map.get(bone) ?? bone), this.inverseBind)
  }
}

/**
 * A mesh deformed by a skeleton: each vertex is moved by up to four bones. Its own transform is ignored
 * for drawing (the bones are in world space), as glTF specifies; it still places the mesh for culling.
 */
export class SkinnedMesh extends Mesh {
  constructor(geometry: Geometry, material: Material, public skeleton: Skeleton) {
    super(geometry, material)
  }

  protected override cloneSelf(): Node {
    const copy = new SkinnedMesh(this.geometry, this.material, this.skeleton)
    copy.castShadow = this.castShadow
    copy.receiveShadow = this.receiveShadow
    copy.renderOrder = this.renderOrder
    copy.frustumCulled = this.frustumCulled
    copy.tint.set(this.tint)
    return copy
  }

  protected override afterClone(map: Map<Node, Node>) {
    this.skeleton = this.skeleton.remap(map)
  }
}
