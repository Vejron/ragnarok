import { coverSurface, surfaceWetness } from '@/sim/surface'
import type { Point2, TileData } from '@/world'
import type { Pixels } from './terrainmesh'

/*
 * Where a marsh's pools and tussocks lie. The terrain shader grows them from the same field
 * (`engine/src/materials/marsh.wgsl.ts`), so the sedge, the splashes and the mud agree with what is
 * drawn. Nothing here touches the scene, so it runs in the tile worker too.
 */

/** How wet a cover kind is, 0 to 1: 0 for anything but marsh. */
export function marshWetness(kind: number): number {
  return surfaceWetness(coverSurface(kind) ?? -1)
}

function hash(ix: number, iz: number): number {
  let h = Math.imul(ix, 0x8da6b343) ^ Math.imul(iz, 0xd8163841)
  h = Math.imul(h ^ (h >>> 13), 0x5bd1e995)
  h ^= h >>> 15
  return (h >>> 8) / 16777216
}

/** Smooth value noise at world `(x, z)`, 0 to 1, one cell a unit. As `marshValue` in WGSL. */
export function value(x: number, z: number): number {
  const cx = Math.floor(x), cz = Math.floor(z)
  const fx = x - cx, fz = z - cz
  const ux = fx * fx * (3 - 2 * fx), uz = fz * fz * (3 - 2 * fz)
  const a = hash(cx, cz), b = hash(cx + 1, cz), c = hash(cx, cz + 1), d = hash(cx + 1, cz + 1)
  const top = a + (b - a) * ux
  return top + (c + (d - c) * ux - top) * uz
}

/** The marsh's lie at world `(x, z)`, 0 to 1: pools in the lows, tussocks on the highs. As `marshField` in WGSL. */
export function marshField(x: number, z: number): number {
  return 0.65 * value(x * 0.6, z * 0.6) + 0.35 * value(x * 1.8 + 17, z * 1.8 + 31)
}

/** The field under which ground this wet (0 to 1) is open water. As `marshPoolLevel` in WGSL. */
export function marshPoolLevel(wet: number): number {
  return wet * 0.5
}

/** Whether `(x, z)` on ground this wet is in a pool. */
export function inMarshPool(x: number, z: number, wet: number): boolean {
  return wet > 0 && marshField(x, z) < marshPoolLevel(wet)
}

/**
 * How wet each texel of an `n`-texel square map over a tile is, 0 to 255, or null on a tile with no
 * marsh. Covers are painted in their order, as the terrain is, so a later dry cover dries what is
 * under it; then one texel of blur, so the shader's edges fade.
 */
export function wetnessPixels(tile: Pick<TileData, 'cover'>, x0: number, z0: number, size: number, n: number): Uint8Array | null {
  if (!tile.cover.some((cover) => marshWetness(cover.kind) > 0)) return null
  const texel = size / n
  const raw = new Uint8Array(n * n)
  for (const { kind, rings } of tile.cover) {
    const wet = Math.round(marshWetness(kind) * 255)
    scanRings(rings, x0, z0, n, texel, (row, from, to) => raw.fill(wet, row * n + from, row * n + to + 1))
  }
  const out = new Uint8Array(n * n)
  for (let j = 0; j < n; j++) {
    for (let i = 0; i < n; i++) {
      let sum = 0, count = 0
      for (let dj = -1; dj <= 1; dj++) {
        const jj = j + dj
        if (jj < 0 || jj >= n) continue
        for (let di = -1; di <= 1; di++) {
          const ii = i + di
          if (ii < 0 || ii >= n) continue
          sum += raw[jj * n + ii]!
          count++
        }
      }
      out[j * n + i] = Math.round(sum / count)
    }
  }
  return out
}

/** Calls `span(row, from, to)` for each run of texels, centres inside `rings` (even-odd), in a row of an `n`-texel grid. */
function scanRings(rings: readonly Point2[][], x0: number, z0: number, n: number, texel: number, span: (row: number, from: number, to: number) => void) {
  for (let row = 0; row < n; row++) {
    const z = z0 + (row + 0.5) * texel
    const crossings: number[] = []
    for (const ring of rings) {
      for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
        const [ax, az] = ring[i]!
        const [bx, bz] = ring[j]!
        if (az > z !== bz > z) crossings.push(ax + ((z - az) / (bz - az)) * (bx - ax))
      }
    }
    crossings.sort((a, b) => a - b)
    for (let k = 0; k + 1 < crossings.length; k += 2) {
      const from = Math.max(0, Math.ceil((crossings[k]! - x0) / texel - 0.5))
      const to = Math.min(n - 1, Math.floor((crossings[k + 1]! - x0) / texel - 0.5))
      if (from <= to) span(row, from, to)
    }
  }
}

/**
 * A relief map (`reliefPixels`) as a marsh's terrain reads it (`MAT_MARSH`): the normal's x and z in
 * red and green, the wetness in blue, the tone left in alpha. `wet` is `wetnessPixels` at the map's size.
 */
export function packMarsh(relief: Pixels, wet: Uint8Array): Pixels {
  const { data, width, height } = relief
  const out = new Uint8Array(data.length)
  for (let k = 0, p = 0; k < width * height; k++, p += 4) {
    out[p] = data[p]!
    out[p + 1] = data[p + 2]!
    out[p + 2] = wet[k]!
    out[p + 3] = data[p + 3]!
  }
  return { data: out, width, height }
}
