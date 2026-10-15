import type { Mat4 } from './mat4'

/**
 * The six planes of a view-projection, for culling bounding spheres. Planes are stored as
 * (nx, ny, nz, d) with normals pointing inward, so a point is inside when n·p + d >= 0 for all six.
 */
export class Frustum {
  readonly planes = new Float32Array(24)

  /** Extracts the planes of clip space with WebGPU's 0 <= z <= w depth range. */
  setFromMatrix(m: Mat4): this {
    const p = this.planes
    const r = (row: number, col: number) => m[col * 4 + row]!
    const set = (i: number, a: number, b: number, c: number, d: number) => {
      const len = Math.hypot(a, b, c) || 1
      p[i * 4] = a / len
      p[i * 4 + 1] = b / len
      p[i * 4 + 2] = c / len
      p[i * 4 + 3] = d / len
    }
    for (let axis = 0; axis < 2; axis++) {
      set(axis * 2, r(3, 0) + r(axis, 0), r(3, 1) + r(axis, 1), r(3, 2) + r(axis, 2), r(3, 3) + r(axis, 3))
      set(axis * 2 + 1, r(3, 0) - r(axis, 0), r(3, 1) - r(axis, 1), r(3, 2) - r(axis, 2), r(3, 3) - r(axis, 3))
    }
    set(4, r(2, 0), r(2, 1), r(2, 2), r(2, 3))
    set(5, r(3, 0) - r(2, 0), r(3, 1) - r(2, 1), r(3, 2) - r(2, 2), r(3, 3) - r(2, 3))
    return this
  }

  intersectsSphere(x: number, y: number, z: number, radius: number): boolean {
    const p = this.planes
    for (let i = 0; i < 24; i += 4) {
      if (p[i]! * x + p[i + 1]! * y + p[i + 2]! * z + p[i + 3]! < -radius) return false
    }
    return true
  }
}
