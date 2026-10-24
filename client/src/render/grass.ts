import { Color, Geometry, Group, Mesh, ShaderMaterial } from '@ragnarok/engine'
import { COVER_INDUSTRIAL, COVER_PARK, COVER_URBAN, COVER_WOOD, type Point2, type TileData, type TileRef } from '@/world'
import { rasterSurface, SURFACE_CELL, SURFACE_GRASS } from '@/sim/surface'
import type { TileGround } from '@/sim/terrain'
import { COVER_COLORS, GRASS_COLOR } from './cover'
import { GROUND_LIGHT_WGSL } from './groundlight'
import { normalAt } from './shade'

/**
 * Grass: short blades near the camera that lean with the wind, part around whoever walks or drives
 * through them, and are flattened by a blast's shock and spring back. It is all for show: nothing
 * on the server knows it is there, and it hides nothing.
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
/** Chunks built a frame, at most, so walking into a meadow does not hitch. */
const BUILDS_PER_FRAME = 3
/** Metres ahead of need a chunk is given more blades, so they are there before they show. */
const LOOKAHEAD = 6
/** Chunks holding at most this share of the blades are far enough off to draw each as one triangle. */
const SIMPLE_SHARE = 1 / 8

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

/** The share of the blades that shows at `distance` metres. */
export function keptAt(density: GrassDensity, distance: number) {
  return Math.min(1, (density.full / Math.max(distance, 1e-3)) ** 2)
}

/** The power-of-two share of the blades a chunk holds when the nearest it is to the camera is `distance`. */
export function tierFor(density: GrassDensity, distance: number) {
  const kept = keptAt(density, Math.max(0, distance))
  return 2 ** Math.max(-6, Math.ceil(Math.log2(kept)))
}

/** Grass colour per cell of a mask: 0 for no grass, else an index into `PALETTE`. */
const PALETTE = [0, GRASS_COLOR, COVER_COLORS[COVER_PARK]!, COVER_COLORS[COVER_WOOD]!].map((hex) => Color.hex(hex))
const COVER_SHADE: Record<number, number> = { [COVER_PARK]: 2, [COVER_WOOD]: 3, [COVER_URBAN]: 0, [COVER_INDUSTRIAL]: 0 }

/** Where one tile has grass, on the surface grid (`SURFACE_CELL` metres a cell). */
export interface GrassMask {
  x0: number
  z0: number
  side: number
  cells: Uint8Array
}

/** Calls `paint(row, column)` for every cell of the grid whose centre is inside `rings` (even-odd). */
function fillRings(rings: readonly Point2[][], x0: number, z0: number, side: number, paint: (row: number, column: number) => void) {
  for (let row = 0; row < side; row++) {
    const z = z0 + (row + 0.5) * SURFACE_CELL
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
      const from = Math.max(0, Math.ceil((crossings[k]! - x0) / SURFACE_CELL - 0.5))
      const to = Math.min(side - 1, Math.floor((crossings[k + 1]! - x0) / SURFACE_CELL - 0.5))
      for (let column = from; column <= to; column++) paint(row, column)
    }
  }
}

/**
 * Where grass grows on a tile: ground the terrain paints green (plain grass, parks and woods), that
 * is not a road, track, water or sand, and not under a building. Built-up land is left bare: the
 * terrain paints it grey, and its yards are full of houses.
 */
export function grassMask(tile: Pick<TileData, 'cover' | 'roads' | 'water' | 'buildings' | 'streams'> & Partial<Pick<TileData, 'rails'>>, x0: number, z0: number, size: number, ground: TileGround | null = null): GrassMask {
  const surface = rasterSurface(tile, x0, z0, size, ground)
  const { side } = surface
  const cells = new Uint8Array(side * side).fill(1)
  for (const cover of tile.cover) {
    const shade = COVER_SHADE[cover.kind]
    if (shade === undefined) continue
    fillRings(cover.rings, x0, z0, side, (row, column) => (cells[row * side + column] = shade))
  }
  for (let i = 0; i < cells.length; i++) if (surface.cells[i] !== SURFACE_GRASS) cells[i] = 0
  for (const building of tile.buildings) fillRings([building.footprint], x0, z0, side, (row, column) => (cells[row * side + column] = 0))
  // Streams are drawn as ribbons on the ground: keep the blades out of the water.
  for (const stream of tile.streams) {
    const half = stream.width / 2 + SURFACE_CELL / 2
    for (let i = 1; i < stream.points.length; i++) {
      const [ax, az] = stream.points[i - 1]!
      const [bx, bz] = stream.points[i]!
      const dx = bx - ax, dz = bz - az
      const length2 = dx * dx + dz * dz
      const c0 = Math.max(0, Math.floor((Math.min(ax, bx) - half - x0) / SURFACE_CELL))
      const c1 = Math.min(side - 1, Math.floor((Math.max(ax, bx) + half - x0) / SURFACE_CELL))
      const r0 = Math.max(0, Math.floor((Math.min(az, bz) - half - z0) / SURFACE_CELL))
      const r1 = Math.min(side - 1, Math.floor((Math.max(az, bz) + half - z0) / SURFACE_CELL))
      for (let row = r0; row <= r1; row++) {
        for (let column = c0; column <= c1; column++) {
          const px = x0 + (column + 0.5) * SURFACE_CELL - ax
          const pz = z0 + (row + 0.5) * SURFACE_CELL - az
          const t = length2 > 0 ? Math.min(1, Math.max(0, (px * dx + pz * dz) / length2)) : 0
          if ((px - t * dx) ** 2 + (pz - t * dz) ** 2 <= half * half) cells[row * side + column] = 0
        }
      }
    }
  }
  return { x0, z0, side, cells }
}

/** A small, fast hash to [0, 1): the same chunk always grows the same blades. */
function random(seed: number) {
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
}

/** Smooth noise in 0..1 on a lattice of unit cells. */
function valueNoise(x: number, z: number) {
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

/**
 * The blades of chunk `(cx, cz)`: `share` (0..1) of the `perMetre` a square metre a full chunk has.
 * A smaller share is the first blades of a larger one, so a chunk that is rebuilt with more keeps the
 * blades it had. `shadeAt` is the mask (0 where nothing grows), `heightAt` the ground (null where it is
 * not loaded).
 */
export function chunkBlades(cx: number, cz: number, perMetre: number, share: number, shadeAt: (x: number, z: number) => number, heightAt: (x: number, z: number) => number | null): Blade[] {
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
    const patch = patchAt(x, z)
    // A few stray tufts grow between the patches.
    if (chance > patch * 0.96 + 0.04) continue
    const shade = shadeAt(x, z)
    if (shade === 0) continue
    const y = heightAt(x, z)
    if (y === null) continue
    const normal = normalAt((nx, nz) => heightAt(nx, nz) ?? y, x, z, NORMAL_STEP)
    // Taller in the thick of a patch, short at its edge.
    blades.push({ x, z, y, height: height * (0.55 + 0.6 * patch), turn, rank: i / total, shade, tone, normal })
  }
  return blades
}

/** Metres either side of a root the ground is sampled at for its slope: about the terrain's own grid. */
const NORMAL_STEP = 2

/** Bend, as tip offset per metre of blade, a mover or blast can give before it stops adding. */
const MAX_BEND = 1.35

/**
 * The blades' geometry: two segments and a tip (5 vertices) a blade, or for far-off chunks (`simple`)
 * a single triangle, which from there looks the same. Each vertex carries its root, its height above
 * it and the blade's rank in `custom`, and the blade's full height in the colour's alpha, for the
 * vertex shader to bend, thin and widen it about its root.
 */
export function bladeGeometry(blades: readonly Blade[], simple = false): Geometry {
  const perBlade = simple ? 3 : 5
  const positions = new Float32Array(blades.length * perBlade * 3)
  const normals = new Float32Array(blades.length * perBlade * 3)
  const colors = new Float32Array(blades.length * perBlade * 4)
  const custom = new Float32Array(blades.length * perBlade * 4)
  const triangles = simple ? 1 : 3
  const indices = new Uint32Array(blades.length * triangles * 3)
  // Base width and how much of it is left at each level up the blade.
  const width = 0.07
  const levels = simple ? [[0, 1]] : [[0, 1], [0.55, 0.7]]
  let v = 0
  let n = 0
  for (const b of blades) {
    const sx = Math.cos(b.turn) * width * 0.5
    const sz = Math.sin(b.turn) * width * 0.5
    const color = PALETTE[b.shade]!
    const first = v
    const put = (x: number, y: number, z: number, along: number, light: number) => {
      positions.set([x, y, z], v * 3)
      normals.set(b.normal, v * 3)
      // Darker at the root, where it meets the ground's own colour, lighter toward the tip.
      const k = b.tone * light
      colors.set([color.r * k, color.g * k, color.b * k, Math.min(1, b.height)], v * 4)
      custom.set([b.x, b.z, along, b.rank], v * 4)
      v++
    }
    for (const [at, keep] of levels) {
      const y = b.y + at! * b.height
      const light = 0.92 + at! * 0.35
      put(b.x - sx * keep!, y, b.z - sz * keep!, at! * b.height, light)
      put(b.x + sx * keep!, y, b.z + sz * keep!, at! * b.height, light)
    }
    put(b.x, b.y + b.height, b.z, b.height, 1.3)
    if (simple) indices.set([first, first + 1, first + 2], n)
    else indices.set([first, first + 1, first + 2, first + 1, first + 3, first + 2, first + 2, first + 3, first + 4], n)
    n += triangles * 3
  }
  const geometry = new Geometry({ positions, normals, colors, custom, indices })
  // The blades bend outside where they stand: grow the bounds so the chunk is not culled too soon.
  geometry.bounds = { ...geometry.bounds, radius: geometry.bounds.radius + MAX_BEND }
  return geometry
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
  let height = max(color.a, 0.05);
  let base = vec3f(root.x, world.y - along, root.y);
  let eye = distance(base.xz, frame.cameraPos.xz);
  let kept = min(1.0, pow(params.fade.z / max(eye, 0.01), 2.0));
  let grow = (1.0 - smoothstep(kept * 0.6, kept, rank)) * (1.0 - smoothstep(params.fade.x, params.fade.y, eye));
  // Fewer blades further off, each wider, so the field still reads as a field.
  let wide = clamp(eye / params.fade.z, 1.0, 3.5);
  let side = vec2f(world.x - root.x, world.z - root.y) * wide * grow;
  var p = base + vec3f(side.x, along * grow, side.y);
  if (along <= 0.0 || grow <= 0.0) {
    return p;
  }
  let lean = grassLean(root, fract(rank * 7919.0) * 6.2832);
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
  let t = clamp(s.custom.z / max(s.color.a, 0.05), 0.0, 1.0);
  let even = c / (0.92 + 0.38 * t) * 1.1;
  c = mix(c, even, far * 0.85);
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
const EDGE_PROBES = [[EDGE, 0], [-EDGE, 0], [0, EDGE], [0, -EDGE]] as const

const chunkKey = (cx: number, cz: number) => (cz + 32768) * 65536 + (cx + 32768)

export class Grass {
  readonly root = new Group()
  private readonly material: ShaderMaterial
  private readonly masks = new Map<number, GrassMask>()
  private readonly chunks = new Map<number, Chunk>()
  private density: GrassDensity = GRASS_FULL
  private enabled = true
  /** Ring of the latest blasts: x, z, start (seconds on the shader's clock), radius. */
  private readonly blasts: [number, number, number, number][] = Array.from({ length: BLASTS }, () => [0, 0, -1000, 0])
  private nextBlast = 0
  private readonly picked: Bender[] = []

  constructor(
    private readonly halfExtent: number,
    private readonly tileSize: number,
    private readonly heightAt: (x: number, z: number) => number | null,
  ) {
    const uniforms: ShaderMaterial['uniforms'] = { wind: [0.8, 0.6, 0.18, 0], fade: [0, 0, 1, 0], wash: [0, 1] }
    for (let i = 0; i < BENDERS; i++) uniforms[`b${i}`] = [0, 0, 0, 0]
    for (let i = 0; i < BLASTS; i++) uniforms[`e${i}`] = [0, 0, -1000, 0]
    this.material = new ShaderMaterial({ wgsl: GRASS_WGSL, vertex: GRASS_VERTEX_WGSL, uniforms, side: 'double', vertexColors: true, shadows: true })
    this.applyDensity()
  }

  private tileKey(col: number, row: number) {
    return row * 65536 + col
  }

  /** Grows grass on a streamed-in tile. Chunks already built over it are rebuilt. */
  addTile(ref: TileRef, tile: TileData, ground: TileGround | null) {
    const x0 = -this.halfExtent + ref.col * this.tileSize
    const z0 = -this.halfExtent + ref.row * this.tileSize
    this.masks.set(this.tileKey(ref.col, ref.row), grassMask(tile, x0, z0, this.tileSize, ground))
    this.dropOver(x0, z0)
  }

  removeTile(ref: TileRef) {
    this.masks.delete(this.tileKey(ref.col, ref.row))
    this.dropOver(-this.halfExtent + ref.col * this.tileSize, -this.halfExtent + ref.row * this.tileSize)
  }

  /** Marks the chunks touching the tile at `(x0, z0)` for `update` to build again. */
  private dropOver(x0: number, z0: number) {
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

  /**
   * `shadeAt`, but bare within `EDGE` of a cell without grass: the mask is coarse, and a road's edge
   * can be up to half a cell either side of where its cells end.
   */
  growsAt(x: number, z: number): number {
    const shade = this.shadeAt(x, z)
    if (shade === 0) return 0
    for (const [dx, dz] of EDGE_PROBES) if (this.shadeAt(x + dx, z + dz) === 0) return 0
    return shade
  }

  setEnabled(on: boolean) {
    this.enabled = on
    this.root.visible = on
    if (!on) for (const [key, chunk] of this.chunks) this.drop(key, chunk)
  }

  setDensity(density: GrassDensity) {
    if (density === this.density) return
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
    for (const { cx, cz, share } of wanted.slice(0, BUILDS_PER_FRAME)) {
      const key = chunkKey(cx, cz)
      const blades = chunkBlades(cx, cz, density.perMetre, share, (bx, bz) => this.growsAt(bx, bz), this.heightAt)
      const mesh = new Mesh(bladeGeometry(blades, share <= SIMPLE_SHARE), this.material)
      // An empty chunk is kept too, so it is not tried again every frame.
      mesh.visible = blades.length > 0
      // The old blades stay until the new ones are in: the same blades, so the swap does not show.
      const old = this.chunks.get(key)
      if (old) this.drop(key, old)
      this.root.add(mesh)
      this.chunks.set(key, { mesh, share, stale: false })
    }

    // The movers nearest the camera, where the bending shows.
    this.picked.length = 0
    for (const b of benders) if (Math.hypot(b.x - x, b.z - z) < Math.min(far, 60)) this.picked.push(b)
    this.picked.sort((a, b) => Math.hypot(a.x - x, a.z - z) - Math.hypot(b.x - x, b.z - z))
    const u = this.material.uniforms
    for (let i = 0; i < BENDERS; i++) {
      const b = this.picked[i]
      u[`b${i}`] = b ? [b.x, b.z, b.radius, b.strength] : [0, 0, 0, 0]
    }
    const now = performance.now() / 1000
    for (let i = 0; i < BLASTS; i++) {
      const e = this.blasts[i]!
      u[`e${i}`] = now - e[2] < BLAST_LIFE + e[3] / 10 ? [e[0], e[1], e[2], e[3]] : [0, 0, -1000, 0]
    }
    this.material.needsUpdate()
  }

  dispose() {
    for (const [key, chunk] of this.chunks) this.drop(key, chunk)
    this.material.dispose()
  }
}
