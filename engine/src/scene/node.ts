import * as mat4 from '../math/mat4'
import { Quat, Vec3 } from '../math/vec'

/**
 * A transform in the scene tree. Local TRS lives in `position`, `quaternion` and `scale`; writing any of
 * them marks the node stale, and `updateWorld` recomputes only stale nodes and their descendants.
 */
export class Node {
  name = ''
  parent: Node | null = null
  readonly children: Node[] = []
  readonly position = new Vec3()
  readonly quaternion = new Quat()
  readonly scale = new Vec3(1, 1, 1)
  /** Hidden nodes hide their whole subtree. */
  visible = true
  /** Free for the application: the engine never reads it. */
  userData: Record<string, unknown> = {}
  readonly matrix = mat4.create()
  readonly matrixWorld = mat4.create()
  /** Bumped whenever `matrixWorld` changes, so caches keyed on it can tell. */
  worldVersion = 0
  private localDirty = true
  private worldDirty = true

  constructor() {
    const dirty = () => { this.localDirty = true }
    this.position.onChange = dirty
    this.quaternion.onChange = dirty
    this.scale.onChange = dirty
  }

  add(...nodes: Node[]): this {
    for (const node of nodes) {
      node.removeFromParent()
      node.parent = this
      node.worldDirty = true
      this.children.push(node)
    }
    return this
  }

  remove(...nodes: Node[]): this {
    for (const node of nodes) {
      const i = this.children.indexOf(node)
      if (i < 0) continue
      this.children.splice(i, 1)
      node.parent = null
      node.worldDirty = true
    }
    return this
  }

  removeFromParent(): this {
    this.parent?.remove(this)
    return this
  }

  traverse(visit: (node: Node) => void) {
    visit(this)
    for (const child of this.children) child.traverse(visit)
  }

  getObjectByName(name: string): Node | undefined {
    if (this.name === name) return this
    for (const child of this.children) {
      const found = child.getObjectByName(name)
      if (found) return found
    }
    return undefined
  }

  /** Overwrites the local matrix (and the TRS it implies is NOT updated): for nodes driven by raw matrices. */
  setMatrix(m: ArrayLike<number>) {
    mat4.copy(this.matrix, m)
    this.matrixFromTRS = false
    this.localDirty = false
    this.worldDirty = true
  }

  /** False once `setMatrix` has taken over; writing TRS again hands control back. */
  private matrixFromTRS = true

  /**
   * Recomputes world matrices below this node. `force` is set when an ancestor moved. Returns nothing;
   * call it on the root each frame (the renderer does).
   */
  updateWorld(force = false) {
    if (this.localDirty) {
      const p = this.position, q = this.quaternion, s = this.scale
      mat4.compose(this.matrix, p.x, p.y, p.z, q.x, q.y, q.z, q.w, s.x, s.y, s.z)
      this.localDirty = false
      this.matrixFromTRS = true
      this.worldDirty = true
    }
    if (this.worldDirty || force) {
      if (this.parent) mat4.multiply(this.matrixWorld, this.parent.matrixWorld, this.matrix)
      else mat4.copy(this.matrixWorld, this.matrix)
      this.worldVersion++
      this.worldDirty = false
      force = true
    }
    for (const child of this.children) child.updateWorld(force)
  }

  /** World position, from the last `updateWorld`. */
  getWorldPosition(out: { x: number; y: number; z: number }) {
    const m = this.matrixWorld
    out.x = m[12]!
    out.y = m[13]!
    out.z = m[14]!
    return out
  }

  /** Copies the node with its subtree. Meshes share geometry and material with the original. */
  clone(): Node {
    const copy = this.cloneSelf()
    copy.name = this.name
    copy.visible = this.visible
    copy.userData = { ...this.userData }
    copy.position.copy(this.position)
    copy.quaternion.copy(this.quaternion)
    copy.scale.copy(this.scale)
    if (!this.matrixFromTRS) copy.setMatrix(this.matrix)
    for (const child of this.children) copy.add(child.clone())
    return copy
  }

  /** Subclasses return a fresh instance carrying their own fields. */
  protected cloneSelf(): Node {
    return new Node()
  }
}
