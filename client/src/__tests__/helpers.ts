import { Mesh, SkinnedMesh, type Node, type XYZ, Matrix4, Vector3 } from '@ragnarok/engine'

/** An axis-aligned box, empty until a point is put in it. */
export class Box {
  readonly min = new Vector3(Infinity, Infinity, Infinity)
  readonly max = new Vector3(-Infinity, -Infinity, -Infinity)

  expandByPoint(point: XYZ): this {
    this.min.min(point)
    this.max.max(point)
    return this
  }

  getCenter(out: Vector3) {
    return out.addVectors(this.min, this.max).multiplyScalar(0.5)
  }

  getSize(out: Vector3) {
    return out.subVectors(this.max, this.min)
  }
}

/** The axis-aligned box around every vertex under `root`, with skinned meshes posed by their bones. */
export function boxOf(root: Node): Box {
  root.updateMatrixWorld(true)
  const box = new Box()
  const v = new Vector3()
  const world = new Matrix4()
  const bone = new Matrix4()
  const inverse = new Matrix4()
  const sum = new Vector3()
  root.traverse((node) => {
    if (!(node instanceof Mesh) || !node.visible) return
    const { positions, joints, weights } = node.geometry
    const count = positions.length / 3
    if (node instanceof SkinnedMesh && joints && weights) {
      for (let i = 0; i < count; i++) {
        sum.set(0, 0, 0)
        for (let k = 0; k < 4; k++) {
          const w = weights[i * 4 + k]!
          if (w === 0) continue
          const j = joints[i * 4 + k]!
          bone.fromArray(node.skeleton.bones[j]!.matrixWorld).multiply(inverse.fromArray(node.skeleton.inverseBind, j * 16))
          sum.add(v.fromArray(positions, i * 3).applyMatrix4(bone).multiplyScalar(w))
        }
        box.expandByPoint(sum)
      }
    } else {
      world.fromArray(node.matrixWorld)
      for (let i = 0; i < count; i++) box.expandByPoint(v.fromArray(positions, i * 3).applyMatrix4(world))
    }
  })
  return box
}

/** The triangles under `root`. */
export function triangleCount(root: Node): number {
  let count = 0
  root.traverse((node) => {
    if (node instanceof Mesh) count += node.geometry.indexCount / 3
  })
  return count
}
