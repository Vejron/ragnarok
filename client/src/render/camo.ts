import { Geometry, Texture, type PixelData } from '@ragnarok/engine'

/** Lattice cells across the tile for the patches' coarsest octave, along and up: wider than tall, so they run in bands. */
const CELLS_U = 3
const CELLS_V = 5
/** How much of the tile each sprayed colour covers, the first under the second. */
const COVERAGE = [0.34, 0.24] as const
/** How soft a patch's edge is: the share of the tile over which it fades in, as from a spray gun. */
const FEATHER = 0.09

/** An integer hash of a lattice point, 0..1. */
function hash(x: number, y: number, seed: number) {
  let h = Math.imul(x, 374761393) ^ Math.imul(y, 668265263) ^ Math.imul(seed, 2147483647)
  h = Math.imul(h ^ (h >>> 13), 1274126177)
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296
}

/** Value noise on a lattice of `pu` × `pv` cells that wraps, at (u, v) in cells. */
function noise(u: number, v: number, pu: number, pv: number, seed: number) {
  const x0 = Math.floor(u), y0 = Math.floor(v)
  const fx = u - x0, fy = v - y0
  const sx = fx * fx * (3 - 2 * fx), sy = fy * fy * (3 - 2 * fy)
  const xa = ((x0 % pu) + pu) % pu, xb = (xa + 1) % pu, ya = ((y0 % pv) + pv) % pv, yb = (ya + 1) % pv
  const top = hash(xa, ya, seed) + (hash(xb, ya, seed) - hash(xa, ya, seed)) * sx
  const bottom = hash(xa, yb, seed) + (hash(xb, yb, seed) - hash(xa, yb, seed)) * sx
  return top + (bottom - top) * sy
}

/** Three octaves of wrapping noise at (x, y) in tiles (0..1 is one tile): broad bands with wavy edges. */
function fbm(x: number, y: number, seed: number) {
  let sum = 0, amp = 1, total = 0
  for (let octave = 0, scale = 1; octave < 3; octave++, scale *= 2, amp *= 0.4) {
    sum += noise(x * CELLS_U * scale, y * CELLS_V * scale, CELLS_U * scale, CELLS_V * scale, seed + octave * 101) * amp
    total += amp
  }
  return sum / total
}

const smoothstep = (a: number, b: number, x: number) => {
  const t = Math.min(1, Math.max(0, (x - a) / (b - a)))
  return t * t * (3 - 2 * t)
}

/**
 * A tiling WW2 camouflage, `size` pixels square: `scheme[0]` as the base coat, with each later colour
 * sprayed over it in wavy patches of warped noise whose edges fade, as from a spray gun.
 */
export function camoPixels(scheme: readonly number[], seed = 1, size = 256): PixelData {
  const rgb = new Float32Array(size * size * 3)
  const unpack = (c: number) => [(c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff] as const
  const [r0, g0, b0] = unpack(scheme[0]!)
  for (let i = 0; i < size * size; i++) rgb.set([r0, g0, b0], i * 3)
  const field = new Float32Array(size * size)
  scheme.slice(1).forEach((color, index) => {
    const s = seed * 7919 + index * 31
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const u = x / size, v = y / size
        // A warp of the domain by more noise bends the bands into waves and fingers.
        const wu = u + (fbm(u, v, s + 1000) - 0.5) * 0.25
        const wv = v + (fbm(u, v, s + 2000) - 0.5) * 0.15
        field[y * size + x] = fbm(wu, wv, s)
      }
    }
    // The edge fades between two quantiles, so its softness is a share of the tile whatever the noise's range.
    const sorted = Float32Array.from(field).sort()
    const at = (q: number) => sorted[Math.min(sorted.length - 1, Math.max(0, Math.floor(sorted.length * q)))]!
    const cover = COVERAGE[index] ?? COVERAGE[COVERAGE.length - 1]!
    const lo = at(1 - cover - FEATHER / 2), hi = at(1 - cover + FEATHER / 2)
    const [r, g, b] = unpack(color)
    for (let i = 0; i < field.length; i++) {
      const t = smoothstep(lo, hi, field[i]!)
      rgb[i * 3] = rgb[i * 3]! + (r - rgb[i * 3]!) * t
      rgb[i * 3 + 1] = rgb[i * 3 + 1]! + (g - rgb[i * 3 + 1]!) * t
      rgb[i * 3 + 2] = rgb[i * 3 + 2]! + (b - rgb[i * 3 + 2]!) * t
    }
  })
  const data = new Uint8Array(size * size * 4)
  for (let i = 0; i < size * size; i++) {
    data[i * 4] = Math.round(rgb[i * 3]!)
    data[i * 4 + 1] = Math.round(rgb[i * 3 + 1]!)
    data[i * 4 + 2] = Math.round(rgb[i * 3 + 2]!)
    data[i * 4 + 3] = 255
  }
  return { data, width: size, height: size }
}

const textures = new Map<string, Texture>()

/** The camo texture of `scheme` (base coat first), made on first use and shared by every tank painted so. */
export function camoTexture(scheme: readonly number[]): Texture {
  const key = scheme.join()
  let texture = textures.get(key)
  if (!texture) {
    texture = new Texture(camoPixels(scheme, textures.size + 1), { wrap: 'repeat', anisotropy: 4 })
    textures.set(key, texture)
  }
  return texture
}

/**
 * The same triangles with uvs projected from the box round them: each triangle takes the plane its
 * face most nearly faces and its uv is the other two coordinates, `metresPerTile` to a tile. Every
 * corner of a triangle shares one projection, so none smears across two.
 */
export function boxUvs(geometry: Geometry, metresPerTile = 5): Geometry {
  const flat = geometry.toNonIndexed()
  const p = flat.positions
  const uvs = new Float32Array((p.length / 3) * 2)
  for (let t = 0; t < p.length; t += 9) {
    const ax = p[t + 3]! - p[t]!, ay = p[t + 4]! - p[t + 1]!, az = p[t + 5]! - p[t + 2]!
    const bx = p[t + 6]! - p[t]!, by = p[t + 7]! - p[t + 1]!, bz = p[t + 8]! - p[t + 2]!
    const nx = Math.abs(ay * bz - az * by), ny = Math.abs(az * bx - ax * bz), nz = Math.abs(ax * by - ay * bx)
    // The coordinates the face spreads over: drop the axis it faces along.
    const [a, b] = nx >= ny && nx >= nz ? [2, 1] : ny >= nz ? [0, 2] : [0, 1]
    for (let corner = 0; corner < 3; corner++) {
      const v = t + corner * 3
      const uv = (t / 9) * 6 + corner * 2
      uvs[uv] = p[v + a]! / metresPerTile
      uvs[uv + 1] = p[v + b]! / metresPerTile
    }
  }
  flat.uvs = uvs
  if (!flat.normals) flat.computeNormals()
  return flat
}
