import { Color, Geometry, Group, Mesh, ShaderMaterial, VERTEX_STRIDE } from '@ragnarok/engine'
import { COVER_GARDEN, COVER_PARK, COVER_WOOD, CROP_NONE, STONES_DENSITY, type TileData, type TileRef } from '@/world'
import { CROP_HEIGHT } from '@/sim/consts.generated'
import { WIND_HEADING } from '@/sim/wind'
import { SURFACE_CELL } from '@/sim/surface'
import type { TileGround } from '@/sim/terrain'
import { COVER_COLORS, GRASS_COLOR } from './cover'
import { GROUND_LIGHT_WGSL } from './groundlight'
import { isRock } from './rocks'
import { normalAt } from './shade'
import { FLOOR_HEATH, FLOOR_LEAF, FLOOR_MIXED, FLOOR_MOSS } from './forest'
import { GARDEN_SHADE, SHADE_FLOORS, grassMask, type GrassMask } from './grassmask'
import { inMarshPool, marshField, marshPoolLevel } from './marshnoise'

export { grassMask, type GrassMask }

/**
 * Grass: short blades near the camera that lean with the wind, part around whoever walks or drives
 * through them, and are flattened by a blast's shock and spring back. It is all for show: nothing
 * on the server knows it is there, and it hides nothing.
 *
 * Crops are grown the same way (`WHEAT`), by a second `Grass` over the fields of `TileData.crops`.
 * Those the server does know: wheat stands `CROP_HEIGHT` high and hides whoever is low in it, from
 * the eye here and from the bots there (CROPS.md). What tells the two apart is their `Growth`.
 *
 * The blades are built on the CPU in square chunks around the camera, one merged mesh a chunk, and
 * all the movement is in the vertex shader, from a handful of uniforms: the wind, the nearest few
 * movers and the latest few blasts.
 *
 * Nothing pops in or out. Every blade has a rank, and the field thins with distance by the shader
 * shrinking the blades whose rank is past what that distance keeps, while the ones left grow wider.
 * Chunks only hold the blades their distance can show (a power-of-two share of the full count) and
 * are rebuilt with more before the camera gets close enough to miss them.
 */

/** Side of a chunk of blades, metres. */
export const GRASS_CHUNK = 16
/** Most movers (players, vehicles, rotor wash) that bend the grass at once. */
const BENDERS = 8
/** Most blasts whose shock is still crossing the grass. */
const BLASTS = 4
/** Seconds a blast moves the grass for. */
const BLAST_LIFE = 3
/**
 * About how long a frame spends building chunks, ms, at least one a frame: so walking into a meadow, or
 * driving through miles of it, does not hitch. A chunk takes a fraction of a millisecond.
 */
const BUILD_MS = 1.5
/** The camera moves this far, metres, before the chunks round it are looked over again. */
const RESCAN_DISTANCE = 0.5
/** Metres ahead of need a chunk is given more blades, so they are there before they show. */
const LOOKAHEAD = 6

/** How much grass there is: the full field, or a thinner and nearer one for phones and tablets. */
export interface GrassDensity {
  /** Blades a square metre inside a patch, within `full` of the camera. */
  perMetre: number
  /** Metres from the camera within which every blade shows; past it they thin as the square of the distance. */
  full: number
  /** Metres from the camera where the last blades have shrunk away. */
  far: number
}

export const GRASS_FULL: GrassDensity = { perMetre: 12, full: 20, far: 110 }
export const GRASS_LITE: GrassDensity = { perMetre: 6, full: 13, far: 65 }
/**
 * A field of wheat, the same at every quality: it hides people, so a thinner field on a lower setting
 * would be a way to see through it. Past `far` the field is the lid of `croplid.ts`.
 */
export const WHEAT_DENSITY: GrassDensity = { perMetre: 8, full: 25, far: 140 }
/** A winter field's stubble: sparse, and only underfoot. */
export const STUBBLE_DENSITY: GrassDensity = { perMetre: 5, full: 12, far: 45 }

/** The share of the blades that shows at `distance` metres. */
export function keptAt(density: GrassDensity, distance: number) {
  return Math.min(1, (density.full / Math.max(distance, 1e-3)) ** 2)
}

/** The power-of-two share of the blades a chunk holds when the nearest it is to the camera is `distance`. */
export function tierFor(density: GrassDensity, distance: number) {
  const kept = keptAt(density, Math.max(0, distance))
  return 2 ** Math.max(-6, Math.ceil(Math.log2(kept)))
}

/**
 * Grass colour per cell of a mask: 0 for no grass, else an index into `PALETTE`. The last three are a
 * marsh's sedge (`MARSH_SHADES`), damp to bog, yellowing and browning the wetter it is, after them a
 * garden's lawn (`GARDEN_SHADE`), and last what grows under pine, spruce and birch (`FLOOR_SHADES`): a
 * heath's pale hair-grass, the few dark blades the moss lets through, and a birch wood's fresh green.
 */
const PALETTE = [0, GRASS_COLOR, COVER_COLORS[COVER_PARK]!, COVER_COLORS[COVER_WOOD]!, 0x9fae62, 0x9a9f55, 0x8e8a4c, COVER_COLORS[COVER_GARDEN]!, 0x8f9c5c, 0x5c8540, 0x70a548].map((hex) => Color.hex(hex))
/**
 * How much of a meadow's grass grows on each forest floor, as a share of the blades: thin on a
 * heath, next to none in the moss under spruce, thick under birch. The shrubs and ferns of
 * `forestfloor.ts` stand where the grass does not.
 */
const FLOOR_GRASS: Record<number, number> = { [FLOOR_MIXED]: 0.5, [FLOOR_HEATH]: 0.3, [FLOOR_MOSS]: 0.12, [FLOOR_LEAF]: 1 }
/** A mown lawn's blades against a meadow's. */
const LAWN_HEIGHT = 0.3
/** How wet the ground of each `PALETTE` entry is: as `surfaceWetness` has the marsh's grades. */
const PALETTE_WET = [0, 0, 0, 0, 0.35, 0.65, 1]
/** Cottongrass's seed heads. */
const COTTON = Color.hex(0xf4f1e6)


/** A small, fast hash to [0, 1): the same chunk always grows the same blades. */
export function random(seed: number) {
  let s = seed >>> 0
  return () => {
    s = (s + 0x6d2b79f5) >>> 0
    let t = s
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

export interface Blade {
  x: number
  z: number
  /** Ground height at the root. */
  y: number
  height: number
  /** Facing of the blade's flat side, radians. */
  turn: number
  /** 0..1: where in the order of thinning it goes; the blades below `keptAt` show. */
  rank: number
  shade: number
  /** Brightness, around 1. */
  tone: number
  /** The ground's normal at the root, so the blade is lit as the slope it grows on. */
  normal: readonly [number, number, number]
  /** A head at the top, in the growth's `head` colour: cottongrass's white tuft, an ear of wheat. */
  head?: boolean
}

/** What a draw of a blade grows into: its height, metres, whether it has a head and whether two shorter blades stand about it. */
interface Planted {
  height: number
  head: boolean
  tuft: boolean
}

/**
 * What a `Grass` grows: a meadow's grass, a field's wheat or its stubble. Everything else (the chunks,
 * the thinning with distance, the wind, the bending) is the same for all of them.
 */
export interface Growth {
  /** The name of its node in the scene, which the GPU bench turns off by. */
  name: string
  /** A blade's colour for each value of the mask; entry 0 is never read. */
  palette: readonly Color[]
  /** The colour of a blade's head. */
  head: Color
  /** Whether a head takes the blade's own brightness, or is the plain colour. */
  headToned: boolean
  /** A blade's width at its root, metres. */
  width: number
  /** Chunks holding at most this share of the blades are far enough off to draw each as one triangle. */
  simpleShare: number
  /** How far up a blade each level under its tip is, and how much of the width is left there: full, and far-off chunks' one triangle. */
  levels: readonly (readonly [number, number])[]
  simpleLevels: readonly (readonly [number, number])[]
  /** The same for a blade with a head. */
  headLevels: readonly (readonly [number, number])[]
  simpleHeadLevels: readonly (readonly [number, number])[]
  /** Where it grows on a tile. */
  mask(tile: TileData, x0: number, z0: number, size: number, ground: TileGround | null): GrassMask
  /** The blade at `(x, z)` on ground of this `shade`, or null for none. `height` (0.3 to 0.7, mostly low), `tone` (0.85 to 1.15) and `chance` (0 to 1) are the blade's own draws. */
  plant(x: number, z: number, shade: number, height: number, tone: number, chance: number): Planted | null
}

/**
 * A marsh's sedge at `(x, z)` on ground this `wet`, or null for open water: tussocks thick and tall on
 * the highs of the marsh's field, thinning to the pools' edges, with reeds round the bog's pools and the
 * odd cottongrass. `chance`, `height` and `pick` are the blade's own draws, 0 to 1.
 */
function sedge(x: number, z: number, wet: number, chance: number, height: number, pick: number): Planted | null {
  const field = marshField(x, z)
  const level = marshPoolLevel(wet)
  if (field < level + 0.02) return null
  const t = Math.min(1, (field - level) / 0.25)
  const clump = t * t * (3 - 2 * t)
  if (chance > 0.15 + 0.85 * clump) return null
  // Reeds stand in the shallows at a bog pool's edge.
  if (wet >= 1 && field < level + 0.09 && pick < 0.3) return { height: 1.3 + ((pick * 13) % 1) * 0.7, head: false, tuft: false }
  if (wet > 0.5 && pick > 0.95 && clump > 0.3) return { height: 0.35 + height * 0.3, head: true, tuft: false }
  const grown = height * 1.3 * (0.7 + 0.45 * clump)
  // Sedge grows in tufts, but for the reeds.
  return { height: grown, head: false, tuft: grown < 1.2 }
}

/** Smooth noise in 0..1 on a lattice of unit cells. */
export function valueNoise(x: number, z: number) {
  const ix = Math.floor(x), iz = Math.floor(z)
  const fx = x - ix, fz = z - iz
  const corner = (i: number, j: number) => {
    let h = Math.imul(i, 0x27d4eb2d) ^ Math.imul(j, 0x165667b1)
    h = Math.imul(h ^ (h >>> 15), 0x85ebca77)
    return ((h ^ (h >>> 13)) >>> 0) / 4294967296
  }
  const sx = fx * fx * (3 - 2 * fx), sz = fz * fz * (3 - 2 * fz)
  const top = corner(ix, iz) + (corner(ix + 1, iz) - corner(ix, iz)) * sx
  const bottom = corner(ix, iz + 1) + (corner(ix + 1, iz + 1) - corner(ix, iz + 1)) * sx
  return top + (bottom - top) * sz
}

/**
 * How thick the grass grows at `(x, z)`, 0..1: meadows of twenty-odd metres and clumps of a few inside
 * them, with bare ground (and the odd tuft) between.
 */
export function patchAt(x: number, z: number) {
  const n = valueNoise(x / 19, z / 19) * 0.7 + valueNoise(x / 5.5 + 17.3, z / 5.5 - 4.1) * 0.3
  const t = Math.min(1, Math.max(0, (n - 0.4) / 0.22))
  return t * t * (3 - 2 * t)
}

/** A meadow: grass in patches, and a marsh's sedge. */
export const MEADOW: Growth = {
  name: 'grass',
  palette: PALETTE,
  head: COTTON,
  headToned: false,
  width: 0.07,
  simpleShare: 1 / 8,
  levels: [[0, 1], [0.55, 0.7]],
  simpleLevels: [[0, 1]],
  // Cottongrass: a thin stalk under a white head, widest just below its top.
  headLevels: [[0, 0.4], [0.82, 1.7]],
  simpleHeadLevels: [[0, 0.5]],
  mask: (tile, x0, z0, size, ground) => grassMask(tile, x0, z0, size, ground),
  plant(x, z, shade, height, tone, chance) {
    const wet = PALETTE_WET[shade] ?? 0
    if (wet > 0) return sedge(x, z, wet, chance, height, (tone - 0.85) / 0.3)
    // A lawn is mown: short, and even all over.
    if (shade === GARDEN_SHADE) return { height: height * LAWN_HEIGHT, head: false, tuft: false }
    const patch = patchAt(x, z)
    const floor = SHADE_FLOORS[shade]
    // A few stray tufts grow between the patches; fewer blades of either on a forest's floor.
    if (chance > (patch * 0.96 + 0.04) * (floor === undefined ? 1 : FLOOR_GRASS[floor]!)) return null
    // Taller in the thick of a patch, short at its edge.
    return { height: height * (0.55 + 0.6 * patch), head: false, tuft: false }
  },
}

/**
 * Where a tile's crops stand (`TileData.crops`), on the surface grid a `Grass` reads: 1 where there
 * are any. A tile without crops has a mask of no cells.
 */
export function cropMask(crops: Uint8Array | undefined, x0: number, z0: number, size: number): GrassMask {
  if (!crops) return { x0, z0, side: 0, cells: new Uint8Array() }
  const n = Math.round(Math.sqrt(crops.length))
  const side = Math.max(1, Math.ceil(size / SURFACE_CELL))
  const cells = new Uint8Array(side * side)
  const under = (index: number) => Math.min(n - 1, Math.floor((((index + 0.5) * SURFACE_CELL) / size) * n))
  for (let row = 0; row < side; row++) {
    for (let column = 0; column < side; column++) cells[row * side + column] = crops[under(row) * n + under(column)] === CROP_NONE ? 0 : 1
  }
  return { x0, z0, side, cells }
}

/**
 * A field of wheat: every stalk about `CROP_HEIGHT`, a broad leaf to near its top and an ear over it,
 * even across the field with no bare patches, so that a line of sight through a few metres of it is shut.
 */
export const WHEAT: Growth = {
  name: 'crops',
  palette: [0, 0xd2bf64].map((hex) => Color.hex(hex)),
  head: Color.hex(0xf6da7a),
  headToned: true,
  width: 0.14,
  // As far off as grass, some 75 m: a stalk's one triangle is narrower at the top and darker than its
  // leaf and ear, so nearer than that a chunk changing from the one to the other shows as a patch
  // of thicker crop popping in. It costs 5 MB of vertices in the middle of a field.
  simpleShare: 1 / 8,
  levels: [[0, 0.75], [0.72, 1]],
  simpleLevels: [[0, 0.9]],
  headLevels: [[0, 0.75], [0.72, 1]],
  simpleHeadLevels: [[0, 0.9]],
  mask: (tile, x0, z0, size) => cropMask(tile.crops, x0, z0, size),
  plant: (_x, _z, _shade, _height, tone) => ({ height: CROP_HEIGHT * (0.9 + ((tone - 0.85) / 0.3) * 0.2), head: true, tuft: false }),
}

/** What a winter leaves of the wheat: short stubble through the snow, which hides no one. */
export const STUBBLE: Growth = {
  ...WHEAT,
  palette: [0, 0xcfc28c].map((hex) => Color.hex(hex)),
  width: 0.05,
  levels: [[0, 1], [0.55, 0.8]],
  simpleLevels: [[0, 1]],
  plant: (_x, _z, _shade, height, _tone, chance) => (chance < 0.7 ? { height: 0.1 + (height - 0.3) * 0.35, head: false, tuft: false } : null),
}

/**
 * The blades of chunk `(cx, cz)`: `share` (0..1) of the `perMetre` a square metre a full chunk has.
 * A smaller share is the first blades of a larger one, so a chunk that is rebuilt with more keeps the
 * blades it had. `shadeAt` is the mask (0 where nothing grows), `heightAt` the ground (null where it is
 * not loaded).
 */
export function chunkBlades(cx: number, cz: number, perMetre: number, share: number, shadeAt: (x: number, z: number) => number, heightAt: (x: number, z: number) => number | null, growth: Growth = MEADOW): Blade[] {
  const next = random(Math.imul(cx, 0x9e3779b1) ^ Math.imul(cz, 0x85ebca77) ^ 0x2545f491)
  const total = Math.round(perMetre * GRASS_CHUNK * GRASS_CHUNK)
  const count = Math.min(total, Math.ceil(total * share))
  const blades: Blade[] = []
  for (let i = 0; i < count; i++) {
    // Every draw is taken whether the blade grows or not, so blade i is the same at any share.
    const x = (cx + next()) * GRASS_CHUNK
    const z = (cz + next()) * GRASS_CHUNK
    const height = 0.3 + next() * next() * 0.4
    const turn = next() * Math.PI
    const tone = 0.85 + next() * 0.3
    const chance = next()
    const shade = shadeAt(x, z)
    if (shade === 0) continue
    const planted = growth.plant(x, z, shade, height, tone, chance)
    if (!planted) continue
    const grown = planted.height
    const y = heightAt(x, z)
    if (y === null) continue
    const normal = normalAt((nx, nz) => heightAt(nx, nz) ?? y, x, z, NORMAL_STEP)
    blades.push({ x, z, y, height: grown, turn, rank: i / total, shade, tone, normal, ...(planted.head ? { head: true } : {}) })
    // A tuft: two more blades about the first, shorter and turned, at its rank.
    if (planted.tuft) {
      const wet = PALETTE_WET[shade] ?? 0
      for (let k = 1; k <= 2; k++) {
        const angle = turn * 2 + k * 2.1
        const out = 0.05 + 0.05 * k
        const tx = x + Math.cos(angle) * out, tz = z + Math.sin(angle) * out
        if (inMarshPool(tx, tz, wet)) continue
        blades.push({ x: tx, z: tz, y, height: grown * (1 - 0.17 * k), turn: turn + k * 1.1, rank: i / total, shade, tone: tone * (1 - 0.05 * k), normal })
      }
    }
  }
  return blades
}

/** The tallest blade, metres, a reed: the colour's alpha (unorm) holds the height as a share of it. */
const MAX_BLADE = 2

/** Metres either side of a root the ground is sampled at for its slope: about the terrain's own grid. */
const NORMAL_STEP = 2

/** Bend, as tip offset per metre of blade, a mover or blast can give before it stops adding. */
const MAX_BEND = 1.35

/**
 * The blades' geometry: two segments and a tip (5 vertices) a blade, or for far-off chunks (`simple`)
 * a single triangle, which from there looks the same. Each vertex carries its root, its height above
 * it and the blade's rank in `custom`, and the blade's full height (over `MAX_BLADE`) in the colour's alpha, for the
 * vertex shader to bend, thin and widen it about its root.
 *
 * Written straight in the renderer's vertex layout (`GeometryData.packed`, as `packVertices` lays it
 * out), so the frame a chunk is built in neither fills four arrays nor has them interleaved again.
 */
export function bladeGeometry(blades: readonly Blade[], simple = false, growth: Growth = MEADOW): Geometry {
  const perBlade = simple ? 3 : 5
  const packed = new ArrayBuffer(blades.length * perBlade * VERTEX_STRIDE)
  const f32 = new Float32Array(packed)
  const u16 = new Uint16Array(packed)
  const triangles = simple ? 1 : 3
  const indices = new Uint32Array(blades.length * triangles * 3)
  // Base width and how much of it is left at each level up the blade.
  const width = growth.width
  const levels = simple ? growth.simpleLevels : growth.levels
  const headLevels = simple ? growth.simpleHeadLevels : growth.headLevels
  let [x0, y0, z0, x1, y1, z1] = [Infinity, Infinity, Infinity, -Infinity, -Infinity, -Infinity]
  let v = 0
  let n = 0
  /** One vertex of blade `b`, `along` up it in `light`: position, normal, colour (alpha its height), and its root, height and rank in `custom`. */
  const put = (b: Blade, stem: Color, x: number, y: number, z: number, along: number, light: number) => {
    const f = v * (VERTEX_STRIDE / 4)
    f32[f] = x
    f32[f + 1] = y
    f32[f + 2] = z
    f32[f + 3] = b.normal[0]
    f32[f + 4] = b.normal[1]
    f32[f + 5] = b.normal[2]
    f32[f + 10] = b.x
    f32[f + 11] = b.z
    f32[f + 12] = along
    f32[f + 13] = b.rank
    // Darker at the root, where it meets the ground's own colour, lighter toward the tip.
    const head = b.head === true && along > b.height * 0.7
    const color = head ? growth.head : stem
    const k = head ? (growth.headToned ? b.tone : 1) : b.tone * light
    const c = v * (VERTEX_STRIDE / 2) + 16
    u16[c] = unorm16(color.r * k)
    u16[c + 1] = unorm16(color.g * k)
    u16[c + 2] = unorm16(color.b * k)
    u16[c + 3] = unorm16(Math.min(1, b.height / MAX_BLADE))
    if (x < x0) x0 = x
    if (x > x1) x1 = x
    if (y < y0) y0 = y
    if (y > y1) y1 = y
    if (z < z0) z0 = z
    if (z > z1) z1 = z
    v++
  }
  for (const b of blades) {
    const sx = Math.cos(b.turn) * width * 0.5
    const sz = Math.sin(b.turn) * width * 0.5
    const stem = growth.palette[b.shade]!
    const first = v
    for (const [at, keep] of b.head ? headLevels : levels) {
      const y = b.y + at * b.height
      const light = 0.92 + at * 0.35
      put(b, stem, b.x - sx * keep, y, b.z - sz * keep, at * b.height, light)
      put(b, stem, b.x + sx * keep, y, b.z + sz * keep, at * b.height, light)
    }
    put(b, stem, b.x, b.y + b.height, b.z, b.height, 1.3)
    indices[n] = first
    indices[n + 1] = first + 1
    indices[n + 2] = first + 2
    if (!simple) {
      indices[n + 3] = first + 1
      indices[n + 4] = first + 3
      indices[n + 5] = first + 2
      indices[n + 6] = first + 2
      indices[n + 7] = first + 3
      indices[n + 8] = first + 4
    }
    n += triangles * 3
  }
  // About the box the blades stand in, out to its corners; they bend outside where they stand, so the
  // bounds are grown by as far as they bend, and the chunk is not culled too soon.
  const bounds = v === 0 ? { x: 0, y: 0, z: 0, radius: 0 } : { x: (x0 + x1) / 2, y: (y0 + y1) / 2, z: (z0 + z1) / 2, radius: Math.hypot(x1 - x0, y1 - y0, z1 - z0) / 2 + MAX_BEND }
  return new Geometry({ packed, indices, bounds })
}

/** `v` (0..1) as an unsigned 16-bit fraction, as the renderer stores a colour. */
function unorm16(v: number) {
  return Math.round(Math.min(1, Math.max(0, v)) * 65535)
}

/**
 * The blades' shader. `displace` bends each blade about its root: the tip moves furthest, along an
 * arc so the blade keeps its length. Three things lean it, summed: the wind (a slow gust travelling
 * downwind over a quick sway), every mover within reach (away from it) and every blast's shock
 * front (outward, then a damped spring back).
 *
 * With distance the field thins: past `fade.z` metres only the blades ranked under (fade.z / d)²
 * show, those near the cut shrinking away, and the ones left grow wider to fill in. From `fade.x`
 * to `fade.y` the last of them shrink to nothing.
 */
const GRASS_VERTEX_WGSL = /* wgsl */ `
fn grassLean(root: vec2f, phase: f32) -> vec2f {
  let time = frame.fog.w;
  let wind = params.wind.xy;
  let gust = sin(dot(root, wind) * 0.09 - time * 1.3) * 0.5 + 0.5;
  let sway = sin(time * 2.1 + phase + dot(root, vec2f(0.61, 0.37))) * 0.35 + sin(time * 3.7 + phase * 1.7) * 0.15;
  var lean = wind * params.wind.z * (0.25 + gust * 0.75 + sway);
  var push = vec2f(0.0);
  var movers = array<vec4f, ${BENDERS}>(${Array.from({ length: BENDERS }, (_, i) => `params.b${i}`).join(', ')});
  for (var i = 0u; i < ${BENDERS}u; i++) {
    let b = movers[i];
    let away = root - b.xy;
    let d = length(away);
    if (b.z > 0.0 && d < b.z) {
      let k = 1.0 - d / b.z;
      push += away / max(d, 0.05) * k * k * b.w;
    }
  }
  var blasts = array<vec4f, ${BLASTS}>(${Array.from({ length: BLASTS }, (_, i) => `params.e${i}`).join(', ')});
  for (var i = 0u; i < ${BLASTS}u; i++) {
    let e = blasts[i];
    let away = root - e.xy;
    let d = length(away);
    let reach = e.w * 3.0;
    // Seconds since the shock front, racing out at 40 m/s, passed this blade.
    let since = time - e.z - d / 40.0;
    if (e.w > 0.0 && d < reach && since > 0.0 && since < ${BLAST_LIFE.toFixed(1)}) {
      let k = 1.0 - d / reach;
      // Laid flat at once, then up again over a second or so, with a small overshoot.
      let spring = exp(-since * 1.6) * cos(since * 2.6) * smoothstep(0.0, 0.04, since);
      push += away / max(d, 0.05) * k * spring * 2.5;
    }
  }
  // Movers and blasts can only push so far, however many there are.
  let p = length(push);
  if (p > ${MAX_BEND.toFixed(2)}) {
    push *= ${MAX_BEND.toFixed(2)} / p;
  }
  return lean + push;
}

fn displace(world: vec3f, custom: vec4f, color: vec4f) -> vec3f {
  let root = custom.xy;
  let along = custom.z;
  let rank = custom.w;
  let height = max(color.a * ${MAX_BLADE.toFixed(1)}, 0.05);
  let base = vec3f(root.x, world.y - along, root.y);
  let eye = distance(base.xz, frame.cameraPos.xz);
  let kept = min(1.0, pow(params.fade.z / max(eye, 0.01), 2.0));
  let grow = (1.0 - smoothstep(kept * 0.6, kept, rank)) * (1.0 - smoothstep(params.fade.x, params.fade.y, eye));
  // Fewer blades further off, each wider, so the field still reads as a field.
  let wide = clamp(eye / params.fade.z, 1.0, 3.5);
  let side = vec2f(world.x - root.x, world.z - root.y) * wide * grow;
  var p = base + vec3f(side.x, along * grow, side.y);
  // Craters (the scene's, SCORCH.md) lay the blades round their rims flat, away from the middle, and
  // char the ground: the same for every vertex of a blade, so the fragment is handed how burnt it is.
  let crater = craterGrass(root);
  vertexExtra[0] = vec4f(crater.z, 0.0, 0.0, 0.0);
  if (along <= 0.0 || grow <= 0.0) {
    return p;
  }
  let lean = grassLean(root, fract(rank * 7919.0) * 6.2832) + crater.xy;
  let amount = length(lean);
  if (amount < 1e-4) {
    return p;
  }
  // Bent through angle a, a blade of height h has its tip at h·sin a out and h·cos a up; the
  // levels below follow the square of their height, which curves the blade.
  let a = min(atan(amount), 1.35);
  let t = along / height;
  let h = height * grow;
  let out = lean / amount * sin(a) * h * t * t;
  return p + vec3f(out.x, -(1.0 - cos(a)) * h * t * t, out.y);
}
`

/**
 * Lit as the terrain under it is (by the ground's normal at the root, with the terrain's own
 * eight-band ramp, and in its shadows), so the root melts into the ground and only the colour
 * toward the tip tells a blade from the earth.
 */
const GRASS_WGSL = /* wgsl */ `
${GROUND_LIGHT_WGSL}

fn surface(s: Surface) -> vec4f {
  // The ground's normal, as the blade carries it: not turned over on the blade's back face.
  let ground = select(-s.normal, s.normal, s.front);
  // In the shadows the ground lies in: one tap, as the blades cover a lot of the screen.
  let lit = groundLight(s.world, ground, sunShadowTap(s.world, ground));
  var c = s.color.rgb;
  // Washed out with distance: the dark roots and bright tips even out, and the green pales and greys
  // a little toward the sky, so far grass is a soft haze over the ground rather than a field of lines.
  let far = smoothstep(params.wash.x, params.wash.y, s.viewDepth);
  let t = clamp(s.custom.z / max(s.color.a * ${MAX_BLADE.toFixed(1)}, 0.05), 0.0, 1.0);
  let even = c / (0.92 + 0.38 * t) * 1.1;
  c = mix(c, even, far * 0.85);
  // Burnt where the ground under it is charred, most at the tips.
  c = mix(c, vec3f(0.035, 0.028, 0.02), s.extra[0].x * (0.6 + 0.4 * t));
  let grey = vec3f(dot(c, vec3f(0.3, 0.59, 0.11)));
  c = mix(c, mix(grey, frame.fogColor.rgb * 0.5, 0.3) * 1.15, far * 0.35);
  return vec4f(c / PI * lit, 1.0);
}
`

/** Something that bends the grass this frame: a foot, a wheel, a rotor's downwash. */
export interface Bender {
  x: number
  z: number
  /** Metres it reaches. */
  radius: number
  /** Lean at its centre, as tip offset per metre of blade. */
  strength: number
}

interface Chunk {
  mesh: Mesh
  /** The share of the full count of blades it holds. */
  share: number
  /** Its tile changed under it: to be rebuilt, shown as it is until then. */
  stale: boolean
}

/** Metres a blade keeps from bare ground. */
const EDGE = 0.8
const BENDER_KEYS = Array.from({ length: BENDERS }, (_, i) => `b${i}`)
const BLAST_KEYS = Array.from({ length: BLASTS }, (_, i) => `e${i}`)

/** Sets a uniform's four values; whether any changed. */
function set4(v: number[], a: number, b: number, c: number, d: number) {
  if (v[0] === a && v[1] === b && v[2] === c && v[3] === d) return false
  v[0] = a
  v[1] = b
  v[2] = c
  v[3] = d
  return true
}

/** The share of a crater's radius that is bare earth: the rim keeps its grass, thinned by the slope. */
const CRATER_BARE = 0.8
const EDGE_PROBES = [[EDGE, 0], [-EDGE, 0], [0, EDGE], [0, -EDGE]] as const

const chunkKey = (cx: number, cz: number) => (cz + 32768) * 65536 + (cx + 32768)

export class Grass {
  readonly root = new Group()
  private readonly material: ShaderMaterial
  private readonly masks = new Map<number, GrassMask>()
  /** Each tile's stony ground (`TileData.stones`), where the grass thins out between the stones. */
  private readonly stones = new Map<number, Uint8Array>()
  private readonly chunks = new Map<number, Chunk>()
  /** Where the camera was when the chunks were last looked over, and whether something since calls for another look. */
  private scannedX = Infinity
  private scannedZ = Infinity
  private rescan = true
  private density: GrassDensity = GRASS_FULL
  private enabled = true
  /** Ring of the latest blasts: x, z, start (seconds on the shader's clock), radius. */
  private readonly blasts: [number, number, number, number][] = Array.from({ length: BLASTS }, () => [0, 0, -1000, 0])
  private nextBlast = 0
  private readonly picked: Bender[] = []
  /** Craters shells dug: bare earth inside, no grass grows there. Keyed by `x,z,radius` so telling of one twice adds nothing. */
  private readonly craters = new Map<string, { x: number; z: number; radius: number }>()
  /** Rocks as `[x, z, radius²]` runs, by the chunk they stand over and then by tile, so no blade grows through one. */
  private readonly rocks = new Map<number, Map<number, number[]>>()

  constructor(
    private readonly halfExtent: number,
    private readonly tileSize: number,
    private readonly heightAt: (x: number, z: number) => number | null,
    private growth: Growth = MEADOW,
  ) {
    this.root.name = growth.name
    // The wind's direction along x and z (its average heading until `blowing` says otherwise), and how far it leans a blade.
    const uniforms: ShaderMaterial['uniforms'] = { wind: [Math.cos(WIND_HEADING), Math.sin(WIND_HEADING), 0.18, 0], fade: [0, 0, 1, 0], wash: [0, 1] }
    for (let i = 0; i < BENDERS; i++) uniforms[`b${i}`] = [0, 0, 0, 0]
    for (let i = 0; i < BLASTS; i++) uniforms[`e${i}`] = [0, 0, -1000, 0]
    this.material = new ShaderMaterial({ wgsl: GRASS_WGSL, vertex: GRASS_VERTEX_WGSL, uniforms, side: 'double', vertexColors: true, shadows: true })
    this.applyDensity()
  }

  private tileKey(col: number, row: number) {
    return row * 65536 + col
  }

  /** Grows grass on a streamed-in tile, by `mask` when it was worked out already (`prepareTile`). Chunks already built over it are rebuilt. */
  addTile(ref: TileRef, tile: TileData, ground: TileGround | null, mask?: GrassMask | null) {
    const x0 = -this.halfExtent + ref.col * this.tileSize
    const z0 = -this.halfExtent + ref.row * this.tileSize
    this.rescan = true
    this.masks.set(this.tileKey(ref.col, ref.row), mask ?? this.growth.mask(tile, x0, z0, this.tileSize, ground))
    const tileKey = this.tileKey(ref.col, ref.row)
    if (tile.stones) this.stones.set(tileKey, tile.stones)
    else this.stones.delete(tileKey)
    this.dropRocks(tileKey)
    for (const tree of tile.trees) {
      if (!isRock(tree.kind)) continue
      // The rock's foot: a little inside its widest, where it meets the ground.
      const radius = tree.crown * 0.85
      for (let cz = Math.floor((tree.z - radius) / GRASS_CHUNK); cz <= Math.floor((tree.z + radius) / GRASS_CHUNK); cz++) {
        for (let cx = Math.floor((tree.x - radius) / GRASS_CHUNK); cx <= Math.floor((tree.x + radius) / GRASS_CHUNK); cx++) {
          const byTile = this.rocks.get(chunkKey(cx, cz)) ?? new Map<number, number[]>()
          this.rocks.set(chunkKey(cx, cz), byTile)
          const list = byTile.get(tileKey) ?? []
          list.push(tree.x, tree.z, radius * radius)
          byTile.set(tileKey, list)
        }
      }
    }
    this.dropOver(x0, z0)
  }

  private dropRocks(tileKey: number) {
    for (const [key, byTile] of this.rocks) {
      if (byTile.delete(tileKey) && byTile.size === 0) this.rocks.delete(key)
    }
  }

  /** The ground of a tile moved: its blades are grown again on it. */
  regrowTile(ref: Pick<TileRef, 'col' | 'row'>) {
    this.dropOver(-this.halfExtent + ref.col * this.tileSize, -this.halfExtent + ref.row * this.tileSize)
  }

  removeTile(ref: Pick<TileRef, 'col' | 'row'>) {
    this.rescan = true
    this.masks.delete(this.tileKey(ref.col, ref.row))
    this.dropRocks(this.tileKey(ref.col, ref.row))
    this.stones.delete(this.tileKey(ref.col, ref.row))
    this.dropOver(-this.halfExtent + ref.col * this.tileSize, -this.halfExtent + ref.row * this.tileSize)
  }

  /** Marks the chunks touching the tile at `(x0, z0)` for `update` to build again. */
  private dropOver(x0: number, z0: number) {
    this.rescan = true
    const x1 = x0 + this.tileSize, z1 = z0 + this.tileSize
    for (const [key, chunk] of this.chunks) {
      const cx = (key % 65536) - 32768, cz = Math.floor(key / 65536) - 32768
      if ((cx + 1) * GRASS_CHUNK < x0 || cx * GRASS_CHUNK > x1 || (cz + 1) * GRASS_CHUNK < z0 || cz * GRASS_CHUNK > z1) continue
      chunk.stale = true
    }
  }

  private drop(key: number, chunk: Chunk) {
    this.root.remove(chunk.mesh)
    chunk.mesh.geometry.dispose()
    this.chunks.delete(key)
  }

  /** 0 where no grass grows (or the tile is not loaded), else the colour to grow it in. */
  shadeAt(x: number, z: number): number {
    const grid = Math.max(1, Math.ceil((2 * this.halfExtent) / this.tileSize))
    const cell = (v: number) => Math.min(Math.max(Math.floor((v + this.halfExtent) / this.tileSize), 0), grid - 1)
    const mask = this.masks.get(this.tileKey(cell(x), cell(z)))
    if (!mask) return 0
    const column = Math.floor((x - mask.x0) / SURFACE_CELL)
    const row = Math.floor((z - mask.z0) / SURFACE_CELL)
    if (column < 0 || row < 0 || column >= mask.side || row >= mask.side) return 0
    return mask.cells[row * mask.side + column]!
  }

  /** How stony the ground is at `(x, z)`, 0 to 1. */
  stonyAt(x: number, z: number): number {
    const grid = Math.max(1, Math.ceil((2 * this.halfExtent) / this.tileSize))
    const col = Math.floor((x + this.halfExtent) / this.tileSize), row = Math.floor((z + this.halfExtent) / this.tileSize)
    if (col < 0 || row < 0 || col >= grid || row >= grid) return 0
    const stones = this.stones.get(this.tileKey(col, row))
    if (!stones) return 0
    const n = Math.round(Math.sqrt(stones.length))
    const i = Math.floor(((x + this.halfExtent) / this.tileSize - col) * n), j = Math.floor(((z + this.halfExtent) / this.tileSize - row) * n)
    return (stones[Math.min(j, n - 1) * n + Math.min(i, n - 1)]! & STONES_DENSITY) / STONES_DENSITY
  }

  /**
   * `shadeAt`, but bare within `EDGE` of a cell without grass: the mask is coarse, and a road's edge
   * can be up to half a cell either side of where its cells end.
   */
  growsAt(x: number, z: number): number {
    const shade = this.shadeAt(x, z)
    if (shade === 0) return 0
    for (const c of this.craters.values()) if (Math.hypot(x - c.x, z - c.z) < c.radius) return 0
    for (const list of this.rocks.get(chunkKey(Math.floor(x / GRASS_CHUNK), Math.floor(z / GRASS_CHUNK)))?.values() ?? []) {
      for (let i = 0; i < list.length; i += 3) if ((x - list[i]!) ** 2 + (z - list[i + 1]!) ** 2 < list[i + 2]!) return 0
    }
    for (const [dx, dz] of EDGE_PROBES) if (this.shadeAt(x + dx, z + dz) === 0) return 0
    // Between stones a tuft here and there; on the stoniest ground none.
    const stony = this.stonyAt(x, z)
    if (stony > 0 && (Math.abs(Math.sin(x * 12.9898 + z * 78.233)) * 43758.5453) % 1 < stony * 2) return 0
    return shade
  }

  /**
   * A shell dug a crater of `radius` metres at `(x, z)`: the ground there has moved, so the blades over
   * it are grown again on the new ground, and the grass is gone from the bowl, which is bare earth now.
   */
  dent(x: number, z: number, radius: number) {
    const key = `${x},${z},${radius}`
    if (this.craters.has(key)) return
    this.rescan = true
    this.craters.set(key, { x, z, radius: radius * CRATER_BARE })
    for (const [chunkAt, chunk] of this.chunks) {
      const cx = (chunkAt % 65536) - 32768, cz = Math.floor(chunkAt / 65536) - 32768
      if ((cx + 1) * GRASS_CHUNK < x - radius || cx * GRASS_CHUNK > x + radius || (cz + 1) * GRASS_CHUNK < z - radius || cz * GRASS_CHUNK > z + radius) continue
      chunk.stale = true
    }
  }

  /** Which way the wind blows, a unit `[x, z]`: the blades lean along it, the trees and the smoke alike. */
  blowing(direction: readonly [number, number]) {
    const wind = this.material.uniforms.wind as number[]
    wind[0] = direction[0]
    wind[1] = direction[1]
    this.material.needsUpdate()
  }

  setEnabled(on: boolean) {
    this.rescan = true
    this.enabled = on
    this.root.visible = on
    if (!on) for (const [key, chunk] of this.chunks) this.drop(key, chunk)
  }

  /** Grows something else where it grew, a field's stubble for its wheat: the tiles' masks are kept. */
  setGrowth(growth: Growth) {
    if (growth === this.growth) return
    this.rescan = true
    this.growth = growth
    for (const [key, chunk] of this.chunks) this.drop(key, chunk)
  }

  setDensity(density: GrassDensity) {
    if (density === this.density) return
    this.rescan = true
    this.density = density
    for (const [key, chunk] of this.chunks) this.drop(key, chunk)
    this.applyDensity()
  }

  private applyDensity() {
    const { full, far } = this.density
    this.material.uniforms.fade = [far * 0.75, far, full, 0]
    this.material.uniforms.wash = [full, far * 0.85]
    this.material.needsUpdate()
  }

  /** A blast at `(x, z)` on the ground: its shock flattens the grass out to three times `radius`. */
  blast(x: number, z: number, radius: number) {
    this.blasts[this.nextBlast] = [x, z, performance.now() / 1000, radius]
    this.nextBlast = (this.nextBlast + 1) % BLASTS
  }

  /** Builds and drops chunks around the camera at `(x, z)`, and hands the shader this frame's benders. */
  update(x: number, z: number, benders: readonly Bender[]) {
    if (!this.enabled) return
    const density = this.density
    const { far } = density
    const reach = Math.ceil(far / GRASS_CHUNK) + 1
    const ccx = Math.floor(x / GRASS_CHUNK), ccz = Math.floor(z / GRASS_CHUNK)
    // From the camera to the nearest point of a chunk.
    const distance = (cx: number, cz: number) => {
      const dx = Math.max(cx * GRASS_CHUNK - x, 0, x - (cx + 1) * GRASS_CHUNK)
      const dz = Math.max(cz * GRASS_CHUNK - z, 0, z - (cz + 1) * GRASS_CHUNK)
      return Math.hypot(dx, dz)
    }
    // Which chunks to build is the same answer until the camera has moved or the ground changed
    // under one: most frames skip the few hundred chunks' worth of looking.
    const moved = Math.abs(x - this.scannedX) > RESCAN_DISTANCE || Math.abs(z - this.scannedZ) > RESCAN_DISTANCE
    if (moved || this.rescan) {
      this.scannedX = x
      this.scannedZ = z
      this.rescan = this.build(x, z, far, reach, ccx, ccz, distance)
    }
    this.bend(x, z, far, benders)
  }

  /** Drops the chunks out of reach and builds the nearest of those wanted. True when more are wanted than a frame builds. */
  private build(x: number, z: number, far: number, reach: number, ccx: number, ccz: number, distance: (cx: number, cz: number) => number): boolean {
    const density = this.density
    // A chunk is kept a little past where it would be built, so standing on the line does not churn it.
    for (const [key, chunk] of this.chunks) {
      const cx = (key % 65536) - 32768, cz = Math.floor(key / 65536) - 32768
      if (distance(cx, cz) > far + GRASS_CHUNK) this.drop(key, chunk)
    }
    const wanted: { cx: number; cz: number; d: number; share: number }[] = []
    for (let cz = ccz - reach; cz <= ccz + reach; cz++) {
      for (let cx = ccx - reach; cx <= ccx + reach; cx++) {
        const d = distance(cx, cz)
        if (d > far) continue
        const share = tierFor(density, d - LOOKAHEAD)
        const chunk = this.chunks.get(chunkKey(cx, cz))
        // More blades as the camera nears; fewer only once it is a further LOOKAHEAD off, so walking to
        // and fro does not churn it.
        const thin = chunk !== undefined && chunk.share > tierFor(density, d - 2 * LOOKAHEAD)
        if (!chunk || chunk.stale || chunk.share < share || thin) wanted.push({ cx, cz, d, share: thin ? share : Math.max(share, chunk?.share ?? 0) })
      }
    }
    wanted.sort((a, b) => a.d - b.d)
    const start = performance.now()
    let built = 0
    for (const { cx, cz, share } of wanted) {
      if (built > 0 && performance.now() - start >= BUILD_MS) break
      built++
      const key = chunkKey(cx, cz)
      const blades = chunkBlades(cx, cz, density.perMetre, share, (bx, bz) => this.growsAt(bx, bz), this.heightAt, this.growth)
      const geometry = bladeGeometry(blades, share <= this.growth.simpleShare, this.growth)
      // A megabyte a chunk that nothing reads again: a chunk that changes is built anew.
      geometry.uploadOnce = true
      const mesh = new Mesh(geometry, this.material)
      // An empty chunk is kept too, so it is not tried again every frame.
      mesh.visible = blades.length > 0
      // The old blades stay until the new ones are in: the same blades, so the swap does not show.
      const old = this.chunks.get(key)
      if (old) this.drop(key, old)
      this.root.add(mesh)
      this.chunks.set(key, { mesh, share, stale: false })
    }
    return wanted.length > built
  }

  /** Hands the shader this frame's benders and blasts. */
  private bend(x: number, z: number, far: number, benders: readonly Bender[]) {
    // The movers nearest the camera, where the bending shows.
    this.picked.length = 0
    for (const b of benders) if (Math.hypot(b.x - x, b.z - z) < Math.min(far, 60)) this.picked.push(b)
    this.picked.sort((a, b) => Math.hypot(a.x - x, a.z - z) - Math.hypot(b.x - x, b.z - z))
    // Written in place, and uploaded only when something moved: most frames nothing near is bending it.
    const u = this.material.uniforms
    let changed = false
    for (let i = 0; i < BENDERS; i++) {
      const b = this.picked[i]
      changed = set4(u[BENDER_KEYS[i]!] as number[], b ? b.x : 0, b ? b.z : 0, b ? b.radius : 0, b ? b.strength : 0) || changed
    }
    const now = performance.now() / 1000
    for (let i = 0; i < BLASTS; i++) {
      const e = this.blasts[i]!
      const live = now - e[2] < BLAST_LIFE + e[3] / 10
      changed = set4(u[BLAST_KEYS[i]!] as number[], live ? e[0] : 0, live ? e[1] : 0, live ? e[2] : -1000, live ? e[3] : 0) || changed
    }
    if (changed) this.material.needsUpdate()
  }

  dispose() {
    for (const [key, chunk] of this.chunks) this.drop(key, chunk)
    this.material.dispose()
  }
}
