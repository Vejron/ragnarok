/**
 * Vectors, rotations and matrices to calculate with. Plain fields and no callbacks, unlike the `Vec3`
 * and `Quat` a node exposes (which tell it when they are written): these are what scratch values and
 * per-frame maths are made of. Each accepts any `XYZ`, so the two mix freely. Matrices are column-major
 * in `elements`, as `mat4` and a node's `matrixWorld` are, but in doubles.
 */

import type { XYZ, XYZW } from './vec'

export class Vector3 implements XYZ {
  // `declare`, and assigned only in the constructor: a real field would start out undefined, and V8
  // would then box every number written to it. These stay plain doubles, which is several times faster.
  declare x: number
  declare y: number
  declare z: number

  constructor(x = 0, y = 0, z = 0) {
    this.x = x
    this.y = y
    this.z = z
  }

  set(x: number, y: number, z: number): this {
    this.x = x
    this.y = y
    this.z = z
    return this
  }

  setScalar(s: number): this {
    this.x = s
    this.y = s
    this.z = s
    return this
  }

  setX(x: number): this {
    this.x = x
    return this
  }

  setY(y: number): this {
    this.y = y
    return this
  }

  setZ(z: number): this {
    this.z = z
    return this
  }

  clone(): Vector3 {
    return new Vector3(this.x, this.y, this.z)
  }

  copy(v: XYZ): this {
    this.x = v.x
    this.y = v.y
    this.z = v.z
    return this
  }

  add(v: XYZ): this {
    this.x += v.x
    this.y += v.y
    this.z += v.z
    return this
  }

  addVectors(a: XYZ, b: XYZ): this {
    this.x = a.x + b.x
    this.y = a.y + b.y
    this.z = a.z + b.z
    return this
  }

  addScaledVector(v: XYZ, s: number): this {
    this.x += v.x * s
    this.y += v.y * s
    this.z += v.z * s
    return this
  }

  sub(v: XYZ): this {
    this.x -= v.x
    this.y -= v.y
    this.z -= v.z
    return this
  }

  subVectors(a: XYZ, b: XYZ): this {
    this.x = a.x - b.x
    this.y = a.y - b.y
    this.z = a.z - b.z
    return this
  }

  multiply(v: XYZ): this {
    this.x *= v.x
    this.y *= v.y
    this.z *= v.z
    return this
  }

  multiplyScalar(s: number): this {
    this.x *= s
    this.y *= s
    this.z *= s
    return this
  }

  divideScalar(s: number): this {
    return this.multiplyScalar(1 / s)
  }

  negate(): this {
    this.x = -this.x
    this.y = -this.y
    this.z = -this.z
    return this
  }

  min(v: XYZ): this {
    this.x = Math.min(this.x, v.x)
    this.y = Math.min(this.y, v.y)
    this.z = Math.min(this.z, v.z)
    return this
  }

  max(v: XYZ): this {
    this.x = Math.max(this.x, v.x)
    this.y = Math.max(this.y, v.y)
    this.z = Math.max(this.z, v.z)
    return this
  }

  dot(v: XYZ): number {
    return this.x * v.x + this.y * v.y + this.z * v.z
  }

  lengthSq(): number {
    return this.x * this.x + this.y * this.y + this.z * this.z
  }

  length(): number {
    return Math.sqrt(this.x * this.x + this.y * this.y + this.z * this.z)
  }

  distanceToSquared(v: XYZ): number {
    const dx = this.x - v.x, dy = this.y - v.y, dz = this.z - v.z
    return dx * dx + dy * dy + dz * dz
  }

  distanceTo(v: XYZ): number {
    return Math.sqrt(this.distanceToSquared(v))
  }

  normalize(): this {
    return this.divideScalar(this.length() || 1)
  }

  setLength(length: number): this {
    return this.normalize().multiplyScalar(length)
  }

  lerp(v: XYZ, t: number): this {
    this.x += (v.x - this.x) * t
    this.y += (v.y - this.y) * t
    this.z += (v.z - this.z) * t
    return this
  }

  lerpVectors(a: XYZ, b: XYZ, t: number): this {
    this.x = a.x + (b.x - a.x) * t
    this.y = a.y + (b.y - a.y) * t
    this.z = a.z + (b.z - a.z) * t
    return this
  }

  cross(v: XYZ): this {
    return this.crossVectors(this, v)
  }

  crossVectors(a: XYZ, b: XYZ): this {
    const ax = a.x, ay = a.y, az = a.z
    const bx = b.x, by = b.y, bz = b.z
    this.x = ay * bz - az * by
    this.y = az * bx - ax * bz
    this.z = ax * by - ay * bx
    return this
  }

  applyQuaternion(q: XYZW): this {
    const vx = this.x, vy = this.y, vz = this.z
    const qx = q.x, qy = q.y, qz = q.z, qw = q.w
    // t = 2q × v, then v + w t + q × t.
    const tx = 2 * (qy * vz - qz * vy)
    const ty = 2 * (qz * vx - qx * vz)
    const tz = 2 * (qx * vy - qy * vx)
    this.x = vx + qw * tx + qy * tz - qz * ty
    this.y = vy + qw * ty + qz * tx - qx * tz
    this.z = vz + qw * tz + qx * ty - qy * tx
    return this
  }

  /** As a point: through `m`, with the perspective divide. */
  applyMatrix4(m: Matrix4): this {
    const x = this.x, y = this.y, z = this.z
    const e = m.elements
    const w = 1 / (e[3]! * x + e[7]! * y + e[11]! * z + e[15]!)
    this.x = (e[0]! * x + e[4]! * y + e[8]! * z + e[12]!) * w
    this.y = (e[1]! * x + e[5]! * y + e[9]! * z + e[13]!) * w
    this.z = (e[2]! * x + e[6]! * y + e[10]! * z + e[14]!) * w
    return this
  }

  /** As a direction: turned by `m`'s upper 3×3, and made unit length. */
  transformDirection(m: Matrix4): this {
    const x = this.x, y = this.y, z = this.z
    const e = m.elements
    this.x = e[0]! * x + e[4]! * y + e[8]! * z
    this.y = e[1]! * x + e[5]! * y + e[9]! * z
    this.z = e[2]! * x + e[6]! * y + e[10]! * z
    return this.normalize()
  }

  fromArray(a: ArrayLike<number>, offset = 0): this {
    this.x = a[offset]!
    this.y = a[offset + 1]!
    this.z = a[offset + 2]!
    return this
  }

  toArray<T extends { [i: number]: number } = number[]>(out: T = [] as unknown as T, offset = 0): T {
    out[offset] = this.x
    out[offset + 1] = this.y
    out[offset + 2] = this.z
    return out
  }
}

export class Quaternion implements XYZW {
  declare x: number
  declare y: number
  declare z: number
  declare w: number

  constructor(x = 0, y = 0, z = 0, w = 1) {
    this.x = x
    this.y = y
    this.z = z
    this.w = w
  }

  set(x: number, y: number, z: number, w: number): this {
    this.x = x
    this.y = y
    this.z = z
    this.w = w
    return this
  }

  identity(): this {
    return this.set(0, 0, 0, 1)
  }

  clone(): Quaternion {
    return new Quaternion(this.x, this.y, this.z, this.w)
  }

  copy(q: XYZW): this {
    return this.set(q.x, q.y, q.z, q.w)
  }

  /** Rotation of `angle` radians about the unit `axis`. */
  setFromAxisAngle(axis: XYZ, angle: number): this {
    const half = angle / 2, s = Math.sin(half)
    return this.set(axis.x * s, axis.y * s, axis.z * s, Math.cos(half))
  }

  /** The shortest rotation taking unit vector `from` to unit vector `to`. */
  setFromUnitVectors(from: XYZ, to: XYZ): this {
    const r = from.x * to.x + from.y * to.y + from.z * to.z + 1
    if (r < Number.EPSILON) {
      // Opposite: any axis at right angles to `from` will do.
      if (Math.abs(from.x) > Math.abs(from.z)) return this.set(-from.y, from.x, 0, 0).normalize()
      return this.set(0, -from.z, from.y, 0).normalize()
    }
    return this.set(from.y * to.z - from.z * to.y, from.z * to.x - from.x * to.z, from.x * to.y - from.y * to.x, r).normalize()
  }

  /** Rotation of a matrix's upper 3×3, which must not be scaled. */
  setFromRotationMatrix(m: Matrix4): this {
    const e = m.elements
    const m11 = e[0]!, m12 = e[4]!, m13 = e[8]!
    const m21 = e[1]!, m22 = e[5]!, m23 = e[9]!
    const m31 = e[2]!, m32 = e[6]!, m33 = e[10]!
    const trace = m11 + m22 + m33
    if (trace > 0) {
      const s = 0.5 / Math.sqrt(trace + 1)
      return this.set((m32 - m23) * s, (m13 - m31) * s, (m21 - m12) * s, 0.25 / s)
    }
    if (m11 > m22 && m11 > m33) {
      const s = 2 * Math.sqrt(1 + m11 - m22 - m33)
      return this.set(0.25 * s, (m12 + m21) / s, (m13 + m31) / s, (m32 - m23) / s)
    }
    if (m22 > m33) {
      const s = 2 * Math.sqrt(1 + m22 - m11 - m33)
      return this.set((m12 + m21) / s, 0.25 * s, (m23 + m32) / s, (m13 - m31) / s)
    }
    const s = 2 * Math.sqrt(1 + m33 - m11 - m22)
    return this.set((m13 + m31) / s, (m23 + m32) / s, 0.25 * s, (m21 - m12) / s)
  }

  normalize(): this {
    const l = Math.sqrt(this.x * this.x + this.y * this.y + this.z * this.z + this.w * this.w)
    if (l === 0) return this.identity()
    const s = 1 / l
    this.x *= s
    this.y *= s
    this.z *= s
    this.w *= s
    return this
  }

  /** The inverse rotation (the conjugate: these are unit quaternions). */
  invert(): this {
    return this.set(-this.x, -this.y, -this.z, this.w)
  }

  dot(q: XYZW): number {
    return this.x * q.x + this.y * q.y + this.z * q.z + this.w * q.w
  }

  /** this = this × q: rotate by q first, then by this. */
  multiply(q: XYZW): this {
    return this.multiplyQuaternions(this, q)
  }

  /** this = q × this: rotate by this first, then by q. */
  premultiply(q: XYZW): this {
    return this.multiplyQuaternions(q, this)
  }

  multiplyQuaternions(a: XYZW, b: XYZW): this {
    const ax = a.x, ay = a.y, az = a.z, aw = a.w
    const bx = b.x, by = b.y, bz = b.z, bw = b.w
    return this.set(
      ax * bw + aw * bx + ay * bz - az * by,
      ay * bw + aw * by + az * bx - ax * bz,
      az * bw + aw * bz + ax * by - ay * bx,
      aw * bw - ax * bx - ay * by - az * bz,
    )
  }

  /** Spherical interpolation towards `q`, `t` from 0 (this) to 1 (q). Nearly equal, a normalised lerp does. */
  slerp(q: XYZW, t: number): this {
    let qx = q.x, qy = q.y, qz = q.z, qw = q.w
    let cos = this.dot(q)
    if (cos < 0) {
      qx = -qx
      qy = -qy
      qz = -qz
      qw = -qw
      cos = -cos
    }
    let a = 1 - t, b = t
    if (cos < 0.9995) {
      const theta = Math.acos(cos), sin = Math.sin(theta)
      a = Math.sin(a * theta) / sin
      b = Math.sin(b * theta) / sin
      return this.set(this.x * a + qx * b, this.y * a + qy * b, this.z * a + qz * b, this.w * a + qw * b)
    }
    return this.set(this.x * a + qx * b, this.y * a + qy * b, this.z * a + qz * b, this.w * a + qw * b).normalize()
  }

  fromArray(a: ArrayLike<number>, offset = 0): this {
    return this.set(a[offset]!, a[offset + 1]!, a[offset + 2]!, a[offset + 3]!)
  }
}

export class Matrix4 {
  declare readonly elements: number[]

  constructor() {
    this.elements = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
  }

  identity(): this {
    const e = this.elements
    e[0] = 1; e[1] = 0; e[2] = 0; e[3] = 0
    e[4] = 0; e[5] = 1; e[6] = 0; e[7] = 0
    e[8] = 0; e[9] = 0; e[10] = 1; e[11] = 0
    e[12] = 0; e[13] = 0; e[14] = 0; e[15] = 1
    return this
  }

  clone(): Matrix4 {
    return new Matrix4().copy(this)
  }

  copy(m: Matrix4): this {
    const e = this.elements, f = m.elements
    e[0] = f[0]!; e[1] = f[1]!; e[2] = f[2]!; e[3] = f[3]!
    e[4] = f[4]!; e[5] = f[5]!; e[6] = f[6]!; e[7] = f[7]!
    e[8] = f[8]!; e[9] = f[9]!; e[10] = f[10]!; e[11] = f[11]!
    e[12] = f[12]!; e[13] = f[13]!; e[14] = f[14]!; e[15] = f[15]!
    return this
  }

  fromArray(a: ArrayLike<number>, offset = 0): this {
    const e = this.elements
    for (let i = 0; i < 16; i++) e[i] = a[offset + i]!
    return this
  }

  /** this = this × m. */
  multiply(m: Matrix4): this {
    return this.multiplyMatrices(this, m)
  }

  /** this = m × this. */
  premultiply(m: Matrix4): this {
    return this.multiplyMatrices(m, this)
  }

  multiplyMatrices(a: Matrix4, b: Matrix4): this {
    const ae = a.elements, be = b.elements, e = this.elements
    const a11 = ae[0]!, a12 = ae[4]!, a13 = ae[8]!, a14 = ae[12]!
    const a21 = ae[1]!, a22 = ae[5]!, a23 = ae[9]!, a24 = ae[13]!
    const a31 = ae[2]!, a32 = ae[6]!, a33 = ae[10]!, a34 = ae[14]!
    const a41 = ae[3]!, a42 = ae[7]!, a43 = ae[11]!, a44 = ae[15]!
    const b11 = be[0]!, b12 = be[4]!, b13 = be[8]!, b14 = be[12]!
    const b21 = be[1]!, b22 = be[5]!, b23 = be[9]!, b24 = be[13]!
    const b31 = be[2]!, b32 = be[6]!, b33 = be[10]!, b34 = be[14]!
    const b41 = be[3]!, b42 = be[7]!, b43 = be[11]!, b44 = be[15]!
    e[0] = a11 * b11 + a12 * b21 + a13 * b31 + a14 * b41
    e[4] = a11 * b12 + a12 * b22 + a13 * b32 + a14 * b42
    e[8] = a11 * b13 + a12 * b23 + a13 * b33 + a14 * b43
    e[12] = a11 * b14 + a12 * b24 + a13 * b34 + a14 * b44
    e[1] = a21 * b11 + a22 * b21 + a23 * b31 + a24 * b41
    e[5] = a21 * b12 + a22 * b22 + a23 * b32 + a24 * b42
    e[9] = a21 * b13 + a22 * b23 + a23 * b33 + a24 * b43
    e[13] = a21 * b14 + a22 * b24 + a23 * b34 + a24 * b44
    e[2] = a31 * b11 + a32 * b21 + a33 * b31 + a34 * b41
    e[6] = a31 * b12 + a32 * b22 + a33 * b32 + a34 * b42
    e[10] = a31 * b13 + a32 * b23 + a33 * b33 + a34 * b43
    e[14] = a31 * b14 + a32 * b24 + a33 * b34 + a34 * b44
    e[3] = a41 * b11 + a42 * b21 + a43 * b31 + a44 * b41
    e[7] = a41 * b12 + a42 * b22 + a43 * b32 + a44 * b42
    e[11] = a41 * b13 + a42 * b23 + a43 * b33 + a44 * b43
    e[15] = a41 * b14 + a42 * b24 + a43 * b34 + a44 * b44
    return this
  }

  determinant(): number {
    const e = this.elements
    const n11 = e[0]!, n12 = e[4]!, n13 = e[8]!, n14 = e[12]!
    const n21 = e[1]!, n22 = e[5]!, n23 = e[9]!, n24 = e[13]!
    const n31 = e[2]!, n32 = e[6]!, n33 = e[10]!, n34 = e[14]!
    const n41 = e[3]!, n42 = e[7]!, n43 = e[11]!, n44 = e[15]!
    return (
      n41 * (n14 * n23 * n32 - n13 * n24 * n32 - n14 * n22 * n33 + n12 * n24 * n33 + n13 * n22 * n34 - n12 * n23 * n34) +
      n42 * (n11 * n23 * n34 - n11 * n24 * n33 + n14 * n21 * n33 - n13 * n21 * n34 + n13 * n24 * n31 - n14 * n23 * n31) +
      n43 * (n11 * n24 * n32 - n11 * n22 * n34 - n14 * n21 * n32 + n12 * n21 * n34 + n14 * n22 * n31 - n12 * n24 * n31) +
      n44 * (-n13 * n22 * n31 - n11 * n23 * n32 + n11 * n22 * n33 + n13 * n21 * n32 - n12 * n21 * n33 + n12 * n23 * n31)
    )
  }

  /** In place. A matrix that has no inverse becomes all zeros. */
  invert(): this {
    const e = this.elements
    const n11 = e[0]!, n21 = e[1]!, n31 = e[2]!, n41 = e[3]!
    const n12 = e[4]!, n22 = e[5]!, n32 = e[6]!, n42 = e[7]!
    const n13 = e[8]!, n23 = e[9]!, n33 = e[10]!, n43 = e[11]!
    const n14 = e[12]!, n24 = e[13]!, n34 = e[14]!, n44 = e[15]!
    const t11 = n23 * n34 * n42 - n24 * n33 * n42 + n24 * n32 * n43 - n22 * n34 * n43 - n23 * n32 * n44 + n22 * n33 * n44
    const t12 = n14 * n33 * n42 - n13 * n34 * n42 - n14 * n32 * n43 + n12 * n34 * n43 + n13 * n32 * n44 - n12 * n33 * n44
    const t13 = n13 * n24 * n42 - n14 * n23 * n42 + n14 * n22 * n43 - n12 * n24 * n43 - n13 * n22 * n44 + n12 * n23 * n44
    const t14 = n14 * n23 * n32 - n13 * n24 * n32 - n14 * n22 * n33 + n12 * n24 * n33 + n13 * n22 * n34 - n12 * n23 * n34
    const det = n11 * t11 + n21 * t12 + n31 * t13 + n41 * t14
    if (det === 0) {
      e.fill(0)
      return this
    }
    const d = 1 / det
    e[0] = t11 * d
    e[1] = (n24 * n33 * n41 - n23 * n34 * n41 - n24 * n31 * n43 + n21 * n34 * n43 + n23 * n31 * n44 - n21 * n33 * n44) * d
    e[2] = (n22 * n34 * n41 - n24 * n32 * n41 + n24 * n31 * n42 - n21 * n34 * n42 - n22 * n31 * n44 + n21 * n32 * n44) * d
    e[3] = (n23 * n32 * n41 - n22 * n33 * n41 - n23 * n31 * n42 + n21 * n33 * n42 + n22 * n31 * n43 - n21 * n32 * n43) * d
    e[4] = t12 * d
    e[5] = (n13 * n34 * n41 - n14 * n33 * n41 + n14 * n31 * n43 - n11 * n34 * n43 - n13 * n31 * n44 + n11 * n33 * n44) * d
    e[6] = (n14 * n32 * n41 - n12 * n34 * n41 - n14 * n31 * n42 + n11 * n34 * n42 + n12 * n31 * n44 - n11 * n32 * n44) * d
    e[7] = (n12 * n33 * n41 - n13 * n32 * n41 + n13 * n31 * n42 - n11 * n33 * n42 - n12 * n31 * n43 + n11 * n32 * n43) * d
    e[8] = t13 * d
    e[9] = (n14 * n23 * n41 - n13 * n24 * n41 - n14 * n21 * n43 + n11 * n24 * n43 + n13 * n21 * n44 - n11 * n23 * n44) * d
    e[10] = (n12 * n24 * n41 - n14 * n22 * n41 + n14 * n21 * n42 - n11 * n24 * n42 - n12 * n21 * n44 + n11 * n22 * n44) * d
    e[11] = (n13 * n22 * n41 - n12 * n23 * n41 - n13 * n21 * n42 + n11 * n23 * n42 + n12 * n21 * n43 - n11 * n22 * n43) * d
    e[12] = t14 * d
    e[13] = (n13 * n24 * n31 - n14 * n23 * n31 + n14 * n21 * n33 - n11 * n24 * n33 - n13 * n21 * n34 + n11 * n23 * n34) * d
    e[14] = (n14 * n22 * n31 - n12 * n24 * n31 - n14 * n21 * n32 + n11 * n24 * n32 + n12 * n21 * n34 - n11 * n22 * n34) * d
    e[15] = (n12 * n23 * n31 - n13 * n22 * n31 + n13 * n21 * n32 - n11 * n23 * n32 - n12 * n21 * n33 + n11 * n22 * n33) * d
    return this
  }

  /** Columns `x`, `y` and `z` of the rotation, with no translation. */
  makeBasis(x: XYZ, y: XYZ, z: XYZ): this {
    const e = this.elements
    e[0] = x.x; e[1] = x.y; e[2] = x.z; e[3] = 0
    e[4] = y.x; e[5] = y.y; e[6] = y.z; e[7] = 0
    e[8] = z.x; e[9] = z.y; e[10] = z.z; e[11] = 0
    e[12] = 0; e[13] = 0; e[14] = 0; e[15] = 1
    return this
  }

  compose(position: XYZ, q: XYZW, scale: XYZ): this {
    const e = this.elements
    const x = q.x, y = q.y, z = q.z, w = q.w
    const x2 = x + x, y2 = y + y, z2 = z + z
    const xx = x * x2, xy = x * y2, xz = x * z2
    const yy = y * y2, yz = y * z2, zz = z * z2
    const wx = w * x2, wy = w * y2, wz = w * z2
    const sx = scale.x, sy = scale.y, sz = scale.z
    e[0] = (1 - (yy + zz)) * sx
    e[1] = (xy + wz) * sx
    e[2] = (xz - wy) * sx
    e[3] = 0
    e[4] = (xy - wz) * sy
    e[5] = (1 - (xx + zz)) * sy
    e[6] = (yz + wx) * sy
    e[7] = 0
    e[8] = (xz + wy) * sz
    e[9] = (yz - wx) * sz
    e[10] = (1 - (xx + yy)) * sz
    e[11] = 0
    e[12] = position.x
    e[13] = position.y
    e[14] = position.z
    e[15] = 1
    return this
  }

  /** Takes a matrix of translation, rotation and scale with no shear back apart. */
  decompose(position: XYZ, q: Quaternion, scale: XYZ): this {
    const e = this.elements
    let sx = Math.hypot(e[0]!, e[1]!, e[2]!)
    const sy = Math.hypot(e[4]!, e[5]!, e[6]!)
    const sz = Math.hypot(e[8]!, e[9]!, e[10]!)
    if (this.determinant() < 0) sx = -sx
    position.x = e[12]!
    position.y = e[13]!
    position.z = e[14]!
    const ix = 1 / sx, iy = 1 / sy, iz = 1 / sz
    const r = scratch.elements
    r[0] = e[0]! * ix; r[1] = e[1]! * ix; r[2] = e[2]! * ix
    r[4] = e[4]! * iy; r[5] = e[5]! * iy; r[6] = e[6]! * iy
    r[8] = e[8]! * iz; r[9] = e[9]! * iz; r[10] = e[10]! * iz
    q.setFromRotationMatrix(scratch)
    scale.x = sx
    scale.y = sy
    scale.z = sz
    return this
  }
}

const scratch = new Matrix4()
