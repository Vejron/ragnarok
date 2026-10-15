import * as mat4 from '../math/mat4'
import { Euler, Quat, Vec3, type XYZ } from '../math/vec'

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
  private readonly euler = new Euler()
  private eulerStale = false
  private eulerSyncing = false
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
    this.scale.onChange = dirty
    this.quaternion.onChange = () => {
      this.localDirty = true
      if (!this.eulerSyncing) this.eulerStale = true
    }
    this.euler.onChange = () => {
      this.eulerSyncing = true
      this.quaternion.setFromEuler(this.euler.x, this.euler.y, this.euler.z, this.euler.order)
      this.eulerSyncing = false
    }
  }

  /**
   * The orientation as Euler angles. It is the same rotation as `quaternion`: writing either one
   * updates the other, and `rotation.set(x, y, z, order)` replaces any pitch and roll left by earlier writes.
   */
  get rotation(): Euler {
    if (this.eulerStale) {
      this.euler.fromQuaternion(this.quaternion)
      this.eulerStale = false
    }
    return this.euler
  }

  /** Turns by `angle` about this node's own X axis. */
  rotateX(angle: number): this {
    return this.rotateOn(1, 0, 0, angle)
  }

  rotateY(angle: number): this {
    return this.rotateOn(0, 1, 0, angle)
  }

  rotateZ(angle: number): this {
    return this.rotateOn(0, 0, 1, angle)
  }

  private rotateOn(x: number, y: number, z: number, angle: number): this {
    const s = Math.sin(angle / 2)
    this.quaternion.multiply({ x: x * s, y: y * s, z: z * s, w: Math.cos(angle / 2) })
    return this
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

  /** Sets position, rotation and scale from a column-major matrix (no shear). */
  setFromMatrix(m: ArrayLike<number>): this {
    const p = [0, 0, 0], q = [0, 0, 0, 1], s = [1, 1, 1]
    mat4.decompose(m, p, q, s)
    this.position.set(p[0]!, p[1]!, p[2]!)
    this.quaternion.set(q[0]!, q[1]!, q[2]!, q[3]!)
    this.scale.set(s[0]!, s[1]!, s[2]!)
    return this
  }

  /** Three's name for `updateWorld`, for code that came from there. */
  updateMatrixWorld(force = false) {
    this.updateWorld(force)
  }

  /**
   * Brings this node's world matrix up to date, through its ancestors first when `parents`, and the whole
   * subtree below it when `children`. Only what is stale is recomputed.
   */
  updateWorldMatrix(parents = false, children = false) {
    const changed = parents ? this.refresh() : this.refreshSelf(false)
    if (children) for (const child of this.children) child.updateWorld(changed)
  }

  /** Recomputes this node's world matrix if it, or an ancestor, changed; returns whether it did. */
  private refresh(): boolean {
    const parentChanged = this.parent ? this.parent.refresh() : false
    return this.refreshSelf(parentChanged)
  }

  private refreshSelf(parentChanged: boolean): boolean {
    if (!this.localDirty && !this.worldDirty && !parentChanged) return false
    if (this.localDirty) {
      const p = this.position, q = this.quaternion, s = this.scale
      mat4.compose(this.matrix, p.x, p.y, p.z, q.x, q.y, q.z, q.w, s.x, s.y, s.z)
      this.localDirty = false
      this.matrixFromTRS = true
    }
    if (this.parent) mat4.multiply(this.matrixWorld, this.parent.matrixWorld, this.matrix)
    else mat4.copy(this.matrixWorld, this.matrix)
    this.worldVersion++
    this.worldDirty = false
    // Children are now stale against this matrix, and a later `updateWorld` from above must reach them.
    for (const child of this.children) child.worldDirty = true
    return true
  }

  /** Moves `point` (in this node's space) to world space, in place, from the last world update. */
  localToWorld<T extends XYZ>(point: T): T {
    const m = this.matrixWorld
    const x = point.x, y = point.y, z = point.z
    point.x = m[0]! * x + m[4]! * y + m[8]! * z + m[12]!
    point.y = m[1]! * x + m[5]! * y + m[9]! * z + m[13]!
    point.z = m[2]! * x + m[6]! * y + m[10]! * z + m[14]!
    return point
  }

  /** Moves `point` (in world space) into this node's space, in place. */
  worldToLocal<T extends XYZ>(point: T): T {
    const inverse = mat4.invert(mat4.create(), this.matrixWorld)
    if (!inverse) return point
    const x = point.x, y = point.y, z = point.z
    point.x = inverse[0]! * x + inverse[4]! * y + inverse[8]! * z + inverse[12]!
    point.y = inverse[1]! * x + inverse[5]! * y + inverse[9]! * z + inverse[13]!
    point.z = inverse[2]! * x + inverse[6]! * y + inverse[10]! * z + inverse[14]!
    return point
  }

  /** World orientation from the last world update. */
  getWorldQuaternion<T extends { set(x: number, y: number, z: number, w: number): unknown }>(out: T): T {
    this.refresh()
    const q = [0, 0, 0, 0]
    mat4.decompose(this.matrixWorld, [0, 0, 0], q, [0, 0, 0])
    out.set(q[0]!, q[1]!, q[2]!, q[3]!)
    return out
  }

  getWorldScale<T extends XYZ>(out: T): T {
    this.refresh()
    const s = [0, 0, 0]
    mat4.decompose(this.matrixWorld, [0, 0, 0], [0, 0, 0, 0], s)
    out.x = s[0]!
    out.y = s[1]!
    out.z = s[2]!
    return out
  }

  /**
   * Re-parents `node` under this one without moving it in the world: its local transform is rewritten
   * to give the same world matrix. Both world matrices must be current.
   */
  attach(node: Node): this {
    const inverse = mat4.invert(mat4.create(), this.matrixWorld)
    const local = mat4.create()
    if (inverse) mat4.multiply(local, inverse, node.matrixWorld)
    else mat4.copy(local, node.matrixWorld)
    node.removeFromParent()
    this.add(node)
    const p = [0, 0, 0], q = [0, 0, 0, 1], s = [1, 1, 1]
    mat4.decompose(local, p, q, s)
    node.position.set(p[0]!, p[1]!, p[2]!)
    node.quaternion.set(q[0]!, q[1]!, q[2]!, q[3]!)
    node.scale.set(s[0]!, s[1]!, s[2]!)
    return this
  }

  /** World position, brought up to date first. */
  getWorldPosition<T extends XYZ>(out: T): T {
    this.refresh()
    const m = this.matrixWorld
    out.x = m[12]!
    out.y = m[13]!
    out.z = m[14]!
    return out
  }

  /**
   * Copies the node with its subtree. Meshes share geometry and material with the original; skinned
   * meshes in the subtree are rebound to the copied bones.
   */
  clone(): Node {
    const map = new Map<Node, Node>()
    const copy = this.cloneTree(map)
    copy.traverse((node) => node.afterClone(map))
    return copy
  }

  private cloneTree(map: Map<Node, Node>): Node {
    const copy = this.cloneSelf()
    map.set(this, copy)
    copy.name = this.name
    copy.visible = this.visible
    copy.userData = { ...this.userData }
    copy.position.copy(this.position)
    copy.quaternion.copy(this.quaternion)
    copy.scale.copy(this.scale)
    if (!this.matrixFromTRS) copy.setMatrix(this.matrix)
    for (const child of this.children) copy.add(child.cloneTree(map))
    return copy
  }

  /** Lets a copy re-point references into the original subtree (`map` takes originals to copies). */
  protected afterClone(_map: Map<Node, Node>) {}

  /** Subclasses return a fresh instance carrying their own fields. */
  protected cloneSelf(): Node {
    return new Node()
  }
}

/** A node that only groups others. */
export class Group extends Node {
  protected override cloneSelf(): Node {
    return new Group()
  }
}
