import { InstancedMesh, Mesh, SkinnedMesh, mat4, type Node } from '@ragnarok/engine'

type V = { x: number; y: number; z: number }

/** The nearest point of a triangle list to a query, with the outward normal of the triangle it is on. */
export interface Nearest {
  point: [number, number, number]
  normal: [number, number, number]
  /** Squared distance from the query. */
  distance2: number
}

/**
 * The point of triangle `a b c` nearest to `p` (Ericson, "Real-Time Collision Detection" 5.1.5): by
 * which of its seven regions `p` projects into, a corner, an edge or the face.
 */
function closestOnTriangle(p: number[], a: number[], b: number[], c: number[], out: number[]) {
  const ab = [b[0]! - a[0]!, b[1]! - a[1]!, b[2]! - a[2]!]
  const ac = [c[0]! - a[0]!, c[1]! - a[1]!, c[2]! - a[2]!]
  const ap = [p[0]! - a[0]!, p[1]! - a[1]!, p[2]! - a[2]!]
  const dot = (u: number[], v: number[]) => u[0]! * v[0]! + u[1]! * v[1]! + u[2]! * v[2]!
  const set = (base: number[], u: number[], s: number, v: number[] = u, t = 0) => {
    for (let i = 0; i < 3; i++) out[i] = base[i]! + u[i]! * s + v[i]! * t
  }
  const d1 = dot(ab, ap), d2 = dot(ac, ap)
  if (d1 <= 0 && d2 <= 0) return set(a, ab, 0)
  const bp = [p[0]! - b[0]!, p[1]! - b[1]!, p[2]! - b[2]!]
  const d3 = dot(ab, bp), d4 = dot(ac, bp)
  if (d3 >= 0 && d4 <= d3) return set(b, ab, 0)
  const vc = d1 * d4 - d3 * d2
  if (vc <= 0 && d1 >= 0 && d3 <= 0) return set(a, ab, d1 / (d1 - d3))
  const cp = [p[0]! - c[0]!, p[1]! - c[1]!, p[2]! - c[2]!]
  const d5 = dot(ab, cp), d6 = dot(ac, cp)
  if (d6 >= 0 && d5 <= d6) return set(c, ab, 0)
  const vb = d5 * d2 - d1 * d6
  if (vb <= 0 && d2 >= 0 && d6 <= 0) return set(a, ac, d2 / (d2 - d6))
  const va = d3 * d6 - d5 * d4
  if (va <= 0 && d4 - d3 >= 0 && d5 - d6 >= 0) {
    const w = (d4 - d3) / (d4 - d3 + (d5 - d6))
    const bc = [c[0]! - b[0]!, c[1]! - b[1]!, c[2]! - b[2]!]
    return set(b, bc, w)
  }
  const denom = 1 / (va + vb + vc)
  return set(a, ab, vb * denom, ac, vc * denom)
}

/**
 * The point of a triangle list (`positions`, and `indices` or none for a plain list) nearest to `p`,
 * if any is nearer than `max`. Its normal is the triangle's face normal by its winding, which faces
 * out of a closed model.
 */
export function nearestOnTriangles(positions: ArrayLike<number>, indices: ArrayLike<number> | undefined, p: readonly number[], max: number): Nearest | null {
  const count = indices ? indices.length : positions.length / 3
  const a = [0, 0, 0], b = [0, 0, 0], c = [0, 0, 0], q = [0, 0, 0]
  const vertex = (index: number, into: number[]) => {
    into[0] = positions[index * 3]!
    into[1] = positions[index * 3 + 1]!
    into[2] = positions[index * 3 + 2]!
  }
  let best: Nearest | null = null
  let bestTriangle = -1
  let limit = max * max
  for (let t = 0; t + 2 < count; t += 3) {
    vertex(indices ? indices[t]! : t, a)
    vertex(indices ? indices[t + 1]! : t + 1, b)
    vertex(indices ? indices[t + 2]! : t + 2, c)
    closestOnTriangle(p as number[], a, b, c, q)
    const d2 = (q[0]! - p[0]!) ** 2 + (q[1]! - p[1]!) ** 2 + (q[2]! - p[2]!) ** 2
    if (d2 >= limit) continue
    limit = d2
    bestTriangle = t
    best = { point: [q[0]!, q[1]!, q[2]!], normal: [0, 0, 0], distance2: d2 }
  }
  if (!best) return null
  vertex(indices ? indices[bestTriangle]! : bestTriangle, a)
  vertex(indices ? indices[bestTriangle + 1]! : bestTriangle + 1, b)
  vertex(indices ? indices[bestTriangle + 2]! : bestTriangle + 2, c)
  const u = [b[0]! - a[0]!, b[1]! - a[1]!, b[2]! - a[2]!], v = [c[0]! - a[0]!, c[1]! - a[1]!, c[2]! - a[2]!]
  const n = [u[1]! * v[2]! - u[2]! * v[1]!, u[2]! * v[0]! - u[0]! * v[2]!, u[0]! * v[1]! - u[1]! * v[0]!]
  const length = Math.hypot(n[0]!, n[1]!, n[2]!) || 1
  best.normal = [n[0]! / length, n[1]! / length, n[2]! / length]
  return best
}

/** Where on the model under `root` a point lies: the mesh it is on, and the point and normal in that mesh's frame. */
export interface SurfaceHit {
  mesh: Mesh
  point: [number, number, number]
  normal: [number, number, number]
  /** How much the mesh's frame is scaled, so what is placed in it can be sized in metres. */
  scale: number
}

const inverse = mat4.create()

/**
 * The point of the model under `root` nearest to world point `world`, within `max` metres, skipping
 * meshes `skip` says no to (marks already made, say). Instanced and skinned meshes (crews, wheels drawn
 * in batches) are not looked at.
 */
export function nearestSurface(root: Node, world: V, max: number, skip: (mesh: Mesh) => boolean = () => false): SurfaceHit | null {
  root.updateWorldMatrix(true, true)
  let best: SurfaceHit | null = null
  let bestDistance = max
  root.traverse((node) => {
    if (!(node instanceof Mesh) || node instanceof InstancedMesh || node instanceof SkinnedMesh || skip(node)) return
    const { geometry } = node
    if (!geometry.positions.length) return
    mat4.invert(inverse, node.matrixWorld)
    const m = inverse
    const p = [
      m[0]! * world.x + m[4]! * world.y + m[8]! * world.z + m[12]!,
      m[1]! * world.x + m[5]! * world.y + m[9]! * world.z + m[13]!,
      m[2]! * world.x + m[6]! * world.y + m[10]! * world.z + m[14]!,
    ]
    const scale = mat4.maxScale(node.matrixWorld) || 1
    const reach = bestDistance / scale
    const { bounds } = geometry
    if (Math.hypot(p[0]! - bounds.x, p[1]! - bounds.y, p[2]! - bounds.z) - bounds.radius > reach) return
    const hit = nearestOnTriangles(geometry.positions, geometry.indices, p, reach)
    if (!hit) return
    bestDistance = Math.sqrt(hit.distance2) * scale
    best = { mesh: node, point: hit.point, normal: hit.normal, scale }
  })
  return best
}

/**
 * How far along a ray from `o` along `d` (any length) it first meets triangle `a b c`, in units of
 * `d`, either side facing (Möller and Trumbore); null when it misses.
 */
function rayTriangle(o: number[], d: number[], a: number[], b: number[], c: number[]): number | null {
  const e1 = [b[0]! - a[0]!, b[1]! - a[1]!, b[2]! - a[2]!]
  const e2 = [c[0]! - a[0]!, c[1]! - a[1]!, c[2]! - a[2]!]
  const p = [d[1]! * e2[2]! - d[2]! * e2[1]!, d[2]! * e2[0]! - d[0]! * e2[2]!, d[0]! * e2[1]! - d[1]! * e2[0]!]
  const det = e1[0]! * p[0]! + e1[1]! * p[1]! + e1[2]! * p[2]!
  if (Math.abs(det) < 1e-12) return null
  const inv = 1 / det
  const s = [o[0]! - a[0]!, o[1]! - a[1]!, o[2]! - a[2]!]
  const u = (s[0]! * p[0]! + s[1]! * p[1]! + s[2]! * p[2]!) * inv
  if (u < 0 || u > 1) return null
  const q = [s[1]! * e1[2]! - s[2]! * e1[1]!, s[2]! * e1[0]! - s[0]! * e1[2]!, s[0]! * e1[1]! - s[1]! * e1[0]!]
  const v = (d[0]! * q[0]! + d[1]! * q[1]! + d[2]! * q[2]!) * inv
  if (v < 0 || u + v > 1) return null
  const t = (e2[0]! * q[0]! + e2[1]! * q[1]! + e2[2]! * q[2]!) * inv
  return t >= 0 ? t : null
}

/**
 * Where a ray first meets a triangle list (`positions`, and `indices` or none for a plain list), within
 * `max` units of `d`: how far, and the triangle's normal turned to face back along the ray.
 */
export function rayTriangles(positions: ArrayLike<number>, indices: ArrayLike<number> | undefined, o: readonly number[], d: readonly number[], max: number): { t: number; normal: [number, number, number] } | null {
  const count = indices ? indices.length : positions.length / 3
  const a = [0, 0, 0], b = [0, 0, 0], c = [0, 0, 0]
  const vertex = (index: number, into: number[]) => {
    into[0] = positions[index * 3]!
    into[1] = positions[index * 3 + 1]!
    into[2] = positions[index * 3 + 2]!
  }
  let best = max
  let normal: [number, number, number] | null = null
  for (let i = 0; i + 2 < count; i += 3) {
    vertex(indices ? indices[i]! : i, a)
    vertex(indices ? indices[i + 1]! : i + 1, b)
    vertex(indices ? indices[i + 2]! : i + 2, c)
    const t = rayTriangle(o as number[], d as number[], a, b, c)
    if (t === null || t >= best) continue
    best = t
    const u = [b[0]! - a[0]!, b[1]! - a[1]!, b[2]! - a[2]!], v = [c[0]! - a[0]!, c[1]! - a[1]!, c[2]! - a[2]!]
    const n = [u[1]! * v[2]! - u[2]! * v[1]!, u[2]! * v[0]! - u[0]! * v[2]!, u[0]! * v[1]! - u[1]! * v[0]!]
    const facing = n[0]! * d[0]! + n[1]! * d[1]! + n[2]! * d[2]! > 0 ? -1 : 1
    const length = (Math.hypot(n[0]!, n[1]!, n[2]!) || 1) * facing
    normal = [n[0]! / length, n[1]! / length, n[2]! / length]
  }
  return normal ? { t: best, normal } : null
}

/**
 * The first point of the model under `root` that a ray from world `origin` along the unit world `dir`
 * meets within `max` metres, skipping meshes `skip` says no to: what a round going that way struck.
 */
export function raySurface(root: Node, origin: V, dir: V, max: number, skip: (mesh: Mesh) => boolean = () => false): SurfaceHit | null {
  root.updateWorldMatrix(true, true)
  let best: SurfaceHit | null = null
  let bestT = max
  root.traverse((node) => {
    if (!(node instanceof Mesh) || node instanceof InstancedMesh || node instanceof SkinnedMesh || skip(node)) return
    const { geometry } = node
    if (!geometry.positions.length) return
    mat4.invert(inverse, node.matrixWorld)
    const m = inverse
    // The direction goes into the mesh's frame unnormalised, so `t` is world metres in both.
    const o = [m[0]! * origin.x + m[4]! * origin.y + m[8]! * origin.z + m[12]!, m[1]! * origin.x + m[5]! * origin.y + m[9]! * origin.z + m[13]!, m[2]! * origin.x + m[6]! * origin.y + m[10]! * origin.z + m[14]!]
    const d = [m[0]! * dir.x + m[4]! * dir.y + m[8]! * dir.z, m[1]! * dir.x + m[5]! * dir.y + m[9]! * dir.z, m[2]! * dir.x + m[6]! * dir.y + m[10]! * dir.z]
    // Past the geometry's bounding sphere altogether: not worth its triangles.
    const { bounds } = geometry
    const toCentre = [bounds.x - o[0]!, bounds.y - o[1]!, bounds.z - o[2]!]
    const dd = d[0]! ** 2 + d[1]! ** 2 + d[2]! ** 2
    const along = Math.max(0, Math.min(bestT, (toCentre[0]! * d[0]! + toCentre[1]! * d[1]! + toCentre[2]! * d[2]!) / dd))
    const near = [o[0]! + d[0]! * along - bounds.x, o[1]! + d[1]! * along - bounds.y, o[2]! + d[2]! * along - bounds.z]
    if (Math.hypot(near[0]!, near[1]!, near[2]!) > bounds.radius) return
    const hit = rayTriangles(geometry.positions, geometry.indices, o, d, bestT)
    if (!hit) return
    bestT = hit.t
    best = { mesh: node, point: [o[0]! + d[0]! * hit.t, o[1]! + d[1]! * hit.t, o[2]! + d[2]! * hit.t], normal: hit.normal, scale: mat4.maxScale(node.matrixWorld) || 1 }
  })
  return best
}
