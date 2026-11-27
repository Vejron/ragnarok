/*
 * Small geometry helpers shared by what draws buildings from their looks: windows, eaves, glass and
 * the fracture of a hurt building. Pure, so they run in the tile worker too.
 */

export type P = [number, number, number]
export type Poly = P[]
export type RGB = readonly [number, number, number]

const EPS = 1e-4

/** A number in [0, 1) from a point, the same on every client. */
export function hash(x: number, y: number, z: number, salt: number): number {
  const a = Math.sin(x * 127.1 + y * 311.7 + z * 74.7 + salt * 19.19) * 43758.5453
  return a - Math.floor(a)
}

/** The key of a point, to the millimetre. */
export function keyOf(p: P): string {
  return `${Math.round(p[0] * 1000)},${Math.round(p[1] * 1000)},${Math.round(p[2] * 1000)}`
}

/** The key of the edge `a b`, the same either way round. */
export function edgeKey(a: P, b: P): string {
  const [ka, kb] = [keyOf(a), keyOf(b)]
  return ka < kb ? `${ka}|${kb}` : `${kb}|${ka}`
}

/** `poly` cut by the plane `axis = at` into the part below and the part above; either may be empty. */
export function split(poly: Poly, axis: number, at: number): [Poly, Poly] {
  const below: Poly = []
  const above: Poly = []
  const side = (p: P) => (p[axis]! - at > EPS ? 1 : p[axis]! - at < -EPS ? -1 : 0)
  let allOn = true
  for (let i = 0; i < poly.length; i++) {
    const a = poly[i]!
    const b = poly[(i + 1) % poly.length]!
    const [sa, sb] = [side(a), side(b)]
    if (sa !== 0) allOn = false
    if (sa <= 0) below.push(a)
    if (sa >= 0) above.push(a)
    if (sa * sb < 0) {
      const t = (at - a[axis]!) / (b[axis]! - a[axis]!)
      const cut: P = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
      cut[axis] = at
      below.push(cut)
      above.push([...cut] as P)
    }
  }
  if (allOn) return [[], poly]
  return [below.length >= 3 ? below : [], above.length >= 3 ? above : []]
}

/** The unit normal `poly` winds towards, `(c - b) × (a - b)` of its first corners, as the soup draws it. */
export function normalOf(poly: Poly): P {
  let [nx, ny, nz] = [0, 0, 0]
  // Newell's method: sound for any planar polygon, slivers included.
  for (let i = 0; i < poly.length; i++) {
    const a = poly[i]!
    const b = poly[(i + 1) % poly.length]!
    nx += (a[1] - b[1]) * (a[2] + b[2])
    ny += (a[2] - b[2]) * (a[0] + b[0])
    nz += (a[0] - b[0]) * (a[1] + b[1])
  }
  const l = Math.hypot(nx, ny, nz) || 1
  return [nx / l, ny / l, nz / l]
}

export function centreOf(poly: Poly): P {
  const c: P = [0, 0, 0]
  for (const p of poly) for (let a = 0; a < 3; a++) c[a]! += p[a]! / poly.length
  return c
}

export const add = (a: readonly number[], b: readonly number[], k = 1): P => [a[0]! + b[0]! * k, a[1]! + b[1]! * k, a[2]! + b[2]! * k]
export const sub = (a: readonly number[], b: readonly number[]): P => [a[0]! - b[0]!, a[1]! - b[1]!, a[2]! - b[2]!]
export const dot = (a: readonly number[], b: readonly number[]) => a[0]! * b[0]! + a[1]! * b[1]! + a[2]! * b[2]!
export const cross = (a: readonly number[], b: readonly number[]): P => [a[1]! * b[2]! - a[2]! * b[1]!, a[2]! * b[0]! - a[0]! * b[2]!, a[0]! * b[1]! - a[1]! * b[0]!]
export const unit = (a: readonly number[]): P => {
  const l = Math.hypot(a[0]!, a[1]!, a[2]!) || 1
  return [a[0]! / l, a[1]! / l, a[2]! / l]
}
export const lerp = (a: readonly number[], b: readonly number[], t: number): P => [a[0]! + (b[0]! - a[0]!) * t, a[1]! + (b[1]! - a[1]!) * t, a[2]! + (b[2]! - a[2]!) * t]

/** `poly` cut by the plane `n · p = d` into the part behind (`n · p < d`) and the part in front; either may be empty. */
export function splitPlane(poly: Poly, n: P, d: number): [Poly, Poly] {
  const behind: Poly = []
  const front: Poly = []
  const side = (p: P) => (dot(p, n) - d > EPS ? 1 : dot(p, n) - d < -EPS ? -1 : 0)
  let allOn = true
  for (let i = 0; i < poly.length; i++) {
    const a = poly[i]!
    const b = poly[(i + 1) % poly.length]!
    const [sa, sb] = [side(a), side(b)]
    if (sa !== 0) allOn = false
    if (sa <= 0) behind.push(a)
    if (sa >= 0) front.push(a)
    if (sa * sb < 0) {
      const t = (d - dot(a, n)) / (dot(b, n) - dot(a, n))
      const cut = lerp(a, b, t)
      behind.push(cut)
      front.push([...cut] as P)
    }
  }
  if (allOn) return [[], poly]
  return [behind.length >= 3 ? behind : [], front.length >= 3 ? front : []]
}

/**
 * `ring` with each of `holes` let into it (points `[x, z]`, each either way round): one ring that
 * runs round `ring`, in along a cut to a hole, round the hole the other way and back out along the
 * same cut, so that `triangulate` leaves every hole open. A cut is the shortest line from a corner
 * of a hole to a corner of the ring that crosses no side of either. A hole no cut reaches is left
 * filled. As `with_yards` in `interior.rs`.
 */
export function joinHoles(ring: readonly (readonly [number, number])[], holes: readonly (readonly (readonly [number, number])[])[]): (readonly [number, number])[] {
  type Q = readonly [number, number]
  const twice = (r: readonly Q[]) => r.reduce((sum, [ax, az], i) => sum + ax * r[(i + 1) % r.length]![1] - r[(i + 1) % r.length]![0] * az, 0)
  let out: Q[] = twice(ring) < 0 ? [...ring].reverse() : [...ring]
  const left: Q[][] = holes.filter((hole) => hole.length >= 3).map((hole) => (twice(hole) > 0 ? [...hole].reverse() : [...hole]))
  const cross = (o: Q, s: Q, t: Q) => (s[0] - o[0]) * (t[1] - o[1]) - (s[1] - o[1]) * (t[0] - o[0])
  const on = (o: Q, s: Q, t: Q) => t[0] >= Math.min(o[0], s[0]) - 1e-9 && t[0] <= Math.max(o[0], s[0]) + 1e-9 && t[1] >= Math.min(o[1], s[1]) - 1e-9 && t[1] <= Math.max(o[1], s[1]) + 1e-9
  const meet = (p: Q, q: Q, a: Q, b: Q) => {
    const [d1, d2, d3, d4] = [cross(a, b, p), cross(a, b, q), cross(p, q, a), cross(p, q, b)]
    if (((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0))) return true
    return (Math.abs(d1) < 1e-9 && on(a, b, p)) || (Math.abs(d2) < 1e-9 && on(a, b, q)) || (Math.abs(d3) < 1e-9 && on(p, q, a)) || (Math.abs(d4) < 1e-9 && on(p, q, b))
  }
  const same = (a: Q, b: Q) => a[0] === b[0] && a[1] === b[1]
  while (left.length > 0) {
    let best: [number, number, number, number] | null = null
    left.forEach((hole, y) => hole.forEach((h, j) => out.forEach((p, i) => {
      const length = (p[0] - h[0]) ** 2 + (p[1] - h[1]) ** 2
      if (length === 0 || (best && length >= best[0])) return
      // A side that ends at the cut's own end is not crossed by it, unless it runs along it.
      const crosses = (edges: readonly Q[]) => edges.some((a, k) => {
        const b = edges[(k + 1) % edges.length]!
        const [sa, sb] = [same(a, p) || same(a, h), same(b, p) || same(b, h)]
        if (sa && sb) return false
        if (sa) return meet(p, h, b, b)
        if (sb) return meet(p, h, a, a)
        return meet(p, h, a, b)
      })
      if (!crosses(out) && !left.some(crosses)) best = [length, y, j, i]
    })))
    if (!best) break
    const [, y, j, i] = best as [number, number, number, number]
    const hole = left.splice(y, 1)[0]!
    out = [...out.slice(0, i + 1), ...Array.from({ length: hole.length + 1 }, (_, k) => hole[(j + k) % hole.length]!), ...out.slice(i)]
  }
  return out
}

/**
 * A simple polygon in the plane (points `[x, z]`, either way round) as triangles, by ear clipping:
 * indices into `ring`, three a triangle, wound as the ring is. A ring it cannot clip falls back to a fan.
 */
export function triangulate(ring: readonly (readonly [number, number])[]): number[] {
  const n = ring.length
  if (n < 3) return []
  let area = 0
  for (let i = 0; i < n; i++) {
    const [ax, az] = ring[i]!
    const [bx, bz] = ring[(i + 1) % n]!
    area += ax * bz - bx * az
  }
  const sign = area >= 0 ? 1 : -1
  const left = [...Array(n).keys()]
  const out: number[] = []
  const cross2 = (a: number, b: number, c: number) => {
    const [ax, az] = ring[a]!
    const [bx, bz] = ring[b]!
    const [cx, cz] = ring[c]!
    return (bx - ax) * (cz - az) - (bz - az) * (cx - ax)
  }
  const inside = (a: number, b: number, c: number, p: number) => {
    const [s1, s2, s3] = [cross2(a, b, p) * sign, cross2(b, c, p) * sign, cross2(c, a, p) * sign]
    return s1 >= -1e-9 && s2 >= -1e-9 && s3 >= -1e-9
  }
  let guard = 0
  while (left.length > 3 && guard++ < n * n) {
    let clipped = false
    for (let i = 0; i < left.length; i++) {
      const [a, b, c] = [left[(i + left.length - 1) % left.length]!, left[i]!, left[(i + 1) % left.length]!]
      if (cross2(a, b, c) * sign <= 1e-9) continue
      // A corner at the same place as one of the ear's own is the other end of a cut to a hole (`joinHoles`), not in its way.
      const apart = (p: number, q: number) => ring[p]![0] !== ring[q]![0] || ring[p]![1] !== ring[q]![1]
      if (left.some((p) => apart(p, a) && apart(p, b) && apart(p, c) && inside(a, b, c, p))) continue
      out.push(a, b, c)
      left.splice(i, 1)
      clipped = true
      break
    }
    if (!clipped) break
  }
  if (left.length === 3) out.push(left[0]!, left[1]!, left[2]!)
  else for (let i = 1; i + 1 < left.length; i++) out.push(left[0]!, left[i]!, left[i + 1]!)
  return out
}
