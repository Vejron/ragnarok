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
  /**
   * Drawn after everything else, effects included, ignoring depth: a reticle or a marker that must never be
   * hidden. Overlay meshes sort by `renderOrder`, then far to near, and are not batched or shadowed.
   */
  overlay = false
  /**
   * Held in front of the camera (a map in the hands, a first-person item): drawn at the end of the main
   * pass over the scene, ignoring its depth, but writing its own, so effects behind it stay behind and
   * the ink outlines it. Held meshes sort by `renderOrder`, then far to near, and are not shadowed.
   */
  held = false
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
    copy.overlay = this.overlay
    copy.held = this.held
    copy.tint.set(this.tint)
    return copy
  }
}

/** A coarser geometry of an instanced mesh, and the distance from the camera it takes over at, metres. */
export interface DetailLevel {
  geometry: Geometry
  from: number
  /**
   * What the shadow pass draws at this level instead of `geometry`: for a shape the depth-only shadow
   * shader cannot draw, such as a billboard its material turns to the camera.
   */
  shadow?: Geometry
  /**
   * Metres before `from` over which an instance is drawn at both this level and the one before it, so
   * the material can cross-fade the two (a dither: each pixel from one or the other) rather than swap
   * them in a frame. Scaled by the renderer's `detail` as `from` is. Without it the change is instant.
   */
  fade?: number
}

/**
 * How far either side of a fade band an instance is still drawn at both levels, metres: the renderer
 * measures from its bounds' centre, the material from wherever it likes, and the two must not leave a
 * gap where neither draws the instance.
 */
export const FADE_MARGIN = 0.5

/**
 * Which level an instance `distance2` (metres, squared) from the camera is drawn at: 0 is the mesh's
 * own geometry, `n` is `levels[n - 1]`. `levels` are in order of `from`, and `detail` multiplies every
 * `from`, so under 1 the coarser levels come nearer.
 */
export function levelAt(distance2: number, levels: readonly DetailLevel[], detail = 1): number {
  let level = 0
  for (let i = 0; i < levels.length; i++) {
    const from = levels[i]!.from * detail
    if (distance2 < from * from) break
    level = i + 1
  }
  return level
}

/**
 * The second level an instance `distance2` (metres, squared) from the camera is drawn at while it
 * fades between two (`DetailLevel.fade`), or -1 when it is drawn at its own level alone.
 */
export function fadingAt(distance2: number, levels: readonly DetailLevel[], detail = 1): number {
  const level = levelAt(distance2, levels, detail)
  const ahead = levels[level]
  if (ahead?.fade && distance2 >= Math.max(0, (ahead.from - ahead.fade) * detail - FADE_MARGIN) ** 2) return level + 1
  const behind = levels[level - 1]
  if (behind?.fade && distance2 < (behind.from * detail + FADE_MARGIN) ** 2) return level - 1
  return -1
}

/**
 * Many copies of one geometry and material, each with its own matrix and tint, drawn in one call.
 * Matrices are relative to this node.
 */
export class InstancedMesh extends Mesh {
  /**
   * Coarser geometries for instances further from the camera, nearest first. With this set (an empty
   * list will do) each instance is culled on its own and drawn at the level its own distance picks, so
   * a forest costs what is near and in view rather than what is loaded. Without it the mesh is culled
   * and drawn whole. The levels' geometries must fit inside the bounds of `geometry`.
   */
  levels: DetailLevel[] | null = null
  /** The finest level the shadow pass draws an instance at: a shadow needs less than the thing itself. */
  shadowLevel = 0
  /** @internal each instance's bounding sphere in world space (x, y, z, radius), and what it was worked out from. */
  spheres: Float32Array | null = null
  spheresVersion = -1
  spheresWorld = -1
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
    copy.levels = this.levels
    copy.shadowLevel = this.shadowLevel
    copy.castShadow = this.castShadow
    copy.receiveShadow = this.receiveShadow
    copy.renderOrder = this.renderOrder
    copy.frustumCulled = this.frustumCulled
    return copy
  }
}
