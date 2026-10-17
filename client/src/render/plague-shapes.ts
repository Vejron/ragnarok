import { Color, Geometry } from '@ragnarok/engine'

/**
 * Smooth shapes for the Orange Plague's model. At 40 m he fills the screen, so his surfaces are
 * built finely here instead of from the engine's few-sided primitives: each is a grid of points over
 * two parameters, welded at its seams and poles and shaded with normals averaged over the faces
 * around each point. Some carry vertex colours (linear RGB), painted from where on the surface a
 * point lies: the shirt showing in his jacket, the pale rings round his eyes.
 */

export type V3 = [number, number, number]
/** Paints a point of a surface: `u` around, `v` along, and where it is. Returns an sRGB hex colour. */
export type Paint = (u: number, v: number, at: V3) => number

/**
 * A surface over `u` in 0..1 (`segU` steps) and `v` in 0..1 (`segV` steps), its points from `at`.
 * Faces wind so that `(dP/du) x (dP/dv)` is their front; `flip` turns them the other way.
 */
export function surface(segU: number, segV: number, at: (u: number, v: number) => V3, paint?: Paint, flip = false): Geometry {
  const row = segU + 1
  const positions = new Float32Array(row * (segV + 1) * 3)
  const colors = paint ? new Float32Array(row * (segV + 1) * 3) : undefined
  for (let j = 0; j <= segV; j++) {
    for (let i = 0; i <= segU; i++) {
      const k = j * row + i
      const p = at(i / segU, j / segV)
      positions.set(p, k * 3)
      if (colors && paint) {
        const c = Color.hex(paint(i / segU, j / segV, p))
        colors.set([c.r, c.g, c.b], k * 3)
      }
    }
  }
  const indices: number[] = []
  for (let j = 0; j < segV; j++) {
    for (let i = 0; i < segU; i++) {
      const a = j * row + i
      const b = a + 1
      const c = a + row
      const d = c + 1
      if (flip) indices.push(a, c, b, b, c, d)
      else indices.push(a, b, c, b, d, c)
    }
  }
  return new Geometry({ positions, normals: smoothNormals(positions, indices), colors, indices: new Uint32Array(indices) })
}

/**
 * Normals averaged over the faces round each point, weighted by area, with points in the same place
 * (a seam, a pole) sharing one, so the shading runs smoothly across them.
 */
export function smoothNormals(positions: Float32Array, indices: readonly number[]): Float32Array {
  const count = positions.length / 3
  const sums = new Float32Array(count * 3)
  for (let t = 0; t < indices.length; t += 3) {
    const [a, b, c] = [indices[t]!, indices[t + 1]!, indices[t + 2]!]
    const ax = positions[a * 3]!, ay = positions[a * 3 + 1]!, az = positions[a * 3 + 2]!
    const ux = positions[b * 3]! - ax, uy = positions[b * 3 + 1]! - ay, uz = positions[b * 3 + 2]! - az
    const vx = positions[c * 3]! - ax, vy = positions[c * 3 + 1]! - ay, vz = positions[c * 3 + 2]! - az
    const nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx
    for (const k of [a, b, c]) {
      sums[k * 3] = sums[k * 3]! + nx
      sums[k * 3 + 1] = sums[k * 3 + 1]! + ny
      sums[k * 3 + 2] = sums[k * 3 + 2]! + nz
    }
  }
  // Weld: every point takes the sum over all points in the same place.
  const key = (k: number) => `${Math.round(positions[k * 3]! * 1000)},${Math.round(positions[k * 3 + 1]! * 1000)},${Math.round(positions[k * 3 + 2]! * 1000)}`
  const welded = new Map<string, [number, number, number]>()
  for (let k = 0; k < count; k++) {
    const sum = welded.get(key(k)) ?? [0, 0, 0]
    sum[0] += sums[k * 3]!
    sum[1] += sums[k * 3 + 1]!
    sum[2] += sums[k * 3 + 2]!
    welded.set(key(k), sum)
  }
  const normals = new Float32Array(count * 3)
  for (let k = 0; k < count; k++) {
    const [x, y, z] = welded.get(key(k))!
    const length = Math.hypot(x, y, z) || 1
    normals.set([x / length, y / length, z / length], k * 3)
  }
  return normals
}

/** A ring of a lathe: its radius across (x) and front to back (z), its height, and how far it sits forward. */
export interface Ring {
  y: number
  rx: number
  rz?: number
  /** Shifted towards -Z (his front) by this much. */
  ahead?: number
}

/**
 * A body of revolution, bottom ring first, its cross-sections ellipses. `rings` are joined by
 * Catmull-Rom curves through them, `smoothness` points between each pair, so a few rings make a
 * rounded outline. A ring of zero radius closes an end.
 */
export function lathe(rings: readonly Ring[], segU: number, smoothness = 4, paint?: Paint): Geometry {
  const curve = catmullRom(rings.map((r) => [r.y, r.rx, r.rz ?? r.rx, r.ahead ?? 0]), smoothness)
  return surface(
    segU,
    curve.length - 1,
    (u, v) => {
      const [y, rx, rz, ahead] = curve[Math.round(v * (curve.length - 1))] as [number, number, number, number]
      const angle = u * Math.PI * 2
      return [Math.sin(angle) * rx, y, Math.cos(angle) * rz - ahead]
    },
    paint,
  )
}

/** Points of a Catmull-Rom spline through `points`, `steps` per span, ends included. */
function catmullRom(points: readonly number[][], steps: number): number[][] {
  const out: number[][] = []
  for (let i = 0; i < points.length - 1; i++) {
    const p0 = points[Math.max(0, i - 1)]!, p1 = points[i]!, p2 = points[i + 1]!, p3 = points[Math.min(points.length - 1, i + 2)]!
    for (let s = 0; s < steps; s++) {
      const t = s / steps
      const t2 = t * t, t3 = t2 * t
      out.push(p1.map((_, k) => 0.5 * (2 * p1[k]! + (-p0[k]! + p2[k]!) * t + (2 * p0[k]! - 5 * p1[k]! + 4 * p2[k]! - p3[k]!) * t2 + (-p0[k]! + 3 * p1[k]! - 3 * p2[k]! + p3[k]!) * t3)))
    }
  }
  out.push([...points[points.length - 1]!])
  return out
}

/**
 * An ellipsoid of half-axes `rx, ry, rz` about the origin. `from` and `to` (0 at the top, 1 at the
 * bottom) keep only a band of it; `squash` pulls everything below `-squash` up flat, for a sole.
 */
export function ellipsoid(rx: number, ry: number, rz: number, seg = 32, paint?: Paint, from = 0, to = 1, squash?: number): Geometry {
  return surface(
    seg,
    Math.max(2, Math.round(seg * 0.75 * (to - from))),
    (u, v) => {
      const phi = (from + (to - from) * v) * Math.PI
      const theta = u * Math.PI * 2
      const y = Math.cos(phi) * ry
      return [Math.sin(phi) * Math.sin(theta) * rx, squash === undefined ? y : Math.max(y, -squash), Math.sin(phi) * Math.cos(theta) * rz]
    },
    paint,
    true,
  )
}

/** A limb hanging from the origin down -Y: `length` long, `top` thick at the joint and `bottom` at its end, both ends rounded. */
export function limb(length: number, top: number, bottom: number, segU = 24, paint?: Paint): Geometry {
  const rings: Ring[] = []
  const cap = 5
  for (let i = 0; i <= cap; i++) {
    const a = (i / cap) * (Math.PI / 2)
    rings.push({ y: -length + bottom - Math.cos(a) * bottom, rx: Math.sin(a) * bottom })
  }
  for (let i = cap; i >= 0; i--) {
    const a = (i / cap) * (Math.PI / 2)
    rings.push({ y: Math.cos(a) * top, rx: Math.sin(a) * top })
  }
  // The caps are round already: straight lines between the rings.
  return lathe(rings, segU, 1, paint)
}

/** A torus round the Y axis: `radius` to the middle of the tube, `tube` thick. */
export function torus(radius: number, tube: number, segU = 32, segV = 16): Geometry {
  return surface(segU, segV, (u, v) => {
    const a = u * Math.PI * 2
    const b = v * Math.PI * 2
    const r = radius + Math.cos(b) * tube
    return [Math.sin(a) * r, Math.sin(b) * tube, Math.cos(a) * r]
  })
}

/** A flat outline (x, y, counter-clockwise) given `depth` along Z, centred on Z = 0, with flat faces. */
export function extrude(outline: readonly [number, number][], depth: number): Geometry {
  const positions: number[] = []
  const normals: number[] = []
  const indices: number[] = []
  const z = depth / 2
  for (const [side, nz] of [[z, 1], [-z, -1]] as const) {
    const base = positions.length / 3
    for (const [x, y] of outline) {
      positions.push(x, y, side)
      normals.push(0, 0, nz)
    }
    for (let i = 1; i < outline.length - 1; i++) {
      if (nz > 0) indices.push(base, base + i, base + i + 1)
      else indices.push(base, base + i + 1, base + i)
    }
  }
  for (let i = 0; i < outline.length; i++) {
    const [ax, ay] = outline[i]!
    const [bx, by] = outline[(i + 1) % outline.length]!
    const length = Math.hypot(bx - ax, by - ay) || 1
    const [nx, ny] = [(by - ay) / length, -(bx - ax) / length]
    const base = positions.length / 3
    positions.push(ax, ay, z, bx, by, z, bx, by, -z, ax, ay, -z)
    for (let k = 0; k < 4; k++) normals.push(nx, ny, 0)
    indices.push(base, base + 2, base + 1, base, base + 3, base + 2)
  }
  return new Geometry({ positions: new Float32Array(positions), normals: new Float32Array(normals), indices: new Uint32Array(indices) })
}
