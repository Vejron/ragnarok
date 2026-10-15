import * as mat4 from '../math/mat4'
import type { XYZ } from '../math/vec'
import { Node } from './node'

/** A perspective camera looking down its local -Z. Field of view is vertical, in degrees. */
export class PerspectiveCamera extends Node {
  readonly view = mat4.create()
  readonly projection = mat4.create()
  readonly viewProjection = mat4.create()

  constructor(public fov = 60, public aspect = 1, public near = 0.1, public far = 1000) {
    super()
  }

  /** Points the camera at a world position. Assumes the camera has no rotated parent. */
  lookAt(x: number, y: number, z: number) {
    this.updateWorld()
    const p = this.position
    const m = mat4.lookAt(mat4.create(), p.x, p.y, p.z, x, y, z)
    // The camera's rotation is the transpose of the view's, so its row i, column j is view element [i * 4 + j].
    const m00 = m[0]!, m01 = m[1]!, m02 = m[2]!
    const m10 = m[4]!, m11 = m[5]!, m12 = m[6]!
    const m20 = m[8]!, m21 = m[9]!, m22 = m[10]!
    const trace = m00 + m11 + m22
    if (trace > 0) {
      const s = 0.5 / Math.sqrt(trace + 1)
      this.quaternion.set((m21 - m12) * s, (m02 - m20) * s, (m10 - m01) * s, 0.25 / s)
    } else if (m00 > m11 && m00 > m22) {
      const s = 2 * Math.sqrt(1 + m00 - m11 - m22)
      this.quaternion.set(0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s)
    } else if (m11 > m22) {
      const s = 2 * Math.sqrt(1 + m11 - m00 - m22)
      this.quaternion.set((m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s)
    } else {
      const s = 2 * Math.sqrt(1 + m22 - m00 - m11)
      this.quaternion.set((m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s)
    }
  }

  /** The unit vector the camera looks along, in world space. */
  getWorldDirection<T extends XYZ>(out: T): T {
    this.updateWorld()
    const m = this.matrixWorld
    const l = Math.hypot(m[8]!, m[9]!, m[10]!) || 1
    out.x = -m[8]! / l
    out.y = -m[9]! / l
    out.z = -m[10]! / l
    return out
  }

  /**
   * Moves a world-space point to normalised device coordinates in place: x and y run -1..1 with y up, z is
   * the depth buffer value. Returns false when the point is behind the camera (the result is then meaningless).
   */
  project<T extends XYZ>(point: T): boolean {
    this.updateWorld()
    this.updateMatrices()
    const m = this.viewProjection
    const x = point.x, y = point.y, z = point.z
    const w = m[3]! * x + m[7]! * y + m[11]! * z + m[15]!
    const cx = m[0]! * x + m[4]! * y + m[8]! * z + m[12]!
    const cy = m[1]! * x + m[5]! * y + m[9]! * z + m[13]!
    const cz = m[2]! * x + m[6]! * y + m[10]! * z + m[14]!
    if (w <= 0) return false
    point.x = cx / w
    point.y = cy / w
    point.z = cz / w
    return true
  }

  /** Refreshes view, projection and their product from the world matrix. The renderer calls it. */
  updateMatrices() {
    mat4.invert(this.view, this.matrixWorld)
    mat4.perspectiveReversed(this.projection, (this.fov * Math.PI) / 180, this.aspect, this.near, this.far)
    mat4.multiply(this.viewProjection, this.projection, this.view)
  }
}
