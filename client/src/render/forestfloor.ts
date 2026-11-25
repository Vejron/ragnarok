import { Color, Geometry, Group, Mesh, ShaderMaterial } from '@ragnarok/engine'
import type { TileData, TileRef } from '@/world'
import { WIND_HEADING } from '@/sim/wind'
import { FLOOR_HEATH, FLOOR_LEAF, FLOOR_MIXED, FLOOR_MOSS } from './forest'
import { random, valueNoise } from './grass'
import { SHADE_FLOORS } from './grassmask'
import { GROUND_LIGHT_WGSL } from './groundlight'
import { carveRock, isRock, type Carving } from './rocks'
import { winter } from './season'
import { normalAt } from './shade'

/*
 * What stands on a forest's floor (FOREST-FLOOR.md): bilberry, lingonberry and heather, cushions of
 * moss and lichen, ferns, and among them stumps and mossy stones. Which of them, and how
 * many, follows the stand over the spot (`forest.ts`): a pine heath, the moss under spruce, a birch
 * wood. It is all for show, as the grass is: the server knows none of it, and nothing here is solid.
 *
 * Two layers, each built on the CPU in square chunks about the camera, one merged mesh a chunk. The
 * plants are many and small, so they reach a few tens of metres and thin out over that as the grass
 * does; the stumps and stones are few and reach as far as a wood is seen into. Everything
 * shrinks into its own foot at the end of its reach, so nothing pops.
 */

/** How much stands on the floor: all of it, or less and nearer for phones and tablets. */
export interface FloorDensity {
  /** The share of the plants that is grown. */
  plants: number
  /** Metres from the camera within which every plant shows; past it they thin as the square of the distance. */
  full: number
  /** Metres from the camera where the last plants have shrunk away. */
  far: number
  /** Metres from the camera where the stumps and stones have shrunk away. */
  timber: number
}

export const FLOOR_FULL: FloorDensity = { plants: 1, full: 24, far: 48, timber: 130 }
export const FLOOR_LITE: FloorDensity = { plants: 0.5, full: 15, far: 30, timber: 80 }

/** Side of a chunk of plants, and of one of timber, metres. */
const PLANT_CHUNK = 16
const TIMBER_CHUNK = 32
/** Places tried for a plant, a square metre, and for a piece of timber, a chunk. */
const PLANTS_PER_METRE = 1.5
const TIMBER_PER_CHUNK = 30
/** Chunks of each layer built a frame, at most. */
const BUILDS_PER_FRAME = 2

const hex = (color: number) => Color.hex(color)

/** A low cushion of moss or lichen: how big, what colour, and how lumpy. */
interface Mound {
  /** Radius and height, metres: each a least and a most. */
  radius: readonly [number, number]
  height: readonly [number, number]
  /** Its colour at the foot and at the top. */
  low: Color
  high: Color
  /** 0 for a smooth cushion, to 1 for an uneven one. */
  ragged: number
}

const LICHEN: Mound = { radius: [0.15, 0.38], height: [0.05, 0.11], low: hex(0x8d987c), high: hex(0xb7c0a4), ragged: 0.7 }
const HUMMOCK: Mound = { radius: [0.45, 1.0], height: [0.12, 0.26], low: hex(0x44682f), high: hex(0x6f9c3c), ragged: 0.15 }

/** A dwarf shrub: a clump of sprigs, each a leaf-shaped blade standing out of the floor. */
export interface Shrub {
  /** The clump's radius, metres: a least and a most. */
  radius: readonly [number, number]
  /** Sprigs in a clump: a least and a most. */
  sprigs: readonly [number, number]
  /** A sprig's length, metres, and its width at its widest as a share of that. */
  length: readonly [number, number]
  width: number
  /** How far the sprigs at the clump's rim lean out, radians from upright. */
  splay: number
  /** A sprig's colour at its foot and at its tip. */
  low: Color
  high: Color
  /** The colour of the flowering tips, heather's; none without. */
  bloom?: Color
  /** The share of the tips in flower, 0.45 unless said. */
  blooms?: number
  /** How far the wind moves a tip, as offset per metre of height squared. */
  sway: number
}

const BILBERRY: Shrub = { radius: [0.22, 0.42], sprigs: [12, 16], length: [0.2, 0.34], width: 0.62, splay: 1.0, low: hex(0x2f5226), high: hex(0x5c9a40), sway: 0.6 }
const LINGON: Shrub = { radius: [0.22, 0.42], sprigs: [11, 15], length: [0.1, 0.17], width: 0.8, splay: 1.2, low: hex(0x284626), high: hex(0x467838), sway: 0.2 }
const HEATHER: Shrub = { radius: [0.18, 0.34], sprigs: [11, 15], length: [0.22, 0.38], width: 0.3, splay: 0.7, low: hex(0x3f4a2c), high: hex(0x66783f), bloom: hex(0x9c6c9e), sway: 0.7 }

/** A fern's fronds, at their foot and their tips. */
const FERN_LOW = hex(0x3c6b2c)
const FERN_HIGH = hex(0x7cb848)

/**
 * What a place tried for a plant grows, by the floor: the chance of each, of a place in the thick of
 * its patches. What is left of 1 is bare floor. `fern` is the chance of a fern, where ferns grow.
 */
const PLANTS: Record<number, { grows: readonly (readonly [Shrub | Mound, number])[]; fern: number }> = {
  [FLOOR_MIXED]: { grows: [[BILBERRY, 0.4], [LINGON, 0.14], [HUMMOCK, 0.05]], fern: 0.05 },
  [FLOOR_HEATH]: { grows: [[LINGON, 0.36], [HEATHER, 0.2], [LICHEN, 0.1]], fern: 0.005 },
  [FLOOR_MOSS]: { grows: [[BILBERRY, 0.55], [HUMMOCK, 0.09]], fern: 0.07 },
  [FLOOR_LEAF]: { grows: [[BILBERRY, 0.12], [HUMMOCK, 0.03]], fern: 0.3 },
}

/** A stand's dead wood: its bark, and how much of what faces up the moss has taken. */
interface Deadwood {
  bark: Color
  /** A second colour some of its bark has, a birch's black marks. */
  mark: Color
  moss: number
}

const DEADWOOD: Record<number, Deadwood> = {
  [FLOOR_MIXED]: { bark: hex(0x5f4c3c), mark: hex(0x4a3b2f), moss: 0.6 },
  [FLOOR_HEATH]: { bark: hex(0x7a5438), mark: hex(0x5e4230), moss: 0.25 },
  [FLOOR_MOSS]: { bark: hex(0x574739), mark: hex(0x43372c), moss: 0.9 },
  [FLOOR_LEAF]: { bark: hex(0xd4d0c2), mark: hex(0x3a3632), moss: 0.4 },
}

/** The chance of a stump and of a stone at a place tried for timber, by the floor. */
const TIMBER: Record<number, readonly [number, number]> = {
  [FLOOR_MIXED]: [0.13, 0.2],
  [FLOOR_HEATH]: [0.12, 0.16],
  [FLOOR_MOSS]: [0.14, 0.3],
  [FLOOR_LEAF]: [0.1, 0.12],
}

const MOSS_DARK = hex(0x35561f)
const MOSS_LIGHT = hex(0x6c9a36)
/** A stump's cut face: old wood, greyed. */
const HEARTWOOD = hex(0x9c8560)
const STONE = hex(0x7c7b77)

/** A stone on the floor: small enough to step over, since nothing stops a foot on it. */
const STONE_CARVINGS: Carving[] = [
  { cuts: 3, depth: [0.7, 0.85], squash: 0.75, bump: 0.06 },
  { cuts: 4, depth: [0.6, 0.8], squash: 0.6, bump: 0.05 },
  { cuts: 2, depth: [0.6, 0.75], squash: 0.85, bump: 0.07 },
]

let stoneShapes: Geometry[] | undefined

/** One mesh's arrays as they grow. A vertex's `custom` is its root (x, z), its height over it and its rank. */
export class FloorBuilder {
  readonly positions: number[] = []
  readonly normals: number[] = []
  readonly colors: number[] = []
  readonly custom: number[] = []
  readonly indices: number[] = []

  get vertices() {
    return this.positions.length / 3
  }

  /**
   * A vertex at `p`, lit by normal `n`, of `color` times `tone`, which shrinks into the root at
   * `(rootX, rootY, rootZ)` far off and sways `sway` with the wind. Returns its index.
   */
  vertex(p: readonly number[], n: readonly number[], color: Color, tone: number, sway: number, rootX: number, rootY: number, rootZ: number, rank: number): number {
    this.positions.push(p[0]!, p[1]!, p[2]!)
    this.normals.push(n[0]!, n[1]!, n[2]!)
    this.colors.push(color.r * tone, color.g * tone, color.b * tone, sway)
    this.custom.push(rootX, rootZ, p[1]! - rootY, rank)
    return this.vertices - 1
  }

  geometry(): Geometry {
    return new Geometry({
      positions: new Float32Array(this.positions),
      normals: new Float32Array(this.normals),
      colors: new Float32Array(this.colors),
      custom: new Float32Array(this.custom),
      indices: new Uint32Array(this.indices),
    })
  }
}

export const mix = (a: Color, b: Color, t: number) => new Color(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t)
export const lerp = ([a, b]: readonly [number, number], t: number) => a + (b - a) * t
export const unit = (x: number, y: number, z: number): [number, number, number] => {
  const inv = 1 / Math.max(1e-6, Math.hypot(x, y, z))
  return [x * inv, y * inv, z * inv]
}

/** Sides of a mound and of a stump. */
const MOUND_SIDES = 6
const STUMP_SIDES = 7

/**
 * A cushion at `(x, y, z)` on ground of normal `n`: a ring on the ground, a ring part of the way up
 * and in, and a top. Its foot lies along the slope; it is lit mostly as the ground is, turned a little
 * outward so it has a lit and a shaded side.
 */
export function addMound(out: FloorBuilder, mound: Mound, x: number, y: number, z: number, n: readonly number[], rank: number, next: () => number) {
  const radius = lerp(mound.radius, next()), height = lerp(mound.height, next())
  const turn = next() * Math.PI * 2
  const first = out.vertices
  const rootY = y - 0.04
  for (let ring = 0; ring < 2; ring++) {
    for (let k = 0; k < MOUND_SIDES; k++) {
      const angle = turn + ((k + ring * 0.5) / MOUND_SIDES) * Math.PI * 2
      const uneven = next()
      const r = radius * (ring === 0 ? 0.8 + 0.4 * uneven : 0.42 + 0.3 * uneven)
      const dx = Math.cos(angle) * r, dz = Math.sin(angle) * r
      // On the slope under it: the ground's plane through the root.
      const ground = y - (n[0]! * dx + n[2]! * dz) / n[1]!
      const up = ring === 0 ? -0.04 : height * (1 - mound.ragged * 0.5 * next())
      const lit = unit(n[0]! + Math.cos(angle) * 0.25, n[1]!, n[2]! + Math.sin(angle) * 0.25)
      out.vertex([x + dx, ground + up, z + dz], lit, ring === 0 ? mound.low : mound.high, ring === 0 ? 1 : 0.85 + 0.2 * next(), 0, x, rootY, z, rank)
    }
  }
  const top = out.vertex([x, y + height, z], n, mound.high, 1.08, 0, x, rootY, z, rank)
  for (let k = 0; k < MOUND_SIDES; k++) {
    const a = first + k, b = first + ((k + 1) % MOUND_SIDES)
    const c = first + MOUND_SIDES + k, d = first + MOUND_SIDES + ((k + 1) % MOUND_SIDES)
    out.indices.push(a, c, b, b, c, d, c, top, d)
  }
}

/**
 * A dwarf shrub at `(x, y, z)`: sprigs scattered over its clump, upright in the middle and leaning
 * out toward the rim, each a leaf shape of four points (foot, two at its widest, tip) lit as the
 * ground under it, as a blade of grass is.
 */
export function addShrub(out: FloorBuilder, shrub: Shrub, x: number, y: number, z: number, n: readonly number[], rank: number, next: () => number) {
  const radius = lerp(shrub.radius, next())
  const sprigs = Math.round(lerp(shrub.sprigs, next()))
  const turn = next() * Math.PI * 2
  const rootY = y - 0.02
  for (let s = 0; s < sprigs; s++) {
    // Spread evenly round the clump, a golden angle apart, further out each time.
    const angle = turn + s * 2.39996
    const out01 = Math.sqrt((s + 0.5) / sprigs)
    const [ox, oz] = [Math.cos(angle), Math.sin(angle)]
    const [fx, fz] = [ox * radius * out01 * 0.85, oz * radius * out01 * 0.85]
    const foot = [x + fx, y - (n[0]! * fx + n[2]! * fz) / n[1]! - 0.02, z + fz]
    const length = lerp(shrub.length, next())
    const lean = shrub.splay * out01 * (0.6 + 0.6 * next())
    const [along, up] = [Math.sin(lean) * length, Math.cos(lean) * length]
    // The leaf's flat side is turned to the eye above and beside it: across the way it leans, twisted a little.
    const twist = (next() - 0.5) * 1.2
    const half = length * shrub.width * 0.5
    const [sx, sz] = [(-oz * Math.cos(twist) + ox * Math.sin(twist)) * half, (ox * Math.cos(twist) + oz * Math.sin(twist)) * half]
    const tone = 0.85 + 0.3 * next()
    const tip = shrub.bloom && next() < (shrub.blooms ?? 0.45) ? shrub.bloom : shrub.high
    const first = out.vertices
    const mid = [foot[0]! + ox * along * 0.5, foot[1]! + up * 0.55, foot[2]! + oz * along * 0.5]
    out.vertex(foot, n, shrub.low, tone * 0.8, 0, x, rootY, z, rank)
    out.vertex([mid[0]! - sx, mid[1]!, mid[2]! - sz], n, shrub.high, tone * 0.95, shrub.sway * 0.5, x, rootY, z, rank)
    out.vertex([mid[0]! + sx, mid[1]!, mid[2]! + sz], n, shrub.high, tone * 0.95, shrub.sway * 0.5, x, rootY, z, rank)
    out.vertex([foot[0]! + ox * along, foot[1]! + up, foot[2]! + oz * along], n, tip, tone * 1.2, shrub.sway, x, rootY, z, rank)
    out.indices.push(first, first + 1, first + 2, first + 1, first + 3, first + 2)
  }
}

/**
 * A fern at `(x, y, z)`: five to seven fronds from one foot, each rising, arching over and drooping at
 * its tip. A frond is two segments and a point, lit as the ground under it, as a blade of grass is.
 */
export function addFern(out: FloorBuilder, x: number, y: number, z: number, n: readonly number[], rank: number, next: () => number) {
  const fronds = 5 + Math.floor(next() * 3)
  const size = 0.45 + next() * 0.45
  const turn = next() * Math.PI * 2
  const rootY = y - 0.02
  for (let f = 0; f < fronds; f++) {
    const angle = turn + ((f + next() * 0.6) / fronds) * Math.PI * 2
    const length = size * (0.75 + 0.5 * next())
    const [ox, oz] = [Math.cos(angle), Math.sin(angle)]
    const half = length * 0.16
    const [sx, sz] = [-oz * half, ox * half]
    const first = out.vertices
    // Out and up from the foot: the foot, the widest part two fifths out, and the tip.
    const levels = [[0.04, 0.02, 0.35], [0.42, 0.5, 1]] as const
    for (const [along, up, wide] of levels) {
      const [cx, cy, cz] = [x + ox * length * along, y + length * up, z + oz * length * along]
      const tone = 0.75 + up * 0.7
      const color = up > 0.1 ? FERN_HIGH : FERN_LOW
      out.vertex([cx - sx * wide, cy, cz - sz * wide], n, color, tone, up * 2, x, rootY, z, rank)
      out.vertex([cx + sx * wide, cy, cz + sz * wide], n, color, tone, up * 2, x, rootY, z, rank)
    }
    out.vertex([x + ox * length, y + length * 0.3, z + oz * length], n, FERN_HIGH, 1.15, 1, x, rootY, z, rank)
    out.indices.push(first, first + 1, first + 2, first + 1, first + 3, first + 2, first + 2, first + 3, first + 4)
  }
}

/**
 * A stump at `(x, y, z)`: a trunk sawn off or snapped at knee height, wider at its foot, its bark
 * mossed from the ground up and its cut face grey.
 */
export function addStump(out: FloorBuilder, wood: Deadwood, x: number, y: number, z: number, next: () => number) {
  const radius = 0.14 + next() * 0.16, height = 0.25 + next() * 0.4
  const turn = next() * Math.PI * 2
  const rootY = y - 0.12
  const first = out.vertices
  for (let k = 0; k < STUMP_SIDES; k++) {
    const angle = turn + (k / STUMP_SIDES) * Math.PI * 2
    const [ox, oz] = [Math.cos(angle), Math.sin(angle)]
    const bark = next() < 0.3 ? wood.mark : wood.bark
    const foot = mix(bark, MOSS_DARK, Math.min(1, wood.moss * (0.5 + next())))
    const rim = radius * (0.9 + 0.2 * next())
    out.vertex([x + ox * radius * 1.35, rootY, z + oz * radius * 1.35], unit(ox, 0.3, oz), foot, 0.8, 0, x, rootY, z, 0)
    out.vertex([x + ox * rim, y + height * (0.85 + 0.15 * next()), z + oz * rim], unit(ox, 0.1, oz), bark, 1, 0, x, rootY, z, 0)
  }
  const face = mix(HEARTWOOD, MOSS_LIGHT, wood.moss * next() * 0.8)
  const centre = out.vertex([x, y + height * 0.95, z], [0, 1, 0], face, 1, 0, x, rootY, z, 0)
  const cap = out.vertices
  // The face has its own ring, lit from above.
  for (let k = 0; k < STUMP_SIDES; k++) {
    const p = out.positions.slice((first + k * 2 + 1) * 3, (first + k * 2 + 1) * 3 + 3)
    out.vertex(p, [0, 1, 0], face, 0.9, 0, x, rootY, z, 0)
  }
  for (let k = 0; k < STUMP_SIDES; k++) {
    const j = (k + 1) % STUMP_SIDES
    const [a, b, c, d] = [first + k * 2, first + k * 2 + 1, first + j * 2, first + j * 2 + 1]
    out.indices.push(a, b, c, c, b, d, cap + k, centre, cap + j)
  }
}

/** A stone at `(x, y, z)`, `radius` across and `height` high: one of a few carved shapes, mossed on what faces up. */
export function addStone(out: FloorBuilder, moss: number, x: number, y: number, z: number, radius: number, height: number, next: () => number) {
  stoneShapes ??= STONE_CARVINGS.map((carving, i) => carveRock(carving, 311 + i * 53, 1))
  const shape = stoneShapes[Math.floor(next() * stoneShapes.length)]!
  const turn = next() * Math.PI * 2
  const [cs, sn] = [Math.cos(turn), Math.sin(turn)]
  const tone = 0.85 + 0.3 * next()
  // How far down its sides the moss comes.
  const line = 1.05 - Math.min(1, moss * (0.6 + 0.6 * next())) * 1.1
  const green = mix(MOSS_DARK, MOSS_LIGHT, next())
  const rootY = y - height * 0.3
  const { positions: p, normals: n, colors } = shape
  const first = out.vertices
  for (let i = 0; i < shape.vertexCount; i++) {
    const [px, py, pz] = [p[i * 3]!, p[i * 3 + 1]!, p[i * 3 + 2]!]
    const [nx, ny, nz] = [n![i * 3]!, n![i * 3 + 1]!, n![i * 3 + 2]!]
    // A face is moss or stone whole, as its normal has it.
    const mossy = ny > line
    out.vertex([x + (px * cs - pz * sn) * radius, y + py * height, z + (px * sn + pz * cs) * radius], unit(nx * cs - nz * sn, ny * (radius / height), nx * sn + nz * cs), mossy ? green : STONE, (mossy ? 1 : tone) * colors![i * 3]!, 0, x, rootY, z, 0)
    out.indices.push(first + i)
  }
}

/**
 * The plants' and the timber's shader. `displace` shrinks each thing into its root with distance: past
 * `fade.z` metres only those ranked under (fade.z / d)² show, and from `fade.x` to `fade.y` the last of
 * them go. What is left sways in the wind by its colour's alpha, more the higher over its root.
 *
 * Lit as the terrain is (its ramp, its shadows, one tap), by the normal each vertex carries: the
 * ground's own under a plant, so it sits in the floor's light, and the wood's or the stone's own.
 */
const FLOOR_VERTEX_WGSL = /* wgsl */ `
fn displace(world: vec3f, custom: vec4f, color: vec4f) -> vec3f {
  let root = vec3f(custom.x, world.y - custom.z, custom.y);
  let eye = distance(root.xz, frame.cameraPos.xz);
  let kept = pow(params.fade.z / max(eye, 0.01), 2.0);
  let grow = (1.0 - smoothstep(kept * 0.7, kept, custom.w)) * (1.0 - smoothstep(params.fade.x, params.fade.y, eye));
  var p = root + (world - root) * grow;
  let time = frame.fog.w;
  let sway = sin(time * 1.7 + dot(root.xz, vec2f(0.61, 0.37))) * 0.6 + sin(time * 2.9 + root.x * 1.3) * 0.4;
  let lean = params.wind.xy * params.wind.z * (0.4 + sway) * color.a * custom.z * custom.z * grow;
  return p + vec3f(lean.x, 0.0, lean.y);
}
`

const FLOOR_WGSL = /* wgsl */ `
${GROUND_LIGHT_WGSL}

fn surface(s: Surface) -> vec4f {
  // The normal as the vertex carries it: not turned over on a frond's underside.
  let n = select(-s.normal, s.normal, s.front);
  let lit = groundLight(s.world, n, sunShadowTap(s.world, n));
  var c = s.color.rgb;
  ${winter ? `// Snow on what faces the sky.
  c = mix(c * 0.8, vec3f(0.94, 0.96, 0.99), smoothstep(0.45, 0.6, n.y));` : ''}
  return vec4f(c / PI * lit, 1.0);
}
`

interface Chunk {
  mesh: Mesh
  /** Its ground or what grows on it changed: to be built again, shown as it is until then. */
  stale: boolean
}

const chunkKey = (cx: number, cz: number) => (cz + 32768) * 65536 + (cx + 32768)
const chunkAt = (key: number): [number, number] => [(key % 65536) - 32768, Math.floor(key / 65536) - 32768]

/**
 * One layer of small things about the camera, in square chunks `size` metres a side, each one merged
 * mesh built by `build` (a `FloorBuilder`'s), shrinking into their roots with distance. Gardens
 * (`gardens.ts`) are drawn by one too. With `shadows` the things cast them.
 */
export class ChunkLayer {
  readonly root = new Group()
  readonly material: ShaderMaterial
  private readonly chunks = new Map<number, Chunk>()

  constructor(
    name: string,
    private readonly size: number,
    private readonly build: (cx: number, cz: number) => Geometry | null,
    private readonly shadows = false,
  ) {
    this.root.name = name
    this.material = new ShaderMaterial({
      wgsl: FLOOR_WGSL,
      vertex: FLOOR_VERTEX_WGSL,
      uniforms: { wind: [Math.cos(WIND_HEADING), Math.sin(WIND_HEADING), 0.1, 0], fade: [0, 1, 1, 0] },
      side: 'double',
      vertexColors: true,
      shadows: true,
    })
  }

  /** Shows every thing within `full` metres, thinning to none between `from` and `far`. */
  reach(full: number, from: number, far: number) {
    this.material.uniforms.fade = [from, far, full, 0]
    this.material.needsUpdate()
  }

  /** Marks the chunks touching the square from `(x0, z0)` to `(x1, z1)` to be built again. */
  staleOver(x0: number, z0: number, x1: number, z1: number) {
    for (const [key, chunk] of this.chunks) {
      const [cx, cz] = chunkAt(key)
      if ((cx + 1) * this.size < x0 || cx * this.size > x1 || (cz + 1) * this.size < z0 || cz * this.size > z1) continue
      chunk.stale = true
    }
  }

  /** Drops the chunks past `far` metres of `(x, z)` and builds the nearest of those missing or stale. */
  update(x: number, z: number, far: number) {
    const { size } = this
    const distance = (cx: number, cz: number) => Math.hypot(Math.max(cx * size - x, 0, x - (cx + 1) * size), Math.max(cz * size - z, 0, z - (cz + 1) * size))
    for (const [key, chunk] of this.chunks) if (distance(...chunkAt(key)) > far + size) this.drop(key, chunk)
    const wanted: [number, number, number][] = []
    for (let cz = Math.floor((z - far) / size); cz <= Math.floor((z + far) / size); cz++) {
      for (let cx = Math.floor((x - far) / size); cx <= Math.floor((x + far) / size); cx++) {
        const d = distance(cx, cz)
        const chunk = this.chunks.get(chunkKey(cx, cz))
        if (d <= far && (!chunk || chunk.stale)) wanted.push([d, cx, cz])
      }
    }
    wanted.sort((a, b) => a[0] - b[0])
    for (const [, cx, cz] of wanted.slice(0, BUILDS_PER_FRAME)) {
      const key = chunkKey(cx, cz)
      const geometry = this.build(cx, cz) ?? new Geometry({ positions: new Float32Array(0) })
      // Nothing reads a chunk again: one that changes is built anew.
      geometry.uploadOnce = true
      const mesh = new Mesh(geometry, this.material)
      // An empty chunk is kept too, so it is not tried again every frame.
      mesh.visible = geometry.vertexCount > 0
      mesh.castShadow = this.shadows
      mesh.receiveShadow = true
      const old = this.chunks.get(key)
      if (old) this.drop(key, old)
      this.root.add(mesh)
      this.chunks.set(key, { mesh, stale: false })
    }
  }

  clear() {
    for (const [key, chunk] of this.chunks) this.drop(key, chunk)
  }

  private drop(key: number, chunk: Chunk) {
    this.root.remove(chunk.mesh)
    chunk.mesh.geometry.dispose()
    this.chunks.delete(key)
  }

  dispose() {
    this.clear()
    this.material.dispose()
  }
}

/** How thick the shrubs and the ferns grow at `(x, z)`, each 0 to 1: drifts some ten metres across, each its own. */
export function driftsAt(x: number, z: number): [number, number] {
  const smooth = (n: number) => {
    const t = Math.min(1, Math.max(0, (n - 0.3) / 0.35))
    return t * t * (3 - 2 * t)
  }
  return [smooth(valueNoise(x / 9 + 41.7, z / 9 - 13.2) * 0.7 + valueNoise(x / 3.1, z / 3.1 + 7.9) * 0.3), smooth(valueNoise(x / 13 - 5.3, z / 13 + 29.1) - 0.12)]
}

/** Something standing that timber keeps clear of: a tree's trunk or a rock, as `[x, z, radius]`. */
type Standing = readonly [number, number, number]

/** The forest's floor about the camera, over every streamed-in tile's woods. */
export class ForestFloor {
  readonly root = new Group()
  private readonly plants: ChunkLayer
  private readonly timber: ChunkLayer
  private density: FloorDensity = FLOOR_FULL
  private plantsOn = true
  /** What stands on each tile, by the timber chunk it stands in. */
  private readonly standing = new Map<number, Map<number, Standing[]>>()
  private readonly craters: { x: number; z: number; radius: number }[] = []

  /**
   * `floorAt` is the `PALETTE` entry of the grass's mask at a point (`Grass.growsAt`: 0 on a road, in
   * water, under a house, in a crater), which says by `SHADE_FLOORS` what floor is there; `heightAt`
   * is the ground, null where it is not loaded.
   */
  constructor(
    private readonly halfExtent: number,
    private readonly tileSize: number,
    private readonly heightAt: (x: number, z: number) => number | null,
    private readonly floorAt: (x: number, z: number) => number,
  ) {
    this.root.name = 'forestfloor'
    this.plants = new ChunkLayer('undergrowth', PLANT_CHUNK, (cx, cz) => this.buildPlants(cx, cz))
    this.timber = new ChunkLayer('deadwood', TIMBER_CHUNK, (cx, cz) => this.buildTimber(cx, cz))
    this.root.add(this.plants.root, this.timber.root)
    this.applyDensity()
  }

  private tileKey(ref: Pick<TileRef, 'col' | 'row'>) {
    return ref.row * 65536 + ref.col
  }

  private applyDensity() {
    const { full, far, timber } = this.density
    this.plants.reach(full, far * 0.8, far)
    this.timber.reach(1e6, timber * 0.75, timber)
  }

  /** The floor of a streamed-in tile: what is built over it already is built again. Call after the grass has the tile. */
  addTile(ref: Pick<TileRef, 'col' | 'row'>, tile: Pick<TileData, 'trees'>) {
    const key = this.tileKey(ref)
    this.dropStanding(key)
    for (const tree of tile.trees) {
      const rock = isRock(tree.kind)
      if (!rock && !tree.solid) continue
      const at = chunkKey(Math.floor(tree.x / TIMBER_CHUNK), Math.floor(tree.z / TIMBER_CHUNK))
      const byTile = this.standing.get(at) ?? new Map<number, Standing[]>()
      this.standing.set(at, byTile)
      const list = byTile.get(key) ?? []
      list.push([tree.x, tree.z, rock ? tree.crown : tree.trunkRadius])
      byTile.set(key, list)
    }
    this.staleOver(ref)
  }

  removeTile(ref: Pick<TileRef, 'col' | 'row'>) {
    this.dropStanding(this.tileKey(ref))
    this.staleOver(ref)
  }

  /** The ground of a tile moved: what stands on it is stood again. */
  regrowTile(ref: Pick<TileRef, 'col' | 'row'>) {
    this.staleOver(ref)
  }

  private dropStanding(tileKey: number) {
    for (const [key, byTile] of this.standing) if (byTile.delete(tileKey) && byTile.size === 0) this.standing.delete(key)
  }

  private staleOver(ref: Pick<TileRef, 'col' | 'row'>) {
    const x0 = -this.halfExtent + ref.col * this.tileSize, z0 = -this.halfExtent + ref.row * this.tileSize
    this.plants.staleOver(x0, z0, x0 + this.tileSize, z0 + this.tileSize)
    this.timber.staleOver(x0, z0, x0 + this.tileSize, z0 + this.tileSize)
  }

  /** A shell dug a crater of `radius` metres at `(x, z)`: what stood in it is gone, and the rest stands on the new ground. */
  dent(x: number, z: number, radius: number) {
    if (this.craters.some((c) => c.x === x && c.z === z && c.radius === radius)) return
    this.craters.push({ x, z, radius })
    this.plants.staleOver(x - radius, z - radius, x + radius, z + radius)
    this.timber.staleOver(x - radius, z - radius, x + radius, z + radius)
  }

  /** The ground of a crater `dent` was told of has healed: what stood in it grows again. */
  heal(x: number, z: number, radius: number) {
    const at = this.craters.findIndex((c) => c.x === x && c.z === z && c.radius === radius)
    if (at < 0) return
    this.craters.splice(at, 1)
    this.plants.staleOver(x - radius, z - radius, x + radius, z + radius)
    this.timber.staleOver(x - radius, z - radius, x + radius, z + radius)
  }

  /** Which way the wind blows, a unit `[x, z]`. */
  blowing(direction: readonly [number, number]) {
    const wind = this.plants.material.uniforms.wind as number[]
    wind[0] = direction[0]
    wind[1] = direction[1]
    this.plants.material.needsUpdate()
  }

  /** Whether the plants are grown: not under a winter's snow, nor with the grass turned off. The timber always is. */
  setPlants(on: boolean) {
    this.plantsOn = on
    this.plants.root.visible = on
    if (!on) this.plants.clear()
  }

  setDensity(density: FloorDensity) {
    if (density === this.density) return
    this.density = density
    this.plants.clear()
    this.applyDensity()
  }

  /** Builds and drops chunks about the camera at `(x, z)`. */
  update(x: number, z: number) {
    if (this.plantsOn) this.plants.update(x, z, this.density.far)
    this.timber.update(x, z, this.density.timber)
  }

  /** The floor (`FLOOR_*`) at `(x, z)`, or undefined where no forest's floor is. */
  private floor(x: number, z: number): number | undefined {
    return SHADE_FLOORS[this.floorAt(x, z)]
  }

  private buildPlants(cx: number, cz: number): Geometry | null {
    const next = random(Math.imul(cx, 0x9e3779b1) ^ Math.imul(cz, 0x85ebca77) ^ 0x51ed270b)
    const total = Math.round(PLANTS_PER_METRE * PLANT_CHUNK * PLANT_CHUNK)
    const count = Math.ceil(total * this.density.plants)
    const out = new FloorBuilder()
    for (let i = 0; i < count; i++) {
      // Every draw is taken whether the plant grows or not, so plant i is the same at any density.
      const x = (cx + next()) * PLANT_CHUNK, z = (cz + next()) * PLANT_CHUNK
      const pick = next()
      const seed = next()
      const floor = this.floor(x, z)
      if (floor === undefined) continue
      const { grows, fern } = PLANTS[floor]!
      const [shrubs, ferns] = driftsAt(x, z)
      let left = pick
      let grown: Shrub | Mound | 'fern' | null = null
      if ((left -= fern * ferns) < 0) grown = 'fern'
      else for (const [plant, chance] of grows) if (!grown && (left -= chance * (0.15 + 0.85 * shrubs)) < 0) grown = plant
      if (!grown) continue
      const y = this.heightAt(x, z)
      if (y === null) continue
      const normal = normalAt((nx, nz) => this.heightAt(nx, nz) ?? y, x, z, 1)
      // A plant's own draws, apart from the chunk's, so one that does not grow shifts none of the others.
      const own = random(Math.floor(seed * 4294967296))
      // The plants of a thinner field keep their places in the order of thinning.
      const rank = i / count
      if (grown === 'fern') addFern(out, x, y, z, normal, rank, own)
      else if ('sprigs' in grown) addShrub(out, grown, x, y, z, normal, rank, own)
      else addMound(out, grown, x, y, z, normal, rank, own)
    }
    return out.vertices > 0 ? out.geometry() : null
  }

  /** Whether the ground within `radius` of `(x, z)` is free of trunks, rocks and craters. */
  private clear(x: number, z: number, radius: number): boolean {
    if (this.craters.some((c) => Math.hypot(x - c.x, z - c.z) < c.radius + radius)) return false
    const [cx, cz] = [Math.floor(x / TIMBER_CHUNK), Math.floor(z / TIMBER_CHUNK)]
    for (let dz = -1; dz <= 1; dz++) {
      for (let dx = -1; dx <= 1; dx++) {
        for (const list of this.standing.get(chunkKey(cx + dx, cz + dz))?.values() ?? []) {
          for (const [sx, sz, r] of list) if (Math.hypot(x - sx, z - sz) < r + radius) return false
        }
      }
    }
    return true
  }

  private buildTimber(cx: number, cz: number): Geometry | null {
    const next = random(Math.imul(cx, 0x9e3779b1) ^ Math.imul(cz, 0x85ebca77) ^ 0x3c6ef372)
    const out = new FloorBuilder()
    for (let i = 0; i < TIMBER_PER_CHUNK; i++) {
      const x = (cx + next()) * TIMBER_CHUNK, z = (cz + next()) * TIMBER_CHUNK
      const pick = next()
      const own = random(Math.floor(next() * 4294967296))
      const floor = this.floor(x, z)
      if (floor === undefined) continue
      const y = this.heightAt(x, z)
      if (y === null) continue
      const [stump, stone] = TIMBER[floor]!
      const wood = DEADWOOD[floor]!
      if (pick < stump) {
        if (this.clear(x, z, 0.5)) addStump(out, wood, x, y, z, own)
      } else if (pick < stump + stone) {
        const radius = 0.18 + own() * 0.27
        if (this.clear(x, z, radius)) addStone(out, wood.moss, x, y, z, radius, radius * (0.5 + 0.4 * own()), own)
      }
    }
    return out.vertices > 0 ? out.geometry() : null
  }

  dispose() {
    this.plants.dispose()
    this.timber.dispose()
    this.root.removeFromParent()
  }
}
