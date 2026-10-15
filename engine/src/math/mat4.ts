/**
 * 4×4 matrices as column-major Float32Arrays, the layout WGSL's mat4x4f expects. Every function writes
 * into `out` and returns it, so per-frame code allocates nothing. `out` may alias an input.
 */
export type Mat4 = Float32Array

export function create(): Mat4 {
  return identity(new Float32Array(16))
}

export function identity(out: Mat4): Mat4 {
  out.fill(0)
  out[0] = out[5] = out[10] = out[15] = 1
  return out
}

export function copy(out: Mat4, a: ArrayLike<number>): Mat4 {
  for (let i = 0; i < 16; i++) out[i] = a[i]!
  return out
}

export function multiply(out: Mat4, a: Mat4, b: Mat4): Mat4 {
  const a00 = a[0]!, a01 = a[1]!, a02 = a[2]!, a03 = a[3]!
  const a10 = a[4]!, a11 = a[5]!, a12 = a[6]!, a13 = a[7]!
  const a20 = a[8]!, a21 = a[9]!, a22 = a[10]!, a23 = a[11]!
  const a30 = a[12]!, a31 = a[13]!, a32 = a[14]!, a33 = a[15]!
  for (let c = 0; c < 4; c++) {
    const b0 = b[c * 4]!, b1 = b[c * 4 + 1]!, b2 = b[c * 4 + 2]!, b3 = b[c * 4 + 3]!
    out[c * 4] = a00 * b0 + a10 * b1 + a20 * b2 + a30 * b3
    out[c * 4 + 1] = a01 * b0 + a11 * b1 + a21 * b2 + a31 * b3
    out[c * 4 + 2] = a02 * b0 + a12 * b1 + a22 * b2 + a32 * b3
    out[c * 4 + 3] = a03 * b0 + a13 * b1 + a23 * b2 + a33 * b3
  }
  return out
}

/** Translation × rotation (unit quaternion x, y, z, w) × scale. */
export function compose(
  out: Mat4,
  px: number, py: number, pz: number,
  qx: number, qy: number, qz: number, qw: number,
  sx: number, sy: number, sz: number,
): Mat4 {
  const x2 = qx + qx, y2 = qy + qy, z2 = qz + qz
  const xx = qx * x2, xy = qx * y2, xz = qx * z2
  const yy = qy * y2, yz = qy * z2, zz = qz * z2
  const wx = qw * x2, wy = qw * y2, wz = qw * z2
  out[0] = (1 - (yy + zz)) * sx
  out[1] = (xy + wz) * sx
  out[2] = (xz - wy) * sx
  out[3] = 0
  out[4] = (xy - wz) * sy
  out[5] = (1 - (xx + zz)) * sy
  out[6] = (yz + wx) * sy
  out[7] = 0
  out[8] = (xz + wy) * sz
  out[9] = (yz - wx) * sz
  out[10] = (1 - (xx + yy)) * sz
  out[11] = 0
  out[12] = px
  out[13] = py
  out[14] = pz
  out[15] = 1
  return out
}

/** Writes the inverse of `a`; returns null (leaving `out` untouched) when `a` is singular. */
export function invert(out: Mat4, a: Mat4): Mat4 | null {
  const a00 = a[0]!, a01 = a[1]!, a02 = a[2]!, a03 = a[3]!
  const a10 = a[4]!, a11 = a[5]!, a12 = a[6]!, a13 = a[7]!
  const a20 = a[8]!, a21 = a[9]!, a22 = a[10]!, a23 = a[11]!
  const a30 = a[12]!, a31 = a[13]!, a32 = a[14]!, a33 = a[15]!
  const b00 = a00 * a11 - a01 * a10
  const b01 = a00 * a12 - a02 * a10
  const b02 = a00 * a13 - a03 * a10
  const b03 = a01 * a12 - a02 * a11
  const b04 = a01 * a13 - a03 * a11
  const b05 = a02 * a13 - a03 * a12
  const b06 = a20 * a31 - a21 * a30
  const b07 = a20 * a32 - a22 * a30
  const b08 = a20 * a33 - a23 * a30
  const b09 = a21 * a32 - a22 * a31
  const b10 = a21 * a33 - a23 * a31
  const b11 = a22 * a33 - a23 * a32
  let det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06
  if (!det) return null
  det = 1 / det
  out[0] = (a11 * b11 - a12 * b10 + a13 * b09) * det
  out[1] = (a02 * b10 - a01 * b11 - a03 * b09) * det
  out[2] = (a31 * b05 - a32 * b04 + a33 * b03) * det
  out[3] = (a22 * b04 - a21 * b05 - a23 * b03) * det
  out[4] = (a12 * b08 - a10 * b11 - a13 * b07) * det
  out[5] = (a00 * b11 - a02 * b08 + a03 * b07) * det
  out[6] = (a32 * b02 - a30 * b05 - a33 * b01) * det
  out[7] = (a20 * b05 - a22 * b02 + a23 * b01) * det
  out[8] = (a10 * b10 - a11 * b08 + a13 * b06) * det
  out[9] = (a01 * b08 - a00 * b10 - a03 * b06) * det
  out[10] = (a30 * b04 - a31 * b02 + a33 * b00) * det
  out[11] = (a21 * b02 - a20 * b04 - a23 * b00) * det
  out[12] = (a11 * b07 - a10 * b09 - a12 * b06) * det
  out[13] = (a00 * b09 - a01 * b07 + a02 * b06) * det
  out[14] = (a31 * b01 - a30 * b03 - a32 * b00) * det
  out[15] = (a20 * b03 - a21 * b01 + a22 * b00) * det
  return out
}

/**
 * A perspective projection with reversed depth: the near plane maps to 1 and the far plane to 0. Floats
 * hold far more precision near zero, so reversed depth keeps distant geometry from z-fighting.
 */
export function perspectiveReversed(out: Mat4, fovY: number, aspect: number, near: number, far: number): Mat4 {
  const f = 1 / Math.tan(fovY / 2)
  out.fill(0)
  out[0] = f / aspect
  out[5] = f
  out[10] = near / (far - near)
  out[11] = -1
  out[14] = (far * near) / (far - near)
  return out
}

/** An orthographic projection to WebGPU's 0..1 depth range, near at 0. */
export function orthographic(out: Mat4, left: number, right: number, bottom: number, top: number, near: number, far: number): Mat4 {
  out.fill(0)
  out[0] = 2 / (right - left)
  out[5] = 2 / (top - bottom)
  out[10] = 1 / (near - far)
  out[12] = (left + right) / (left - right)
  out[13] = (bottom + top) / (bottom - top)
  out[14] = near / (near - far)
  out[15] = 1
  return out
}

/** A view matrix looking from `eye` at `target`, `up` roughly up. */
export function lookAt(
  out: Mat4,
  ex: number, ey: number, ez: number,
  tx: number, ty: number, tz: number,
  ux = 0, uy = 1, uz = 0,
): Mat4 {
  let zx = ex - tx, zy = ey - ty, zz = ez - tz
  let len = Math.hypot(zx, zy, zz) || 1
  zx /= len; zy /= len; zz /= len
  let xx = uy * zz - uz * zy, xy = uz * zx - ux * zz, xz = ux * zy - uy * zx
  len = Math.hypot(xx, xy, xz)
  if (!len) {
    // Looking straight along `up`: any perpendicular will do.
    xx = 1; xy = 0; xz = 0
  } else {
    xx /= len; xy /= len; xz /= len
  }
  const yx = zy * xz - zz * xy, yy = zz * xx - zx * xz, yz = zx * xy - zy * xx
  out[0] = xx; out[1] = yx; out[2] = zx; out[3] = 0
  out[4] = xy; out[5] = yy; out[6] = zy; out[7] = 0
  out[8] = xz; out[9] = yz; out[10] = zz; out[11] = 0
  out[12] = -(xx * ex + xy * ey + xz * ez)
  out[13] = -(yx * ex + yy * ey + yz * ez)
  out[14] = -(zx * ex + zy * ey + zz * ez)
  out[15] = 1
  return out
}

/** The largest axis scale in `m`, for growing a bounding sphere by a transform. */
export function maxScale(m: Mat4): number {
  const sx = m[0]! * m[0]! + m[1]! * m[1]! + m[2]! * m[2]!
  const sy = m[4]! * m[4]! + m[5]! * m[5]! + m[6]! * m[6]!
  const sz = m[8]! * m[8]! + m[9]! * m[9]! + m[10]! * m[10]!
  return Math.sqrt(Math.max(sx, sy, sz))
}

/**
 * Splits `m` into translation, unit rotation quaternion and scale (assuming no shear). Each output is an
 * array-like of 3, 4 and 3 numbers.
 */
export function decompose(m: ArrayLike<number>, position: { [i: number]: number }, rotation: { [i: number]: number }, scale: { [i: number]: number }) {
  let sx = Math.hypot(m[0]!, m[1]!, m[2]!)
  const sy = Math.hypot(m[4]!, m[5]!, m[6]!)
  const sz = Math.hypot(m[8]!, m[9]!, m[10]!)
  // A mirrored basis shows as a negative determinant; fold it into x.
  const det = m[0]! * (m[5]! * m[10]! - m[6]! * m[9]!) - m[4]! * (m[1]! * m[10]! - m[2]! * m[9]!) + m[8]! * (m[1]! * m[6]! - m[2]! * m[5]!)
  if (det < 0) sx = -sx
  position[0] = m[12]!
  position[1] = m[13]!
  position[2] = m[14]!
  scale[0] = sx
  scale[1] = sy
  scale[2] = sz
  const ix = 1 / sx, iy = 1 / sy, iz = 1 / sz
  const r = [m[0]! * ix, m[1]! * ix, m[2]! * ix, 0, m[4]! * iy, m[5]! * iy, m[6]! * iy, 0, m[8]! * iz, m[9]! * iz, m[10]! * iz, 0]
  const m11 = r[0]!, m12 = r[4]!, m13 = r[8]!
  const m21 = r[1]!, m22 = r[5]!, m23 = r[9]!
  const m31 = r[2]!, m32 = r[6]!, m33 = r[10]!
  const trace = m11 + m22 + m33
  if (trace > 0) {
    const s = 0.5 / Math.sqrt(trace + 1)
    rotation[0] = (m32 - m23) * s
    rotation[1] = (m13 - m31) * s
    rotation[2] = (m21 - m12) * s
    rotation[3] = 0.25 / s
  } else if (m11 > m22 && m11 > m33) {
    const s = 2 * Math.sqrt(1 + m11 - m22 - m33)
    rotation[0] = 0.25 * s
    rotation[1] = (m12 + m21) / s
    rotation[2] = (m13 + m31) / s
    rotation[3] = (m32 - m23) / s
  } else if (m22 > m33) {
    const s = 2 * Math.sqrt(1 + m22 - m11 - m33)
    rotation[0] = (m12 + m21) / s
    rotation[1] = 0.25 * s
    rotation[2] = (m23 + m32) / s
    rotation[3] = (m13 - m31) / s
  } else {
    const s = 2 * Math.sqrt(1 + m33 - m11 - m22)
    rotation[0] = (m13 + m31) / s
    rotation[1] = (m23 + m32) / s
    rotation[2] = 0.25 * s
    rotation[3] = (m21 - m12) / s
  }
}
