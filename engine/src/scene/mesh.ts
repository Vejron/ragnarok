import type { Geometry } from './geometry'
import type { Material } from './material'
import { Node } from './node'

/** Geometry drawn with a material at this node's transform. */
export class Mesh extends Node {
  castShadow = false
  receiveShadow = false
  /** Lower draws first; within one value, opaque draws are batched and transparent ones sorted far to near. */
  renderOrder = 0
  /** Skip the frustum test (for things whose vertices move outside their bounds, like billboards). */
  frustumCulled = true
  /** Multiplies the material colour and opacity for this mesh alone: per-object tint without a new material. */
  readonly tint = new Float32Array([1, 1, 1, 1])

  constructor(public geometry: Geometry, public material: Material) {
    super()
  }

  protected override cloneSelf(): Node {
    const copy = new Mesh(this.geometry, this.material)
    copy.castShadow = this.castShadow
    copy.receiveShadow = this.receiveShadow
    copy.renderOrder = this.renderOrder
    copy.frustumCulled = this.frustumCulled
    copy.tint.set(this.tint)
    return copy
  }
}

/**
 * Many copies of one geometry and material, each with its own matrix and tint, drawn in one call.
 * Matrices are relative to this node.
 */
export class InstancedMesh extends Mesh {
  /** How many of the instances to draw, up to `capacity`. */
  count: number
  readonly matrices: Float32Array
  readonly colors: Float32Array
  /** Bumped by `needsUpdate`, so the renderer recomputes the bounds. */
  version = 0
  /** @internal bounds of all instances in this node's space, cached per version. */
  boundsVersion = -1
  readonly bounds = { x: 0, y: 0, z: 0, radius: 0 }

  constructor(geometry: Geometry, material: Material, readonly capacity: number) {
    super(geometry, material)
    this.count = capacity
    this.matrices = new Float32Array(capacity * 16)
    this.colors = new Float32Array(capacity * 4).fill(1)
    for (let i = 0; i < capacity; i++) this.matrices[i * 16] = this.matrices[i * 16 + 5] = this.matrices[i * 16 + 10] = this.matrices[i * 16 + 15] = 1
  }

  setMatrixAt(i: number, m: ArrayLike<number>) {
    for (let k = 0; k < 16; k++) this.matrices[i * 16 + k] = m[k]!
  }

  setColorAt(i: number, r: number, g: number, b: number, a = 1) {
    this.colors.set([r, g, b, a], i * 4)
  }

  /** Call after changing matrices or count. */
  needsUpdate() {
    this.version++
  }

  protected override cloneSelf(): Node {
    const copy = new InstancedMesh(this.geometry, this.material, this.capacity)
    copy.count = this.count
    copy.matrices.set(this.matrices)
    copy.colors.set(this.colors)
    copy.castShadow = this.castShadow
    copy.receiveShadow = this.receiveShadow
    copy.renderOrder = this.renderOrder
    copy.frustumCulled = this.frustumCulled
    return copy
  }
}
