import type { TileGround } from '@/sim/terrain'
import {
  CROP_NONE,
  COVER_ASPHALT,
  COVER_FARM,
  COVER_GARDEN,
  COVER_INDUSTRIAL,
  COVER_MARSH,
  COVER_MARSH_DEEP,
  COVER_MARSH_FAINT,
  COVER_GOLF_BUNKER,
  COVER_GOLF_FAIRWAY,
  COVER_GOLF_GREEN,
  COVER_GOLF_TEE,
  COVER_PARK,
  COVER_ROCK,
  COVER_SAND,
  COVER_THICKET_FIGHT,
  COVER_THICKET_SLOW,
  COVER_THICKET_WALK,
  COVER_URBAN,
  COVER_WATER,
  COVER_WOOD,
  DECK_ASPHALT,
  DETAIL_SLOPE_SCALE,
  type TileData,
} from '@/world'
import { FLOOR_HEATH, FLOOR_LEAF, FLOOR_MOSS, FOREST_CELL, STAND_FLOORS, WOOD_STRIDE, forestOf, standOf } from './forest'
import { applyMoves, frayMoves } from './fray'
import { winter } from './season'
import { CAVITY_RADIUS, heightSampler, ringMean, toneOf, type HeightSource } from './shade'
import type { Sphere } from '@ragnarok/engine'

/*
 * A tile's terrain as plain arrays, as part of preparing a tile (`tileprep.ts`): in the tile worker
 * (`tile.worker.ts`) or, without one, on the main thread. Nothing here touches the engine or the DOM, so it runs in a worker.
 */

/**
 * The colours of the ground, sRGB: what the ground is under the blades, seen from standing height, and
 * not the colour a map gives it. Open ground nothing covers is meadow. The painted layers
 * (`COVER_LAYERS`) vary each about these.
 */
export const GRASS_COLOR = 0x799f55

export const COVER_COLORS: Record<number, number> = {
  [COVER_WATER]: 0x5cc0f0,
  // A forest's floor: needles, moss and bilberry, in the shade.
  [COVER_WOOD]: 0x56733c,
  [COVER_PARK]: 0x85a95c,
  [COVER_FARM]: 0xc9c98a,
  [COVER_SAND]: 0xd8cfae,
  // Paving round shops and schools.
  [COVER_URBAN]: 0xb9b5ab,
  // A car park: the roads' own asphalt, so its aisles do not stand out from it.
  [COVER_ASPHALT]: DECK_ASPHALT,
  // A garden's lawn: mown, a little yellower than a meadow.
  [COVER_GARDEN]: 0x80a856,
  // Gravel and packed earth.
  [COVER_INDUSTRIAL]: 0xa39d92,
  [COVER_THICKET_SLOW]: 0x4f7040,
  [COVER_THICKET_WALK]: 0x466639,
  [COVER_THICKET_FIGHT]: 0x3b5a32,
  [COVER_MARSH]: 0x6f8f6a,
  [COVER_MARSH_DEEP]: 0x5f7f5e,
  [COVER_MARSH_FAINT]: 0x83a36c,
  // Grey granite and gneiss, the sea's and the ice's smooth slabs.
  [COVER_ROCK]: 0xb6b5b2,
  // Mown short, so a shade richer than the park round them, the green the richest.
  [COVER_GOLF_FAIRWAY]: 0x86d063,
  [COVER_GOLF_TEE]: 0x7ccb5c,
  [COVER_GOLF_GREEN]: 0x62c457,
  [COVER_GOLF_BUNKER]: 0xf2e2b2,
}

/** The gravel round a house in a garden (`APRON_WIDTH`), sRGB. */
export const GRAVEL_COLOR = 0xaaa192

/** A garden's flower bed (`gardenplan.ts`): dug soil, and its edge. */
const BED_COLOR = 0x4d3b2b
const BED_EDGE = 0x3a2c20

/** How far out from a house's walls its gravel reaches in a garden, metres. */
export const APRON_WIDTH = 2

/**
 * One layer of the variation painted over a cover's flat colour: a noise that repeats every `period`
 * metres, with features `period / cells` metres across. Without a `tint` it lightens and darkens the
 * ground by up to `strength` (0 to 1); with one it lays patches of that colour (sRGB) over `share`
 * of the ground, `strength` opaque.
 */
interface CoverLayer {
  period: number
  cells: number
  strength: number
  tint?: number
  share?: number
}

/** Meadow, and the open ground nothing covers: drifts of richer and drier grass. */
const MEADOW_LAYERS: readonly CoverLayer[] = [
  { period: 256, cells: 12, strength: 0.3 },
  { period: 64, cells: 20, strength: 0.16 },
  { period: 128, cells: 14, strength: 0.3, tint: 0x9aa85a, share: 0.3 },
]

/** What is painted over each cover's colour, in order. A cover not here is left flat. */
const COVER_LAYERS: Record<number, readonly CoverLayer[]> = {
  [COVER_PARK]: MEADOW_LAYERS,
  // Moss in its greens, and the brown of needles and bare earth under the densest trees.
  [COVER_WOOD]: [
    { period: 128, cells: 18, strength: 0.3 },
    { period: 96, cells: 22, strength: 0.4, tint: 0x6b6a3e, share: 0.3 },
    { period: 64, cells: 26, strength: 0.35, tint: 0x5f8a3c, share: 0.25 },
  ],
  [COVER_THICKET_SLOW]: [{ period: 128, cells: 18, strength: 0.28 }],
  [COVER_THICKET_WALK]: [{ period: 128, cells: 18, strength: 0.28 }],
  [COVER_THICKET_FIGHT]: [{ period: 128, cells: 18, strength: 0.28 }],
  // A lawn: mown even, with dry and worn patches.
  [COVER_GARDEN]: [
    { period: 128, cells: 14, strength: 0.14 },
    { period: 64, cells: 22, strength: 0.3, tint: 0xa9b060, share: 0.22 },
  ],
  // Slabs a few metres across in paler and darker stone, pale grey-green and dark lichen on them, and
  // now and then the pink of a granite.
  [COVER_ROCK]: [
    { period: 96, cells: 28, strength: 0.16 },
    { period: 128, cells: 12, strength: 0.28, tint: 0xbfa9a0, share: 0.2 },
    { period: 64, cells: 36, strength: 0.5, tint: 0xcbcfc2, share: 0.2 },
    { period: 64, cells: 44, strength: 0.45, tint: 0x77777a, share: 0.12 },
  ],
  [COVER_SAND]: [{ period: 128, cells: 16, strength: 0.14 }],
  [COVER_FARM]: [{ period: 256, cells: 10, strength: 0.14 }],
  [COVER_URBAN]: [{ period: 64, cells: 24, strength: 0.1 }],
  [COVER_ASPHALT]: [{ period: 64, cells: 24, strength: 0.06 }],
  [COVER_INDUSTRIAL]: [
    { period: 64, cells: 24, strength: 0.18 },
    { period: 128, cells: 14, strength: 0.3, tint: 0x8a8272, share: 0.25 },
  ],
  [COVER_MARSH]: [{ period: 128, cells: 20, strength: 0.2 }],
  [COVER_MARSH_DEEP]: [{ period: 128, cells: 20, strength: 0.2 }],
  [COVER_MARSH_FAINT]: [{ period: 128, cells: 20, strength: 0.2 }],
}

/**
 * A wood's floor under each stand (`forest.ts`), sRGB, and what is painted over it. A wood with too
 * few trees near to say (`FLOOR_MIXED`) keeps `COVER_WOOD`'s.
 */
export const FLOOR_COLORS: Record<number, number> = {
  // Pine heath: dry and open, the olive of lingonberry and heather.
  [FLOOR_HEATH]: 0x6d7648,
  // Under spruce: moss, deep green in the shade.
  [FLOOR_MOSS]: 0x4a6c35,
  // Under birch: grass and herbs, the freshest green in the wood.
  [FLOOR_LEAF]: 0x6b9549,
}

const FLOOR_LAYERS: Record<number, readonly CoverLayer[]> = {
  // Cushions of pale reindeer lichen a metre or two across, mats of dark lingonberry, and needles.
  [FLOOR_HEATH]: [
    { period: 128, cells: 18, strength: 0.28 },
    { period: 64, cells: 14, strength: 0.4, tint: 0x4f6a3a, share: 0.3 },
    { period: 64, cells: 22, strength: 0.4, tint: 0x8a6c44, share: 0.2 },
    { period: 64, cells: 40, strength: 0.6, tint: 0xb4bca2, share: 0.2 },
  ],
  // Feather moss in brighter drifts, damp hollows gone dark, and bare needles in the deepest shade.
  [FLOOR_MOSS]: [
    { period: 128, cells: 18, strength: 0.3 },
    { period: 128, cells: 14, strength: 0.5, tint: 0x34492a, share: 0.22 },
    { period: 64, cells: 30, strength: 0.45, tint: 0x6f9a3e, share: 0.28 },
    { period: 64, cells: 26, strength: 0.4, tint: 0x5c5034, share: 0.14 },
  ],
  // Lusher and drier grass, and last year's leaves.
  [FLOOR_LEAF]: [
    { period: 128, cells: 16, strength: 0.3 },
    { period: 64, cells: 20, strength: 0.4, tint: 0x58a03e, share: 0.25 },
    { period: 64, cells: 28, strength: 0.4, tint: 0x97824a, share: 0.2 },
  ],
}

/**
 * What a tree drops round its foot, by its stand (the order of `STAND_FLOORS`): its colour (sRGB),
 * how opaque at the trunk, and how far out it lies as a share of the crown's radius. A spruce's
 * needles lie thick in its own deep shade, a pine's thin and rust-brown, a birch's leaves pale.
 */
const LITTER: readonly { color: number; strength: number; reach: number }[] = [
  { color: 0x8c6c44, strength: 0.45, reach: 0.75 },
  { color: 0x6e5236, strength: 0.7, reach: 1.05 },
  { color: 0x94864f, strength: 0.3, reach: 0.85 },
]
/** No tree's litter lies nearer in than this, metres: a young tree drops some too. */
const LITTER_MIN = 0.9
/** Texels across a litter's picture, which is drawn at each tree's size. */
const LITTER_PIXELS = 64

/** A canvas `pixels` square with its context, or null where there is none to make (tests). */
function scratchCanvas(pixels: number): Painter | null {
  const canvas = typeof OffscreenCanvas !== 'undefined' ? new OffscreenCanvas(pixels, pixels) : typeof document !== 'undefined' ? Object.assign(document.createElement('canvas'), { width: pixels, height: pixels }) : null
  return (canvas?.getContext('2d') as Painter | null | undefined) ?? null
}

/** Each stand's litter as a soft disc, made once. */
let litters: (CanvasImageSource | null)[] | undefined

function litterDiscs(): (CanvasImageSource | null)[] {
  litters ??= LITTER.map(({ color }) => {
    const ctx = scratchCanvas(LITTER_PIXELS)
    if (!ctx) return null
    const half = LITTER_PIXELS / 2
    const rgb = `${color >> 16}, ${(color >> 8) & 255}, ${color & 255}`
    const fade = ctx.createRadialGradient(half, half, 0, half, half, half)
    fade.addColorStop(0, `rgba(${rgb}, 1)`)
    fade.addColorStop(0.45, `rgba(${rgb}, 0.8)`)
    fade.addColorStop(1, `rgba(${rgb}, 0)`)
    ctx.fillStyle = fade
    ctx.fillRect(0, 0, LITTER_PIXELS, LITTER_PIXELS)
    return ctx.canvas
  })
  return litters
}

/**
 * The floors of a tile's stands, to lay over its woods: each stand's ground (`FLOOR_COLORS` and its
 * layers) as opaque as the stand's share of the trees there, fading between the cells of the forest's
 * grid, and over it each tree's litter. Clear where no stand is. Null where there is no canvas.
 */
function paintFloors(woods: Float32Array, x0: number, z0: number, size: number, pixels: number): CanvasImageSource | null {
  const forest = forestOf(woods, x0, z0, size)
  const out = scratchCanvas(pixels), stand = scratchCanvas(pixels), mask = scratchCanvas(forest.n)
  if (!out || !stand || !mask) return null
  const scale = pixels / size
  const { n, shares } = forest
  const whole = (ctx: Painter) => () => {
    ctx.beginPath()
    ctx.rect(0, 0, pixels, pixels)
  }
  STAND_FLOORS.forEach((floor, k) => {
    const image = mask.createImageData(n, n)
    let any = false
    for (let i = 0; i < n * n; i++) {
      const share = Math.round(shares[i * 3 + k]! * 255)
      any ||= share > 0
      image.data.set([255, 255, 255, share], i * 4)
    }
    if (!any) return
    mask.putImageData(image, 0, 0)
    stand.globalCompositeOperation = 'source-over'
    stand.fillStyle = css(FLOOR_COLORS[floor]!)
    stand.fillRect(0, 0, pixels, pixels)
    paintLayers(stand, FLOOR_LAYERS[floor]!, 64 + floor, x0, z0, scale, whole(stand))
    // Kept only where the stand is: a grid cell's share, smoothed out to its neighbours'.
    stand.globalCompositeOperation = 'destination-in'
    stand.imageSmoothingEnabled = true
    stand.drawImage(mask.canvas, 0, 0, n, n, 0, 0, n * FOREST_CELL * scale, n * FOREST_CELL * scale)
    out.drawImage(stand.canvas, 0, 0)
  })
  const discs = litterDiscs()
  for (let i = 0; i < woods.length; i += WOOD_STRIDE) {
    const k = standOf(woods[i + 3]!)
    const disc = k === undefined ? null : discs[k]
    if (k === undefined || !disc) continue
    const radius = Math.max(LITTER_MIN, woods[i + 2]! * LITTER[k]!.reach) * scale
    out.globalAlpha = LITTER[k]!.strength
    out.drawImage(disc, (woods[i]! - x0) * scale - radius, (woods[i + 1]! - z0) * scale - radius, radius * 2, radius * 2)
  }
  out.globalAlpha = 1
  return out.canvas
}

/** Texels along a layer's noise tile. */
const LAYER_PIXELS = 256

/** A hash of a lattice point to [0, 1), the same in every worker so tiles painted apart agree. */
function layerHash(i: number, j: number, salt: number): number {
  let h = (Math.imul(i, 0x9e3779b1) ^ Math.imul(j, 0x85ebca77) ^ Math.imul(salt, 0xc2b2ae3d)) >>> 0
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d)
  h = Math.imul(h ^ (h >>> 12), 0x297a2d39)
  return ((h ^ (h >>> 15)) >>> 8) / 16777216
}

/**
 * A noise, 0 to 1, at each texel of a square tile that repeats at its edges: `cells` lattice cells
 * across, and a second octave of twice as many at half the weight.
 */
export function layerNoise(cells: number, salt: number, pixels = LAYER_PIXELS): Float32Array {
  const out = new Float32Array(pixels * pixels)
  const octave = (n: number, x: number, z: number, seed: number) => {
    const i = Math.floor(x), j = Math.floor(z)
    const u = x - i, v = z - j
    const su = u * u * (3 - 2 * u), sv = v * v * (3 - 2 * v)
    const at = (di: number, dj: number) => layerHash((i + di) % n, (j + dj) % n, seed)
    const top = at(0, 0) + (at(1, 0) - at(0, 0)) * su
    return top + (at(0, 1) + (at(1, 1) - at(0, 1)) * su - top) * sv
  }
  for (let row = 0; row < pixels; row++) {
    for (let column = 0; column < pixels; column++) {
      const x = (column / pixels) * cells, z = (row / pixels) * cells
      out[row * pixels + column] = (octave(cells, x, z, salt) * 2 + octave(cells * 2, x * 2, z * 2, salt + 101)) / 3
    }
  }
  return out
}

type Painter = CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D

/** Each layer's noise tile as a canvas, made once and kept: they are the same for every tile. */
const layerTiles = new Map<CoverLayer, CanvasImageSource | null>()

/**
 * A layer's tile to fill with: grey about the middle for a layer that lightens and darkens (laid with
 * `overlay`, which leaves the ground as it is under mid-grey), or its tint, opaque over its share of
 * the ground and clear over the rest. Null where there is no canvas to make it on.
 */
function layerTile(layer: CoverLayer, salt: number): CanvasImageSource | null {
  const made = layerTiles.get(layer)
  if (made !== undefined) return made
  const canvas = typeof OffscreenCanvas !== 'undefined' ? new OffscreenCanvas(LAYER_PIXELS, LAYER_PIXELS) : typeof document !== 'undefined' ? Object.assign(document.createElement('canvas'), { width: LAYER_PIXELS, height: LAYER_PIXELS }) : null
  const ctx = canvas?.getContext('2d') as Painter | null | undefined
  if (!canvas || !ctx) {
    layerTiles.set(layer, null)
    return null
  }
  const noise = layerNoise(layer.cells, salt)
  const image = ctx.createImageData(LAYER_PIXELS, LAYER_PIXELS)
  // The noise of two octaves keeps to the middle: this much of it is under each level, near enough.
  const level = 0.5 + (0.5 - (layer.share ?? 0)) * 0.5
  for (let k = 0; k < noise.length; k++) {
    const value = noise[k]!
    if (layer.tint === undefined) {
      const grey = Math.round(Math.min(1, Math.max(0, 0.5 + (value - 0.5) * 2.2)) * 255)
      image.data.set([grey, grey, grey, 255], k * 4)
    } else {
      const cover = Math.min(1, Math.max(0, (value - level) / 0.08))
      image.data.set([layer.tint >> 16, (layer.tint >> 8) & 255, layer.tint & 255, Math.round(cover * 255)], k * 4)
    }
  }
  ctx.putImageData(image, 0, 0)
  layerTiles.set(layer, canvas)
  return canvas
}

/**
 * Paints a cover's layers over what the current path of `ctx` holds (`outline` lays it), each
 * anchored to the world, so a patch carries on across the edge of a tile into the next one's painting.
 */
function paintLayers(ctx: Painter, layers: readonly CoverLayer[], kind: number, x0: number, z0: number, scale: number, outline: () => void) {
  layers.forEach((layer, index) => {
    const tile = layerTile(layer, kind * 16 + index)
    const pattern = tile && ctx.createPattern(tile, 'repeat')
    if (!pattern) return
    const zoom = (layer.period * scale) / LAYER_PIXELS
    const wrap = (at: number) => -(((at % layer.period) + layer.period) % layer.period) * scale
    pattern.setTransform({ a: zoom, b: 0, c: 0, d: zoom, e: wrap(x0), f: wrap(z0) })
    ctx.save()
    ctx.globalCompositeOperation = layer.tint === undefined ? 'overlay' : 'source-over'
    ctx.globalAlpha = layer.strength
    ctx.fillStyle = pattern
    outline()
    ctx.fill('evenodd')
    ctx.restore()
  })
}

/**
 * The ground under standing crops (`TileData.crops`): straw, a shade under the ears' own colour, so
 * that what shows between the stalks from above is more field and not pale earth, and the lid that
 * stands for the field far off (`croplid.ts`) meets it without a line.
 */
export const CROP_GROUND_COLOR = 0xe2c86c

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
  /** Its vertices in the renderer's layout and its bounds, once the tile worker has packed it (`pack`). */
  packed?: ArrayBuffer
  bounds?: Sphere
}

/** RGBA8 pixels, rows top to bottom (the engine's `PixelData`). */
export interface Pixels {
  data: Uint8Array
  width: number
  height: number
}

/**
 * Whether the terrain splits the cell at `column`, `row` of its sample grid along the diagonal from
 * its first corner to its last (`(column, row)` to `(column + 1, row + 1)`) rather than across, where
 * it is meshed in full: RTIN's smallest triangles alternate, so the diagonals meet in the middle of
 * every two cells by two. Anything drawn on the ground's cells that must meet the terrain along the
 * same lines (the water's shoreline) splits them so too.
 */
export function splitsAlongMainDiagonal(column: number, row: number): boolean {
  return ((column + row) & 1) === 0
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
      const tone = toneOf(normal, h, meanAt(i, j))
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

/** What of a tile its cover is painted from: the cover, the crops on it, the houses a garden has gravel round, the trees of its woods and its flower beds. */
export type CoverSource = Pick<TileData, 'cover'> & Partial<Pick<TileData, 'crops'>> & {
  buildings?: readonly { footprint: readonly (readonly [number, number])[]; holes?: readonly (readonly (readonly [number, number])[])[] }[]
  /** The tile's trees (`woodsOf`), for the floors of its woods. */
  woods?: Float32Array
  /** Its gardens' flower beds (`GardenPlan.beds`), dug soil. */
  beds?: Float32Array
}

/**
 * Paints a tile's ground cover, seen from above, into `ctx`, `pixels` square. Canvas `(0, 0)` is
 * the tile's `(x0, z0)` corner, which is UV `(0, 0)` with `flipY` off.
 */
export function paintCover(
  ctx: Painter,
  tile: CoverSource,
  x0: number,
  z0: number,
  size: number,
  pixels: number,
) {
  const scale = pixels / size
  const at = ([x, z]: readonly [number, number]) => [(x - x0) * scale, (z - z0) * scale] as const

  ctx.fillStyle = css(GRASS_COLOR)
  ctx.fillRect(0, 0, pixels, pixels)
  paintLayers(ctx, MEADOW_LAYERS, -1, x0, z0, scale, () => {
    ctx.beginPath()
    ctx.rect(0, 0, pixels, pixels)
  })

  // Where crops stand, and a cell further every way: a field's ground is straw to the edge it was
  // drawn with, not to the steps its cells make, so the farmland is painted again in straw inside this.
  const crops = winter ? undefined : tile.crops
  let sown: Path2D | null = null
  if (crops && typeof Path2D !== 'undefined') {
    sown = new Path2D()
    const n = Math.round(Math.sqrt(crops.length))
    const cell = pixels / n
    for (let row = 0; row < n; row++) {
      for (let column = 0; column < n; column++) {
        if (crops[row * n + column] === CROP_NONE) continue
        const from = column
        while (column + 1 < n && crops[row * n + column + 1] !== CROP_NONE) column++
        sown.rect((from - 1) * cell, (row - 1) * cell, (column + 3 - from) * cell, 3 * cell)
      }
    }
  }

  // The floors of the woods' stands, painted once and laid into each wood.
  const floors = tile.woods && tile.woods.length > 0 && tile.cover.some(({ kind }) => kind === COVER_WOOD) ? paintFloors(tile.woods, x0, z0, size, pixels) : null

  for (const { kind, rings } of tile.cover) {
    const outline = () => {
      ctx.beginPath()
      for (const ring of rings) {
        ring.forEach((p, i) => (i === 0 ? ctx.moveTo(...at(p)) : ctx.lineTo(...at(p))))
        ctx.closePath()
      }
    }
    ctx.fillStyle = css(COVER_COLORS[kind] ?? GRASS_COLOR)
    outline()
    ctx.fill('evenodd')
    const layers = COVER_LAYERS[kind]
    if (layers) paintLayers(ctx, layers, kind, x0, z0, scale, outline)
    if (kind === COVER_WOOD && floors) {
      ctx.save()
      outline()
      ctx.clip('evenodd')
      ctx.drawImage(floors, 0, 0)
      ctx.restore()
    }
    if (kind === COVER_GARDEN && tile.buildings?.length) {
      // Gravel round each house, as far as the garden goes: a line along its walls, half of it under the house.
      ctx.save()
      outline()
      ctx.clip('evenodd')
      ctx.strokeStyle = css(GRAVEL_COLOR)
      ctx.lineWidth = APRON_WIDTH * 2 * scale
      ctx.lineJoin = 'round'
      ctx.beginPath()
      for (const { footprint, holes } of tile.buildings) {
        for (const ring of [footprint, ...(holes ?? [])]) {
          ring.forEach((p, i) => (i === 0 ? ctx.moveTo(...at(p)) : ctx.lineTo(...at(p))))
          ctx.closePath()
        }
      }
      ctx.stroke()
      ctx.restore()
    }
    if (kind !== COVER_FARM || !sown) continue
    // Straw under the crops (not in winter: the stubble stands in snow). The covers painted after
    // this one, the woods and water over the field, still go over it.
    ctx.save()
    ctx.clip(sown)
    ctx.fillStyle = css(CROP_GROUND_COLOR)
    outline()
    ctx.fill('evenodd')
    ctx.restore()
  }

  // The line where natural ground meets natural ground is frayed (`fray.ts`): each texel near it may
  // take the colour of one a little way off.
  const moves = frayMoves(tile, x0, z0, size, pixels)
  if (moves.length > 0 && 'getImageData' in ctx) {
    const image = ctx.getImageData(0, 0, pixels, pixels)
    applyMoves(image.data, 4, moves)
    ctx.putImageData(image, 0, 0)
  }

  // Flower beds, dug dark soil edged a shade darker, over whatever the garden was. Not under a winter's snow.
  const beds = winter ? undefined : tile.beds
  if (beds && beds.length > 0) {
    ctx.fillStyle = css(BED_COLOR)
    ctx.strokeStyle = css(BED_EDGE)
    ctx.lineWidth = Math.max(1, 0.08 * scale)
    for (let i = 0; i + 8 <= beds.length; i += 8) {
      ctx.beginPath()
      for (let k = 0; k < 4; k++) (k === 0 ? ctx.moveTo : ctx.lineTo).call(ctx, ...at([beds[i + k * 2]!, beds[i + k * 2 + 1]!]))
      ctx.closePath()
      ctx.fill()
      ctx.stroke()
    }
  }
}
