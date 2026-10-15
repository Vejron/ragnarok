/**
 * The small value types nodes expose. Writes through the setters call `onChange`, which is how a node
 * learns its matrix is stale without recomputing every node every frame.
 */

const noop = () => {}

export class Vec3 {
  private _x: number
  private _y: number
  private _z: number
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

  /** Rotation of `angle` radians about the unit axis (x, y, z). */
  setFromAxisAngle(x: number, y: number, z: number, angle: number): this {
    const s = Math.sin(angle / 2)
    return this.set(x * s, y * s, z * s, Math.cos(angle / 2))
  }

  /** Euler angles applied in the order X then Y then Z (three's default 'XYZ'). */
  setFromEuler(ax: number, ay: number, az: number): this {
    const c1 = Math.cos(ax / 2), c2 = Math.cos(ay / 2), c3 = Math.cos(az / 2)
    const s1 = Math.sin(ax / 2), s2 = Math.sin(ay / 2), s3 = Math.sin(az / 2)
    return this.set(
      s1 * c2 * c3 + c1 * s2 * s3,
      c1 * s2 * c3 - s1 * c2 * s3,
      c1 * c2 * s3 + s1 * s2 * c3,
      c1 * c2 * c3 - s1 * s2 * s3,
    )
  }

  /** this = this × q: rotate by q first, then by this. */
  multiply(q: Quat): this {
    const ax = this._x, ay = this._y, az = this._z, aw = this._w
    const bx = q._x, by = q._y, bz = q._z, bw = q._w
    return this.set(
      ax * bw + aw * bx + ay * bz - az * by,
      ay * bw + aw * by + az * bx - ax * bz,
      az * bw + aw * bz + ax * by - ay * bx,
      aw * bw - ax * bx - ay * by - az * bz,
    )
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

  copy(c: Color): this {
    return this.set(c.r, c.g, c.b)
  }
}

export function srgbToLinear(c: number): number {
  return c < 0.04045 ? c * 0.0773993808 : Math.pow(c * 0.9478672986 + 0.0521327014, 2.4)
}
