import type { TileGround } from '@/sim/terrain'
import {
  COVER_FARM,
  COVER_INDUSTRIAL,
  COVER_PARK,
  COVER_SAND,
  COVER_URBAN,
  COVER_WATER,
  COVER_WOOD,
  DETAIL_SLOPE_SCALE,
  type TileData,
} from '@/world'
import { CAVITY_RADIUS, heightSampler, ringMean, toneOf, type HeightSource } from './shade'

/*
 * A tile's terrain as plain arrays: what both the terrain worker (`terrain.worker.ts`) and the main
 * thread's fallback build. Nothing here touches the engine or the DOM, so it runs in a worker.
 */

export const GRASS_COLOR = 0x8fd16a

export const COVER_COLORS: Record<number, number> = {
  [COVER_WATER]: 0x5cc0f0,
  [COVER_WOOD]: 0x5fae5a,
  [COVER_PARK]: 0xa8e07a,
  [COVER_FARM]: 0xe3e79a,
  [COVER_SAND]: 0xf3e3a6,
  [COVER_URBAN]: 0xd9d3c7,
  [COVER_INDUSTRIAL]: 0xc6c6d0,
}

/** Texels per metre of terrain. */
const TEXELS_PER_METRE = 4

/**
 * The vertical error the drawn terrain may have against the heights, metres, by level of detail
 * (0 nearest). Physics keeps every sample; the mesh drops those it can do without (`terrainBuffers`).
 */
export const TERRAIN_LOD_ERRORS = [0.04, 0.15, 0.5] as const

/** Distances from the viewer to a tile's nearest edge past which it drops to the next level, metres. */
export const TERRAIN_LOD_RANGES = [150, 320] as const
/** A tile changes level only once this far past the range, so one at the border does not flicker between two. */
const LOD_HYSTERESIS = 20

/** The level of detail (into `TERRAIN_LOD_ERRORS`) for a tile `distance` metres away, at `current` now. */
export function terrainLevel(distance: number, current?: number): number {
  let level = 0
  while (level < TERRAIN_LOD_RANGES.length && distance > TERRAIN_LOD_RANGES[level]!) level++
  if (current === undefined || level === current) return level
  const range = TERRAIN_LOD_RANGES[Math.min(level, current)]!
  return Math.abs(distance - range) > LOD_HYSTERESIS ? level : current
}

/**
 * Depth of the skirt hung under every tile edge, metres. Neighbours simplified to different levels
 * can part along their seam by up to both their errors; the skirt fills that gap.
 */
const SKIRT_DEPTH = 2 * TERRAIN_LOD_ERRORS[TERRAIN_LOD_ERRORS.length - 1]!

export interface TerrainBuffers {
  positions: Float32Array
  normals: Float32Array
  uvs: Float32Array
  indices: Uint32Array
}

/** RGBA8 pixels, rows top to bottom (the engine's `PixelData`). */
export interface Pixels {
  data: Uint8Array
  width: number
  height: number
}

/**
 * One tile's terrain as indexed triangles, in world coordinates, with UVs running 0..1 over the tile
 * for its cover texture and relief map (`reliefPixels`), which carry its colour and its shading.
 *
 * The grid is simplified (RTIN, as Mapbox's Martini): split into right triangles, each halved only
 * where some sample under it is more than `maxError` off its plane, so flat streets and lawns take a
 * few big triangles and banks and ditches keep theirs. RTIN wants `2^k + 1` samples a side; any
 * other grid (250 m tiles at 1 m are 251) is carried on past its far edges to the next such size
 * (`terrainErrors`) and the triangles are cut back to the tile. Each edge hangs a skirt, since a
 * neighbour simplified otherwise does not meet it exactly. Pass the tile's `errors` to mesh it again
 * at another level without working them out anew.
 */
export function terrainBuffers(ground: TileGround, maxError = 0, errors = terrainErrors(ground)): TerrainBuffers {
  const { side: p, heights, errors: error } = errors
  const max = p - 1
  // The tile's far edge, in samples: the grid past it is only there to make RTIN's size.
  const edge = ground.side - 1
  // Each vertex's grid position (column, row; fractional where a triangle was cut) and height.
  const xs: number[] = []
  const ys: number[] = []
  const hs: number[] = []
  // The vertex each sample became, plus one; 0 for samples not used.
  const slot = new Uint32Array(p * p)
  const sample = (x: number, y: number) => {
    const at = y * p + x
    if (slot[at] === 0) {
      slot[at] = xs.push(x)
      ys.push(y)
      hs.push(heights[at]!)
    }
    return slot[at]! - 1
  }
  // Vertices made where triangles cross the far edges, by place.
  const cuts = new Map<number, number>()
  const coord = (v: number, axis: number) => (axis === 0 ? xs[v]! : ys[v]!)
  /** Where the side `i`–`j` of a triangle crosses the far edge across `axis`: the same vertex from either triangle on that side. */
  const crossing = (i: number, j: number, axis: number) => {
    if (coord(i, axis) === edge) return i
    if (coord(j, axis) === edge) return j
    const [a, b] = i < j ? [i, j] : [j, i]
    const t = (edge - coord(a, axis)) / (coord(b, axis) - coord(a, axis))
    const x = axis === 0 ? edge : xs[a]! + t * (xs[b]! - xs[a]!)
    const y = axis === 1 ? edge : ys[a]! + t * (ys[b]! - ys[a]!)
    const key = Math.round(x * 4096) * 4194304 + Math.round(y * 4096)
    let v = cuts.get(key)
    if (v === undefined) {
      v = xs.push(x) - 1
      ys.push(y)
      hs.push(hs[a]! + t * (hs[b]! - hs[a]!))
      cuts.set(key, v)
    }
    return v
  }
  /** The part of convex polygon `polygon` (vertices, counter-clockwise from above) on the tile's side of the far edge across `axis`. */
  const clip = (polygon: number[], axis: number) => {
    const out: number[] = []
    const add = (v: number) => {
      if (out[out.length - 1] !== v && out[0] !== v) out.push(v)
    }
    for (let k = 0; k < polygon.length; k++) {
      const [current, next] = [polygon[k]!, polygon[(k + 1) % polygon.length]!]
      const inside = coord(current, axis) <= edge
      if (inside) add(current)
      if (inside !== coord(next, axis) <= edge) add(crossing(current, next, axis))
    }
    return out
  }

  const triangles: number[] = []
  const visit = (ax: number, ay: number, bx: number, by: number, cx: number, cy: number) => {
    // Wholly past the far edge: nothing of it is drawn.
    if (Math.min(ax, bx, cx) >= edge && edge < max) return
    if (Math.min(ay, by, cy) >= edge && edge < max) return
    const mx = (ax + bx) >> 1
    const my = (ay + by) >> 1
    if (Math.abs(ax - cx) + Math.abs(ay - cy) > 1 && error[my * p + mx]! > maxError) {
      visit(cx, cy, ax, ay, mx, my)
      visit(bx, by, cx, cy, mx, my)
    } else if (Math.max(ax, bx, cx) <= edge && Math.max(ay, by, cy) <= edge) {
      // Counter-clockwise seen from above (+Y), so the faces point up.
      triangles.push(sample(ax, ay), sample(bx, by), sample(cx, cy))
    } else {
      const polygon = clip(clip([sample(ax, ay), sample(bx, by), sample(cx, cy)], 0), 1)
      for (let k = 1; k + 1 < polygon.length; k++) triangles.push(polygon[0]!, polygon[k]!, polygon[k + 1]!)
    }
  }
  visit(0, 0, max, max, max, 0)
  visit(max, max, 0, 0, 0, max)

  // Only the vertices the triangles use: a triangle cut back can leave corners past the edge.
  const used = new Int32Array(xs.length).fill(-1)
  const order: number[] = []
  for (let i = 0; i < triangles.length; i++) {
    const v = triangles[i]!
    if (used[v] === -1) used[v] = order.push(v) - 1
    triangles[i] = used[v]!
  }
  // Skirts: each edge walked with the tile on its left, so the wall hung under it faces out.
  const hung: number[] = []
  const surface = order.length
  const skirt = (on: (v: number) => boolean, along: (v: number) => number) => {
    const line: number[] = []
    for (let v = 0; v < surface; v++) if (on(order[v]!)) line.push(v)
    line.sort((a, b) => along(order[a]!) - along(order[b]!))
    const below = line.map((v) => order.push(order[v]!) - 1)
    for (let i = 0; i + 1 < line.length; i++) triangles.push(line[i]!, line[i + 1]!, below[i + 1]!, line[i]!, below[i + 1]!, below[i]!)
    hung.push(...below)
  }
  skirt((v) => ys[v] === 0, (v) => xs[v]!) // z0, facing -z
  skirt((v) => xs[v] === edge, (v) => ys[v]!) // x1, facing +x
  skirt((v) => ys[v] === edge, (v) => -xs[v]!) // z1, facing +z
  skirt((v) => xs[v] === 0, (v) => -ys[v]!) // x0, facing -x

  const { x0, z0, cell } = ground
  const count = order.length
  const positions = new Float32Array(count * 3)
  const normals = new Float32Array(count * 3)
  const uvs = new Float32Array(count * 2)
  const h = (column: number, row: number) => heights[Math.min(Math.max(row, 0), edge) * p + Math.min(Math.max(column, 0), edge)]!
  for (let i = 0; i < count; i++) {
    const v = order[i]!
    const [x, y] = [xs[v]!, ys[v]!]
    positions[i * 3] = x0 + x * cell
    positions[i * 3 + 1] = hs[v]!
    positions[i * 3 + 2] = z0 + y * cell
    // The relief map shades the surface; these are its rough shape, for anything that reads it.
    const [column, row] = [Math.round(x), Math.round(y)]
    const dx = (h(column + 1, row) - h(column - 1, row)) / (2 * cell)
    const dz = (h(column, row + 1) - h(column, row - 1)) / (2 * cell)
    const inv = 1 / Math.hypot(dx, 1, dz)
    normals[i * 3] = -dx * inv
    normals[i * 3 + 1] = inv
    normals[i * 3 + 2] = -dz * inv
    uvs[i * 2] = x / edge
    uvs[i * 2 + 1] = y / edge
  }
  for (const vertex of hung) positions[vertex * 3 + 1]! -= SKIRT_DEPTH
  return { positions, normals, uvs, indices: Uint32Array.from(triangles) }
}

/** The corners `(ax, ay, bx, by)` of every triangle of the RTIN hierarchy, by grid side. */
const hierarchies = new Map<number, Uint16Array>()

/**
 * Triangle `i` of the hierarchy has its right angle at `c` and its hypotenuse `a`–`b`; triangles 0
 * and 1 are the tile's two halves and the children of `i` are `2i + 2` and `2i + 3`, down to (not
 * including) the smallest triangles. Coordinates are (column, row) in the grid.
 */
function hierarchy(side: number): Uint16Array {
  let coords = hierarchies.get(side)
  if (coords) return coords
  const size = side - 1
  const count = size * size * 2 - 2
  coords = new Uint16Array(count * 4)
  for (let i = 0; i < count; i++) {
    let id = i + 2
    let [ax, ay, bx, by, cx, cy] = [0, 0, 0, 0, 0, 0]
    if (id & 1) bx = by = cx = size
    else ax = ay = cy = size
    while ((id >>= 1) > 1) {
      const mx = (ax + bx) >> 1
      const my = (ay + by) >> 1
      if (id & 1) [bx, by, ax, ay] = [ax, ay, cx, cy]
      else [ax, ay, bx, by] = [bx, by, cx, cy]
      ;[cx, cy] = [mx, my]
    }
    coords[i * 4] = ax
    coords[i * 4 + 1] = ay
    coords[i * 4 + 2] = bx
    coords[i * 4 + 3] = by
  }
  hierarchies.set(side, coords)
  return coords
}

/** A tile's grid carried on to `2^k + 1` samples a side, with how far its surface strays at each split. */
export interface TerrainErrors {
  side: number
  heights: Float32Array
  /**
   * How far the surface strays, metres, if the triangle split at each sample is left whole: the most
   * any sample under it sits off its plane, or under its children, so a big triangle is kept only
   * when every sample under it is within the error, and a triangle is split whenever its neighbour
   * across the hypotenuse (which splits at the same sample) is.
   */
  errors: Float32Array
}

/**
 * `ground`'s grid carried on past its far edges to the next `2^k + 1` samples a side, at the slope
 * it had there (so a triangle across the edge bends as little as it can), and its errors.
 */
export function terrainErrors(ground: TileGround): TerrainErrors {
  const { side: p, heights } = padded(ground)
  const coords = hierarchy(p)
  const count = coords.length / 4
  // The hierarchy stops above the smallest triangles (legs one sample long), which are never split:
  // the last `(p - 1)²` triangles in it have only those as children.
  const parents = count - (p - 1) * (p - 1)
  const errors = new Float32Array(p * p)
  for (let i = count - 1; i >= 0; i--) {
    const k = i * 4
    const ax = coords[k]!
    const ay = coords[k + 1]!
    const bx = coords[k + 2]!
    const by = coords[k + 3]!
    const mx = (ax + bx) >> 1
    const my = (ay + by) >> 1
    const cx = mx + my - ay
    const cy = my + ax - mx
    const ha = heights[ay * p + ax]!
    const hb = heights[by * p + bx]!
    const hc = heights[cy * p + cx]!
    // Barycentric weights of b and c at (x, y), over twice the area.
    const det = (bx - ax) * (cy - ay) - (cx - ax) * (by - ay)
    let error = errors[my * p + mx]!
    for (let y = Math.min(ay, by, cy); y <= Math.max(ay, by, cy); y++) {
      for (let x = Math.min(ax, bx, cx); x <= Math.max(ax, bx, cx); x++) {
        const wb = ((x - ax) * (cy - ay) - (cx - ax) * (y - ay)) / det
        const wc = ((bx - ax) * (y - ay) - (x - ax) * (by - ay)) / det
        if (wb < 0 || wc < 0 || wb + wc > 1) continue
        const off = Math.abs(ha + wb * (hb - ha) + wc * (hc - ha) - heights[y * p + x]!)
        if (off > error) error = off
      }
    }
    if (i < parents) {
      error = Math.max(error, errors[((ay + cy) >> 1) * p + ((ax + cx) >> 1)]!, errors[((by + cy) >> 1) * p + ((bx + cx) >> 1)]!)
    }
    errors[my * p + mx] = error
  }
  return { side: p, heights, errors }
}

function padded({ side, heights }: TileGround): { side: number; heights: Float32Array } {
  let size = 2
  while (size < side - 1) size *= 2
  const p = size + 1
  if (p === side) return { side, heights }
  const last = side - 1
  const at = (column: number, row: number) => heights[row * side + column]!
  const out = new Float32Array(p * p)
  for (let row = 0; row < p; row++) {
    for (let column = 0; column < p; column++) {
      const [c, r] = [Math.min(column, last), Math.min(row, last)]
      let height = at(c, r)
      if (column > last && last > 0) height += (column - last) * (at(last, r) - at(last - 1, r))
      if (row > last && last > 0) height += (row - last) * (at(c, last) - at(c, last - 1))
      out[row * p + column] = height
    }
  }
  return { side: p, heights: out }
}

/** Spacing of the ring means the relief map's tone is interpolated from, metres: the ring is smooth. */
const RING_STEP = 3

/**
 * The tile's shading: an object-space normal map with the baked tone (`toneAt`) in alpha, at half
 * scale (the material doubles it: `normalMapTone`). Texel `k` of a row covers `[k, k + 1]` of the
 * `n` texels over the tile, which UV `0..1` spans with `flipY` off; object space is world space, as
 * the mesh is built in world coordinates. There is a texel a cell, or one a slope sample where the
 * tile has a finer `detail` slope map. The normals need only the tile's own heights, but the tone
 * looks `CAVITY_RADIUS` past the edge, into `around` (the neighbours): when a neighbour arrives,
 * pass the tile's map as `previous` and only the texels that near the edge are worked out again.
 */
export function reliefPixels(ground: TileGround, around?: HeightSource, detail?: Int8Array, previous?: Pixels): Pixels {
  const { x0, z0, cell, side, heights } = ground
  const size = cell * (side - 1)
  const fine = detail && detail.length > 0 ? Math.round(Math.sqrt(detail.length / 2)) : 0
  const slopes = fine > side - 1 && fine * fine * 2 === detail!.length ? detail! : null
  const n = slopes ? fine : side - 1
  const texel = size / n
  const height = heightSampler(ground, around)
  const reuse = previous !== undefined && previous.width === n
  const data = reuse ? previous.data.slice() : new Uint8Array(n * n * 4)
  // Texels this near the edge have a tone that sees past it: every texel, for a new map.
  const band = reuse ? Math.ceil(CAVITY_RADIUS / texel) + 1 : n
  const near = (i: number, j: number) => i < band || i >= n - band || j < band || j >= n - band

  // The ring means (`ringMean`) on a coarser lattice of texels, every `step`th and the last.
  const step = Math.max(1, Math.round(RING_STEP / texel))
  const lattice = Math.ceil((n - 1) / step) + 1
  const at = (c: number) => Math.min(c * step, n - 1)
  const means = new Float32Array(lattice * lattice)
  for (let cj = 0; cj < lattice; cj++) {
    for (let ci = 0; ci < lattice; ci++) {
      // Only where a texel worked out below reads it: one between this and the next lattice point.
      const [i, j] = [at(ci), at(cj)]
      if (!near(Math.max(at(ci - 1), 0), Math.max(at(cj - 1), 0)) && !near(at(ci + 1), at(cj + 1))) continue
      means[cj * lattice + ci] = ringMean(height, x0 + (i + 0.5) * texel, z0 + (j + 0.5) * texel)
    }
  }
  const meanAt = (i: number, j: number) => {
    const ci = Math.min(Math.floor(i / step), lattice - 2)
    const cj = Math.min(Math.floor(j / step), lattice - 2)
    if (lattice < 2) return means[0]!
    const u = (i - at(ci)) / (at(ci + 1) - at(ci))
    const v = (j - at(cj)) / (at(cj + 1) - at(cj))
    const top = means[cj * lattice + ci]! * (1 - u) + means[cj * lattice + ci + 1]! * u
    const bottom = means[(cj + 1) * lattice + ci]! * (1 - u) + means[(cj + 1) * lattice + ci + 1]! * u
    return top * (1 - v) + bottom * v
  }

  const normal: [number, number, number] = [0, 1, 0]
  for (let j = 0; j < n; j++) {
    const nearRow = j < band || j >= n - band
    for (let i = 0; i < n; i++) {
      if (!nearRow && i >= band && i < n - band) {
        i = n - band - 1
        continue
      }
      const x = x0 + (i + 0.5) * texel
      const z = z0 + (j + 0.5) * texel
      let h: number
      let sx: number
      let sz: number
      if (slopes) {
        const k = j * n + i
        sx = slopes[k * 2]! / DETAIL_SLOPE_SCALE
        sz = slopes[k * 2 + 1]! / DETAIL_SLOPE_SCALE
        h = height(x, z)
      } else {
        // A texel a cell: its height, and its slope, from the cell's four corners.
        const h00 = heights[j * side + i]!
        const h10 = heights[j * side + i + 1]!
        const h01 = heights[(j + 1) * side + i]!
        const h11 = heights[(j + 1) * side + i + 1]!
        h = (h00 + h10 + h01 + h11) / 4
        sx = (h10 - h00 + h11 - h01) / (2 * cell)
        sz = (h01 - h00 + h11 - h10) / (2 * cell)
      }
      const inv = 1 / Math.hypot(sx, 1, sz)
      normal[0] = -sx * inv
      normal[1] = inv
      normal[2] = -sz * inv
      const tone = toneOf(normal, h, meanAt(i, j), x, z)
      const p = (j * n + i) * 4
      data[p] = Math.round(normal[0] * 127.5 + 127.5)
      data[p + 1] = Math.round(normal[1] * 127.5 + 127.5)
      data[p + 2] = Math.round(normal[2] * 127.5 + 127.5)
      data[p + 3] = Math.round(Math.min(tone / 2, 1) * 255)
    }
  }
  return { data, width: n, height: n }
}

/** Side of the square cover texture for a tile `size` metres across, pixels. */
export function coverPixels(size: number): number {
  return Math.min(1024, Math.max(128, 2 ** Math.round(Math.log2(size * TEXELS_PER_METRE))))
}

const css = (color: number) => `#${color.toString(16).padStart(6, '0')}`

/**
 * Paints a tile's ground cover, seen from above, into `ctx`, `pixels` square. Canvas `(0, 0)` is
 * the tile's `(x0, z0)` corner, which is UV `(0, 0)` with `flipY` off.
 */
export function paintCover(
  ctx: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D,
  tile: Pick<TileData, 'cover'>,
  x0: number,
  z0: number,
  size: number,
  pixels: number,
) {
  const scale = pixels / size
  const at = ([x, z]: [number, number]) => [(x - x0) * scale, (z - z0) * scale] as const

  ctx.fillStyle = css(GRASS_COLOR)
  ctx.fillRect(0, 0, pixels, pixels)

  for (const { kind, rings } of tile.cover) {
    ctx.fillStyle = css(COVER_COLORS[kind] ?? GRASS_COLOR)
    ctx.beginPath()
    for (const ring of rings) {
      ring.forEach((p, i) => (i === 0 ? ctx.moveTo(...at(p)) : ctx.lineTo(...at(p))))
      ctx.closePath()
    }
    ctx.fill('evenodd')
  }
}
