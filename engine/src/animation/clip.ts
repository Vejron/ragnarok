export type TrackPath = 'translation' | 'rotation' | 'scale'
export type Interpolation = 'LINEAR' | 'STEP' | 'CUBICSPLINE'

/** Keyframes for one property of one node, found by the node's name. */
export interface Track {
  node: string
  path: TrackPath
  interpolation: Interpolation
  times: Float32Array
  /** 3 (translation, scale) or 4 (rotation, xyzw) values per key; for CUBICSPLINE, in-tangent, value, out-tangent. */
  values: Float32Array
}

/** A named animation: tracks that move nodes over `duration` seconds. */
export class AnimationClip {
  constructor(readonly name: string, readonly tracks: Track[], readonly duration: number) {}
}

/**
 * Samples a track at `time` (clamped to its keys) into `out` (3 or 4 numbers). Rotations are slerped.
 */
export function sampleTrack(track: Track, time: number, out: Float32Array) {
  const times = track.times
  const size = track.path === 'rotation' ? 4 : 3
  const cubic = track.interpolation === 'CUBICSPLINE'
  // Stride between keys, and where a key's value sits within it.
  const stride = cubic ? size * 3 : size
  const at = cubic ? size : 0
  const v = track.values
  const last = times.length - 1
  if (time <= times[0]! || last === 0) return read(v, at, size, out)
  if (time >= times[last]!) return read(v, last * stride + at, size, out)
  let lo = 0
  let hi = last
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1
    if (times[mid]! <= time) lo = mid
    else hi = mid
  }
  if (track.interpolation === 'STEP') return read(v, lo * stride + at, size, out)
  const t0 = times[lo]!
  const dt = times[hi]! - t0
  const t = dt > 0 ? (time - t0) / dt : 0
  const a = lo * stride + at
  const b = hi * stride + at
  if (cubic) {
    // Hermite spline with the stored tangents (scaled by the key interval).
    const t2 = t * t, t3 = t2 * t
    const h00 = 2 * t3 - 3 * t2 + 1, h10 = t3 - 2 * t2 + t, h01 = -2 * t3 + 3 * t2, h11 = t3 - t2
    for (let k = 0; k < size; k++) {
      out[k] = h00 * v[a + k]! + h10 * dt * v[a + size + k]! + h01 * v[b + k]! + h11 * dt * v[b - size + k]!
    }
    if (size === 4) normalize4(out)
    return out
  }
  if (size === 4) return slerp(v, a, b, t, out)
  for (let k = 0; k < 3; k++) out[k] = v[a + k]! + (v[b + k]! - v[a + k]!) * t
  return out
}

function read(v: Float32Array, o: number, size: number, out: Float32Array) {
  for (let k = 0; k < size; k++) out[k] = v[o + k]!
  return out
}

function slerp(v: Float32Array, a: number, b: number, t: number, out: Float32Array) {
  const ax = v[a]!, ay = v[a + 1]!, az = v[a + 2]!, aw = v[a + 3]!
  let bx = v[b]!, by = v[b + 1]!, bz = v[b + 2]!, bw = v[b + 3]!
  let cos = ax * bx + ay * by + az * bz + aw * bw
  if (cos < 0) {
    cos = -cos
    bx = -bx; by = -by; bz = -bz; bw = -bw
  }
  let ka = 1 - t
  let kb = t
  if (cos < 0.9995) {
    const angle = Math.acos(cos)
    const sin = Math.sin(angle)
    ka = Math.sin((1 - t) * angle) / sin
    kb = Math.sin(t * angle) / sin
  }
  out[0] = ax * ka + bx * kb
  out[1] = ay * ka + by * kb
  out[2] = az * ka + bz * kb
  out[3] = aw * ka + bw * kb
  return normalize4(out)
}

function normalize4(q: Float32Array) {
  const len = Math.hypot(q[0]!, q[1]!, q[2]!, q[3]!) || 1
  for (let k = 0; k < 4; k++) q[k] = q[k]! / len
  return q
}
