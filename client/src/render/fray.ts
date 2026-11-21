import {
  COVER_MARSH,
  COVER_MARSH_DEEP,
  COVER_MARSH_FAINT,
  COVER_PARK,
  COVER_ROCK,
  COVER_SAND,
  COVER_THICKET_FIGHT,
  COVER_THICKET_SLOW,
  COVER_THICKET_WALK,
  COVER_WOOD,
  type Point2,
  type TileData,
} from '@/world'

/*
 * The frayed edges of natural ground. The importer bends the outlines of woods, heath and rock over
 * metres (`crates/osm-import/src/edges.rs`); here, as a tile's ground is baked, the line where one
 * natural cover meets another is frayed finer: each texel near it may take the cover of a texel up to
 * `FRAY_REACH` away, by a noise of where it is. Ground in use (gardens, fields, paving, golf) never
 * frays, nor spreads over its neighbour. The painted colours, the rock and marsh the shader draws,
 * and where grass grows are all frayed alike (`frayMoves`), each at its own resolution.
 */

/** The noise texels are frayed by: for each octave, its wavelength and how far it moves a texel, metres. */
const FRAY: readonly (readonly [number, number])[] = [
  [4, 0.9],
  [1.5, 0.4],
  [0.5, 0.15],
]
/** The furthest a texel takes its cover from, metres. */
export const FRAY_REACH = FRAY.reduce((sum, [, reach]) => sum + reach, 0)
/** Within this of a tile's edge the fraying fades out, metres: a neighbour's cover is not to hand there. */
const EDGE_FADE = 1.5

/** No cover: the plain grass under every tile, natural ground too. */
const NONE = 255

/** Whether a cover is ground left to itself, whose edges fray. */
export function frays(kind: number): boolean {
  switch (kind) {
    case NONE:
    case COVER_WOOD:
    case COVER_PARK:
    case COVER_SAND:
    case COVER_ROCK:
    case COVER_MARSH:
    case COVER_MARSH_DEEP:
    case COVER_MARSH_FAINT:
    case COVER_THICKET_SLOW:
    case COVER_THICKET_WALK:
    case COVER_THICKET_FIGHT:
      return true
    default:
      return false
  }
}

/** `frays` of every cover kind, looked up in the loops over texels. */
const FRAYS = Uint8Array.from({ length: 256 }, (_, kind) => (frays(kind) ? 1 : 0))

function hash(ix: number, iz: number, seed: number): number {
  let h = Math.imul(ix, 0x8da6b343) ^ Math.imul(iz, 0xd8163841) ^ Math.imul(seed, 0xcb1ab31f)
  h = Math.imul(h ^ (h >>> 13), 0x5bd1e995)
  h ^= h >>> 15
  return (h >>> 8) / 8388608 - 1
}

/** Smooth value noise at `(x, z)`, -1 to 1, one cell a unit, a different field for each `seed`. */
function value(x: number, z: number, seed: number): number {
  const cx = Math.floor(x), cz = Math.floor(z)
  const fx = x - cx, fz = z - cz
  const ux = fx * fx * (3 - 2 * fx), uz = fz * fz * (3 - 2 * fz)
  const a = hash(cx, cz, seed), b = hash(cx + 1, cz, seed), c = hash(cx, cz + 1, seed), d = hash(cx + 1, cz + 1, seed)
  const top = a + (b - a) * ux
  return top + (c + (d - c) * ux - top) * uz
}

/** How far the texel at world `(x, z)` takes its cover from, metres, before the fade at a tile's edge. */
export function frayOffset(x: number, z: number): Point2 {
  let dx = 0, dz = 0
  FRAY.forEach(([wavelength, reach], octave) => {
    dx += reach * value(x / wavelength, z / wavelength, 2 * octave)
    dz += reach * value(x / wavelength, z / wavelength, 2 * octave + 1)
  })
  return [dx, dz]
}

/** Calls `span(row, from, to)` for each run of texels, centres inside `rings` (even-odd), in a row of an `n`-texel grid. */
export function scanRings(rings: readonly Point2[][], x0: number, z0: number, n: number, texel: number, span: (row: number, from: number, to: number) => void) {
  // Where each side crosses the rows whose centres it spans, filed by row: a side is looked at once,
  // not once a row.
  const crossings: (number[] | undefined)[] = new Array<number[] | undefined>(n)
  for (const ring of rings) {
    for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
      const [ax, az] = ring[i]!
      const [bx, bz] = ring[j]!
      if (az === bz) continue
      const first = Math.max(0, Math.floor((Math.min(az, bz) - z0) / texel - 0.5))
      const last = Math.min(n - 1, Math.ceil((Math.max(az, bz) - z0) / texel - 0.5))
      for (let row = first; row <= last; row++) {
        const z = z0 + (row + 0.5) * texel
        if (az > z !== bz > z) (crossings[row] ??= []).push(ax + ((z - az) / (bz - az)) * (bx - ax))
      }
    }
  }
  for (let row = 0; row < n; row++) {
    const found = crossings[row]
    if (!found) continue
    found.sort((a, b) => a - b)
    for (let k = 0; k + 1 < found.length; k += 2) {
      const from = Math.max(0, Math.ceil((found[k]! - x0) / texel - 0.5))
      const to = Math.min(n - 1, Math.floor((found[k + 1]! - x0) / texel - 0.5))
      if (from <= to) span(row, from, to)
    }
  }
}

/** The cover on top in each texel of an `n`-texel square over a tile, in the order they are painted; `NONE` where there is none. */
export function coverIds(tile: Pick<TileData, 'cover'>, x0: number, z0: number, size: number, n: number): Uint8Array {
  const ids = new Uint8Array(n * n).fill(NONE)
  const texel = size / n
  for (const { kind, rings } of tile.cover) scanRings(rings, x0, z0, n, texel, (row, from, to) => ids.fill(kind, row * n + from, row * n + to + 1))
  return ids
}

/**
 * The texels of an `n`-texel square over a tile that take another's cover, as pairs `[to, from]` of
 * texel indices. Empty when no natural cover meets another on the tile.
 */
export function frayMoves(tile: Pick<TileData, 'cover'>, x0: number, z0: number, size: number, n: number): Int32Array {
  if (!tile.cover.some(({ kind }) => frays(kind))) return new Int32Array(0)
  const ids = coverIds(tile, x0, z0, size, n)
  const texel = size / n
  // The texels where one natural cover meets another, then all those within reach of one.
  const reach = Math.ceil(FRAY_REACH / texel)
  const edge = new Uint8Array(n * n)
  let any = false
  for (let row = 0; row < n; row++) {
    for (let column = 0; column < n; column++) {
      const k = row * n + column
      const id = ids[k]!
      if (!FRAYS[id]) continue
      const right = column + 1 < n ? ids[k + 1]! : id
      const down = row + 1 < n ? ids[k + n]! : id
      if ((right !== id && FRAYS[right]) || (down !== id && FRAYS[down])) {
        edge[k] = 1
        any = true
      }
    }
  }
  if (!any) return new Int32Array(0)
  const near = dilate(dilate(edge, n, reach, 1, n), n, reach, n, 1)
  const moves: number[] = []
  for (let row = 0; row < n; row++) {
    const z = z0 + (row + 0.5) * texel
    for (let column = 0; column < n; column++) {
      const k = row * n + column
      if (!near[k] || !FRAYS[ids[k]!]) continue
      const x = x0 + (column + 0.5) * texel
      const fade = smoothstep(0, EDGE_FADE, Math.min(x - x0, z - z0, x0 + size - x, z0 + size - z))
      if (fade <= 0) continue
      const [dx, dz] = frayOffset(x, z)
      const c = Math.min(n - 1, Math.max(0, Math.floor((x + dx * fade - x0) / texel)))
      const r = Math.min(n - 1, Math.max(0, Math.floor((z + dz * fade - z0) / texel)))
      const from = r * n + c
      const source = ids[from]!
      if (source !== ids[k] && FRAYS[source]) moves.push(k, from)
    }
  }
  return Int32Array.from(moves)
}

/** Applies `moves` (`frayMoves`) to `data`, `stride` values a texel, each texel taking its source's values as they were. */
export function applyMoves(data: Uint8Array | Uint8ClampedArray, stride: number, moves: Int32Array) {
  const taken = new Uint8Array((moves.length / 2) * stride)
  for (let m = 0; m < moves.length; m += 2) {
    for (let s = 0; s < stride; s++) taken[(m / 2) * stride + s] = data[moves[m + 1]! * stride + s]!
  }
  for (let m = 0; m < moves.length; m += 2) {
    for (let s = 0; s < stride; s++) data[moves[m]! * stride + s] = taken[(m / 2) * stride + s]!
  }
}

/** `mask` grown by `reach` texels along one axis: `step` apart along it, lines `across` apart. */
function dilate(mask: Uint8Array, n: number, reach: number, step: number, across: number): Uint8Array {
  const out = new Uint8Array(n * n)
  for (let line = 0; line < n; line++) {
    const base = line * across
    // The texels seen in the window round each, counted as it slides.
    let seen = 0
    for (let k = 0; k < Math.min(reach, n); k++) seen += mask[base + k * step]!
    for (let k = 0; k < n; k++) {
      if (k + reach < n) seen += mask[base + (k + reach) * step]!
      if (k - reach - 1 >= 0) seen -= mask[base + (k - reach - 1) * step]!
      if (seen > 0) out[base + k * step] = 1
    }
  }
  return out
}

function smoothstep(from: number, to: number, x: number): number {
  const t = Math.min(1, Math.max(0, (x - from) / (to - from)))
  return t * t * (3 - 2 * t)
}
