/**
 * The small value types nodes expose. Writes through the setters call `onChange`, which is how a node
 * learns its matrix is stale without recomputing every node every frame.
 */

const noop = () => {}

export type EulerOrder = 'XYZ' | 'YXZ' | 'ZXY' | 'ZYX' | 'YZX' | 'XZY'

export class Vec3 {
  // `declare`: a real field would start out undefined, and V8 would box every number written to it (see `Vector3`).
  private declare _x: number
  private declare _y: number
  private declare _z: number
  /** @internal set by the owner that caches something derived from this vector. */
  onChange: () => void = noop

  constructor(x = 0, y = 0, z = 0) {
    this._x = x
    this._y = y
    this._z = z
  }

  get x() { return this._x }
  set x(v: number) { this._x = v; this.onChange() }
  get y() { return this._y }
  set y(v: number) { this._y = v; this.onChange() }
  get z() { return this._z }
  set z(v: number) { this._z = v; this.onChange() }

  set(x: number, y: number, z: number): this {
    this._x = x
    this._y = y
    this._z = z
    this.onChange()
    return this
  }

  copy(v: { x: number; y: number; z: number }): this {
    return this.set(v.x, v.y, v.z)
  }

  setScalar(s: number): this {
    return this.set(s, s, s)
  }

  clone(): Vec3 {
    return new Vec3(this._x, this._y, this._z)
  }

  add(v: XYZ): this {
    return this.set(this._x + v.x, this._y + v.y, this._z + v.z)
  }

  sub(v: XYZ): this {
    return this.set(this._x - v.x, this._y - v.y, this._z - v.z)
  }

  addScaledVector(v: XYZ, s: number): this {
    return this.set(this._x + v.x * s, this._y + v.y * s, this._z + v.z * s)
  }

  multiplyScalar(s: number): this {
    return this.set(this._x * s, this._y * s, this._z * s)
  }

  divideScalar(s: number): this {
    return this.multiplyScalar(1 / s)
  }

  lerp(v: XYZ, t: number): this {
    return this.set(this._x + (v.x - this._x) * t, this._y + (v.y - this._y) * t, this._z + (v.z - this._z) * t)
  }

  dot(v: XYZ): number {
    return this._x * v.x + this._y * v.y + this._z * v.z
  }

  lengthSq(): number {
    return this._x * this._x + this._y * this._y + this._z * this._z
  }

  length(): number {
    return Math.sqrt(this.lengthSq())
  }

  distanceTo(v: XYZ): number {
    return Math.hypot(this._x - v.x, this._y - v.y, this._z - v.z)
  }

  normalize(): this {
    return this.divideScalar(this.length() || 1)
  }

  applyQuaternion(q: XYZW): this {
    const x = this._x, y = this._y, z = this._z
    const ix = q.w * x + q.y * z - q.z * y
    const iy = q.w * y + q.z * x - q.x * z
    const iz = q.w * z + q.x * y - q.y * x
    const iw = -q.x * x - q.y * y - q.z * z
    return this.set(
      ix * q.w + iw * -q.x + iy * -q.z - iz * -q.y,
      iy * q.w + iw * -q.y + iz * -q.x - ix * -q.z,
      iz * q.w + iw * -q.z + ix * -q.y - iy * -q.x,
    )
  }

  fromArray(a: ArrayLike<number>, offset = 0): this {
    return this.set(a[offset]!, a[offset + 1]!, a[offset + 2]!)
  }

  toArray<T extends { [i: number]: number } = number[]>(out: T = [] as unknown as T, offset = 0): T {
    out[offset] = this._x
    out[offset + 1] = this._y
    out[offset + 2] = this._z
    return out
  }
}

/** Anything with x, y and z: a `Vec3` or a `Vector3`. */
export interface XYZ {
  x: number
  y: number
  z: number
}

export interface XYZW extends XYZ {
  w: number
}

export class Quat {
  private _x = 0
  private _y = 0
  private _z = 0
  private _w = 1
  /** @internal see Vec3.onChange. */
  onChange: () => void = noop

  get x() { return this._x }
  set x(v: number) { this._x = v; this.onChange() }
  get y() { return this._y }
  set y(v: number) { this._y = v; this.onChange() }
  get z() { return this._z }
  set z(v: number) { this._z = v; this.onChange() }
  get w() { return this._w }
  set w(v: number) { this._w = v; this.onChange() }

  set(x: number, y: number, z: number, w: number): this {
    this._x = x
    this._y = y
    this._z = z
    this._w = w
    this.onChange()
    return this
  }

  copy(q: { x: number; y: number; z: number; w: number }): this {
    return this.set(q.x, q.y, q.z, q.w)
  }

  identity(): this {
    return this.set(0, 0, 0, 1)
  }

  clone(): Quat {
    return new Quat().set(this._x, this._y, this._z, this._w)
  }

  /** Rotation of `angle` radians about the unit axis, given as a vector or as x, y, z. */
  setFromAxisAngle(axis: XYZ, angle: number): this
  setFromAxisAngle(x: number, y: number, z: number, angle: number): this
  setFromAxisAngle(a: XYZ | number, b: number, c?: number, d?: number): this {
    const [x, y, z, angle] = typeof a === 'number' ? [a, b, c!, d!] : [a.x, a.y, a.z, b]
    const s = Math.sin(angle / 2)
    return this.set(x * s, y * s, z * s, Math.cos(angle / 2))
  }

  /** The shortest rotation taking unit vector `from` to unit vector `to`. */
  setFromUnitVectors(from: XYZ, to: XYZ): this {
    let r = from.x * to.x + from.y * to.y + from.z * to.z + 1
    if (r < 1e-6) {
      // Opposite: any axis perpendicular to `from` will do.
      r = 0
      if (Math.abs(from.x) > Math.abs(from.z)) return this.set(-from.y, from.x, 0, r).normalize()
      return this.set(0, -from.z, from.y, r).normalize()
    }
    return this.set(
      from.y * to.z - from.z * to.y,
      from.z * to.x - from.x * to.z,
      from.x * to.y - from.y * to.x,
      r,
    ).normalize()
  }

  normalize(): this {
    const l = Math.hypot(this._x, this._y, this._z, this._w)
    if (l === 0) return this.set(0, 0, 0, 1)
    return this.set(this._x / l, this._y / l, this._z / l, this._w / l)
  }

  /** The inverse rotation (the conjugate, for a unit quaternion). */
  invert(): this {
    return this.set(-this._x, -this._y, -this._z, this._w)
  }

  dot(q: XYZW): number {
    return this._x * q.x + this._y * q.y + this._z * q.z + this._w * q.w
  }

  /** this = q × this: rotate by this first, then by q. */
  premultiply(q: XYZW): this {
    return this.multiplyQuaternions(q, this)
  }

  /** Spherical interpolation towards `q`, `t` from 0 (this) to 1 (q). */
  slerp(q: XYZW, t: number): this {
    if (t === 0) return this
    if (t === 1) return this.copy(q)
    const x = this._x, y = this._y, z = this._z, w = this._w
    let cos = w * q.w + x * q.x + y * q.y + z * q.z
    let qx = q.x, qy = q.y, qz = q.z, qw = q.w
    if (cos < 0) {
      qx = -qx
      qy = -qy
      qz = -qz
      qw = -qw
      cos = -cos
    }
    if (cos >= 1) return this
    const sin2 = 1 - cos * cos
    if (sin2 <= Number.EPSILON) {
      const s = 1 - t
      return this.set(s * x + t * qx, s * y + t * qy, s * z + t * qz, s * w + t * qw).normalize()
    }
    const sin = Math.sqrt(sin2)
    const half = Math.atan2(sin, cos)
    const a = Math.sin((1 - t) * half) / sin
    const b = Math.sin(t * half) / sin
    return this.set(x * a + qx * b, y * a + qy * b, z * a + qz * b, w * a + qw * b)
  }

  /** Rotation from a rotation matrix's upper 3×3 (column-major elements). */
  setFromRotationMatrix(m: ArrayLike<number>): this {
    const m11 = m[0]!, m12 = m[4]!, m13 = m[8]!
    const m21 = m[1]!, m22 = m[5]!, m23 = m[9]!
    const m31 = m[2]!, m32 = m[6]!, m33 = m[10]!
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

  fromArray(a: ArrayLike<number>, offset = 0): this {
    return this.set(a[offset]!, a[offset + 1]!, a[offset + 2]!, a[offset + 3]!)
  }

  toArray<T extends { [i: number]: number } = number[]>(out: T = [] as unknown as T, offset = 0): T {
    out[offset] = this._x
    out[offset + 1] = this._y
    out[offset + 2] = this._z
    out[offset + 3] = this._w
    return out
  }

  /** Euler angles in the given `order` (by default 'XYZ': the matrix is Rx · Ry · Rz). */
  setFromEuler(ax: number, ay: number, az: number, order: EulerOrder = 'XYZ'): this {
    const c1 = Math.cos(ax / 2), c2 = Math.cos(ay / 2), c3 = Math.cos(az / 2)
    const s1 = Math.sin(ax / 2), s2 = Math.sin(ay / 2), s3 = Math.sin(az / 2)
    switch (order) {
      case 'XYZ':
        return this.set(s1 * c2 * c3 + c1 * s2 * s3, c1 * s2 * c3 - s1 * c2 * s3, c1 * c2 * s3 + s1 * s2 * c3, c1 * c2 * c3 - s1 * s2 * s3)
      case 'YXZ':
        return this.set(s1 * c2 * c3 + c1 * s2 * s3, c1 * s2 * c3 - s1 * c2 * s3, c1 * c2 * s3 - s1 * s2 * c3, c1 * c2 * c3 + s1 * s2 * s3)
      case 'ZXY':
        return this.set(s1 * c2 * c3 - c1 * s2 * s3, c1 * s2 * c3 + s1 * c2 * s3, c1 * c2 * s3 + s1 * s2 * c3, c1 * c2 * c3 - s1 * s2 * s3)
      case 'ZYX':
        return this.set(s1 * c2 * c3 - c1 * s2 * s3, c1 * s2 * c3 + s1 * c2 * s3, c1 * c2 * s3 - s1 * s2 * c3, c1 * c2 * c3 + s1 * s2 * s3)
      case 'YZX':
        return this.set(s1 * c2 * c3 + c1 * s2 * s3, c1 * s2 * c3 + s1 * c2 * s3, c1 * c2 * s3 - s1 * s2 * c3, c1 * c2 * c3 - s1 * s2 * s3)
      case 'XZY':
        return this.set(s1 * c2 * c3 - c1 * s2 * s3, c1 * s2 * c3 - s1 * c2 * s3, c1 * c2 * s3 + s1 * s2 * c3, c1 * c2 * c3 + s1 * s2 * s3)
    }
  }

  /** this = this × q: rotate by q first, then by this. */
  multiply(q: XYZW): this {
    return this.multiplyQuaternions(this, q)
  }

  private multiplyQuaternions(a: XYZW, b: XYZW): this {
    const ax = a.x, ay = a.y, az = a.z, aw = a.w
    const bx = b.x, by = b.y, bz = b.z, bw = b.w
    return this.set(
      ax * bw + aw * bx + ay * bz - az * by,
      ay * bw + aw * by + az * bx - ax * bz,
      az * bw + aw * bz + ax * by - ay * bx,
      aw * bw - ax * bx - ay * by - az * bz,
    )
  }
}

/**
 * Rotation as angles about the axes, in `order`. A node's `rotation` is a view of its `quaternion`: writing
 * one updates the other. Angles are extracted from the quaternion the first time they are read after it changed.
 */
export class Euler {
  private _x = 0
  private _y = 0
  private _z = 0
  order: EulerOrder = 'XYZ'
  /** @internal set by the owning node, to push the angles into its quaternion. */
  onChange: () => void = noop

  get x() { return this._x }
  set x(v: number) { this._x = v; this.onChange() }
  get y() { return this._y }
  set y(v: number) { this._y = v; this.onChange() }
  get z() { return this._z }
  set z(v: number) { this._z = v; this.onChange() }

  set(x: number, y: number, z: number, order: EulerOrder = this.order): this {
    this._x = x
    this._y = y
    this._z = z
    this.order = order
    this.onChange()
    return this
  }

  copy(e: { x: number; y: number; z: number; order?: EulerOrder }): this {
    return this.set(e.x, e.y, e.z, e.order ?? this.order)
  }

  /** @internal Takes angles from a quaternion without notifying the owner. */
  fromQuaternion(q: XYZW) {
    const x = q.x, y = q.y, z = q.z, w = q.w
    const x2 = x + x, y2 = y + y, z2 = z + z
    const xx = x * x2, xy = x * y2, xz = x * z2
    const yy = y * y2, yz = y * z2, zz = z * z2
    const wx = w * x2, wy = w * y2, wz = w * z2
    const m11 = 1 - (yy + zz), m12 = xy - wz, m13 = xz + wy
    const m21 = xy + wz, m22 = 1 - (xx + zz), m23 = yz - wx
    const m31 = xz - wy, m32 = yz + wx, m33 = 1 - (xx + yy)
    const clamp = (v: number) => Math.min(1, Math.max(-1, v))
    const EPS = 0.9999999
    switch (this.order) {
      case 'XYZ':
        this._y = Math.asin(clamp(m13))
        if (Math.abs(m13) < EPS) { this._x = Math.atan2(-m23, m33); this._z = Math.atan2(-m12, m11) } else { this._x = Math.atan2(m32, m22); this._z = 0 }
        break
      case 'YXZ':
        this._x = Math.asin(-clamp(m23))
        if (Math.abs(m23) < EPS) { this._y = Math.atan2(m13, m33); this._z = Math.atan2(m21, m22) } else { this._y = Math.atan2(-m31, m11); this._z = 0 }
        break
      case 'ZXY':
        this._x = Math.asin(clamp(m32))
        if (Math.abs(m32) < EPS) { this._y = Math.atan2(-m31, m33); this._z = Math.atan2(-m12, m22) } else { this._y = 0; this._z = Math.atan2(m21, m11) }
        break
      case 'ZYX':
        this._y = Math.asin(-clamp(m31))
        if (Math.abs(m31) < EPS) { this._x = Math.atan2(m32, m33); this._z = Math.atan2(m21, m11) } else { this._x = 0; this._z = Math.atan2(-m12, m22) }
        break
      case 'YZX':
        this._z = Math.asin(clamp(m21))
        if (Math.abs(m21) < EPS) { this._x = Math.atan2(-m23, m22); this._y = Math.atan2(-m31, m11) } else { this._x = 0; this._y = Math.atan2(m13, m33) }
        break
      case 'XZY':
        this._z = Math.asin(-clamp(m12))
        if (Math.abs(m12) < EPS) { this._x = Math.atan2(m32, m22); this._y = Math.atan2(m13, m11) } else { this._x = Math.atan2(-m23, m33); this._y = 0 }
        break
    }
  }
}

/** A linear RGB colour. Hex input is sRGB, as designers pick it, converted to the linear light shaders work in. */
export class Color {
  constructor(public r = 1, public g = 1, public b = 1) {}

  static hex(hex: number): Color {
    return new Color().setHex(hex)
  }

  setHex(hex: number): this {
    this.r = srgbToLinear(((hex >> 16) & 255) / 255)
    this.g = srgbToLinear(((hex >> 8) & 255) / 255)
    this.b = srgbToLinear((hex & 255) / 255)
    return this
  }

  set(r: number, g: number, b: number): this {
    this.r = r
    this.g = g
    this.b = b
    return this
  }

  copy(c: { r: number; g: number; b: number }): this {
    return this.set(c.r, c.g, c.b)
  }

  clone(): Color {
    return new Color(this.r, this.g, this.b)
  }

  setScalar(s: number): this {
    return this.set(s, s, s)
  }

  multiplyScalar(s: number): this {
    return this.set(this.r * s, this.g * s, this.b * s)
  }

  lerp(c: { r: number; g: number; b: number }, t: number): this {
    return this.set(this.r + (c.r - this.r) * t, this.g + (c.g - this.g) * t, this.b + (c.b - this.b) * t)
  }

  /** The colour as sRGB hex, as `setHex` takes it. */
  getHex(): number {
    const channel = (v: number) => Math.round(Math.min(1, Math.max(0, linearToSrgb(v))) * 255)
    return (channel(this.r) << 16) | (channel(this.g) << 8) | channel(this.b)
  }
}

export function linearToSrgb(c: number): number {
  return c < 0.0031308 ? c * 12.92 : 1.055 * Math.pow(c, 1 / 2.4) - 0.055
}

export function srgbToLinear(c: number): number {
  return c < 0.04045 ? c * 0.0773993808 : Math.pow(c * 0.9478672986 + 0.0521327014, 2.4)
}
