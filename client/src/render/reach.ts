/*
 * How far a rigid piece reaches, from a few of its corners rather than every vertex: a falling piece
 * is measured every frame (its lowest corner against the ground), and a section of wall can have
 * thousands of vertices, most of them shared by several triangles and nearly all of them inside its
 * outline. Pure, so the look worker works them out for the pieces it makes.
 */

/** Distinct corners up to which they are all kept: past that only the outermost are. */
const KEPT_CORNERS = 64
/** Directions the outermost corners are looked for along: their spacing bounds what is missed. */
const DIRECTIONS = 256

/** Unit directions spread evenly over the sphere (a Fibonacci lattice), x, y, z each. */
const directions: Float64Array = (() => {
  const out = new Float64Array(DIRECTIONS * 3)
  const golden = Math.PI * (3 - Math.sqrt(5))
  for (let i = 0; i < DIRECTIONS; i++) {
    const y = 1 - (2 * (i + 0.5)) / DIRECTIONS
    const r = Math.sqrt(1 - y * y)
    out[i * 3] = Math.cos(golden * i) * r
    out[i * 3 + 1] = y
    out[i * 3 + 2] = Math.sin(golden * i) * r
  }
  return out
})()

/**
 * The corners of `positions` (x, y, z a vertex) that a piece turned any way can rest on or reach
 * furthest with: every distinct one when there are few, else the outermost along each of a few hundred
 * directions. With 256 directions a corner missed lies within about 1% of the piece's size of one
 * kept, so the lowest corner of a 2 m section is found to a centimetre or two.
 */
export function cornersOf(positions: ArrayLike<number>): Float32Array {
  const seen = new Set<string>()
  const distinct: number[] = []
  for (let i = 0; i + 2 < positions.length; i += 3) {
    const [x, y, z] = [positions[i]!, positions[i + 1]!, positions[i + 2]!]
    const key = `${Math.round(x * 1000)},${Math.round(y * 1000)},${Math.round(z * 1000)}`
    if (seen.has(key)) continue
    seen.add(key)
    distinct.push(x, y, z)
  }
  if (distinct.length <= KEPT_CORNERS * 3) return Float32Array.from(distinct)
  const kept = new Set<number>()
  for (let d = 0; d < DIRECTIONS; d++) {
    const [dx, dy, dz] = [directions[d * 3]!, directions[d * 3 + 1]!, directions[d * 3 + 2]!]
    let [best, at] = [-Infinity, 0]
    for (let i = 0; i < distinct.length; i += 3) {
      const along = distinct[i]! * dx + distinct[i + 1]! * dy + distinct[i + 2]! * dz
      if (along > best) [best, at] = [along, i]
    }
    kept.add(at)
  }
  const out = new Float32Array(kept.size * 3)
  let k = 0
  for (const i of kept) {
    out[k++] = distinct[i]!
    out[k++] = distinct[i + 1]!
    out[k++] = distinct[i + 2]!
  }
  return out
}

/** The unit direction across `positions` (x, y, z a corner) that they spread least along: a slab's face normal. */
export function thinnest(positions: ArrayLike<number>): [number, number, number] {
  const n = positions.length / 3
  if (n < 3) return [0, 1, 0]
  const m = [0, 0, 0]
  for (let i = 0; i < positions.length; i += 3) for (let a = 0; a < 3; a++) m[a]! += positions[i + a]! / n
  // Their spread: the covariance matrix.
  const c = [0, 0, 0, 0, 0, 0, 0, 0, 0]
  for (let i = 0; i < positions.length; i += 3) {
    const d = [positions[i]! - m[0]!, positions[i + 1]! - m[1]!, positions[i + 2]! - m[2]!]
    for (let a = 0; a < 3; a++) for (let b = 0; b < 3; b++) c[a * 3 + b]! += d[a]! * d[b]!
  }
  // The least spread is the greatest of (trace - C): found by repeated multiplying.
  const trace = c[0]! + c[4]! + c[8]!
  const k = c.map((v, i) => (i % 4 === 0 ? trace - v : -v))
  let v = [0.3, 0.5, 0.8]
  for (let it = 0; it < 32; it++) {
    const w = [0, 1, 2].map((a) => k[a * 3]! * v[0]! + k[a * 3 + 1]! * v[1]! + k[a * 3 + 2]! * v[2]!)
    const l = Math.hypot(w[0]!, w[1]!, w[2]!) || 1
    v = w.map((x) => x / l)
  }
  return [v[0]!, v[1]!, v[2]!]
}

/** A piece's corners (`cornersOf`) and the way it is thinnest (`thinnest`, over every vertex). */
export interface Reach {
  corners: Float32Array
  thin: [number, number, number]
}

/** The `Reach` of a piece whose vertices are `positions`. */
export function reachOf(positions: ArrayLike<number>): Reach {
  return { corners: cornersOf(positions), thin: thinnest(positions) }
}
