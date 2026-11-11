import { Color, Geometry, InstancedMesh, ShaderMaterial, type Material, icosphereGeometry, mat4 } from '@ragnarok/engine'
import { TREE_BUSH, TREE_ROCK_BOULDER, TREE_ROCK_SLAB, type Tree } from '@/world'
import { winter } from './season'
import { shoreLevelAt, WET_HEIGHT, type Shore } from './shore'

/*
 * Boulders and slabs. A few shapes a kind, carved once from a sphere, and every rock an instance of one
 * of them: tipped, sunk and stretched by its position, tinted from a handful of stones, and mossed by how
 * much forest stands about it. The shader adds what an instance cannot carry, in world space so no two
 * copies of a shape look alike: mottling, lichen, and moss and snow on what faces the sky.
 */

/** How one shape is carved: flat cuts through a unit sphere, then squashed and roughened. */
export interface Carving {
  /** Planes cut through the sphere, each at a depth (0 the centre, 1 the surface) in `depth`. */
  cuts: number
  depth: [number, number]
  /** A cut straight across the top at this height (0..1), for slabs, or none. */
  top?: number
  /** Height over width, before the rock is scaled to its own. */
  squash: number
  /** Bumps over the faces, as a share of the radius. */
  bump: number
}

const CARVINGS: Record<number, Carving[]> = {
  // Rounded erratics, ground smooth by the ice, with a face or two split off.
  [TREE_ROCK_BOULDER]: [
    { cuts: 3, depth: [0.7, 0.85], squash: 0.8, bump: 0.06 },
    { cuts: 5, depth: [0.6, 0.8], squash: 0.75, bump: 0.05 },
    { cuts: 2, depth: [0.55, 0.7], squash: 0.9, bump: 0.07 },
  ],
  // Slabs and blocks: flat on top, broken square at the sides.
  [TREE_ROCK_SLAB]: [
    { cuts: 6, depth: [0.45, 0.75], top: 0.45, squash: 0.6, bump: 0.03 },
    { cuts: 8, depth: [0.5, 0.7], top: 0.6, squash: 0.7, bump: 0.03 },
  ],
}

/** Rocks with no carving of their own (`TREE_KINDS` grows): drawn as boulders. */
export function isRock(kind: number): boolean {
  return kind === TREE_ROCK_BOULDER || kind === TREE_ROCK_SLAB
}

/** A seeded generator, 0..1. */
function random(seed: number) {
  let s = seed >>> 0
  return () => {
    s = (s + 0x6d2b79f5) >>> 0
    let t = Math.imul(s ^ (s >>> 15), 1 | s)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

/**
 * One shape, in the unit space trees use: foot near the origin, `y` up to 1 at the top, radius 1, and
 * the lowest quarter below the ground so it sits in the earth rather than on it. Flat-faced, and darker
 * toward its foot (the vertex colour, which the instance's tint multiplies).
 */
export function carveRock(carving: Carving, seed: number, detail = 2): Geometry {
  const next = random(seed)
  const sphere = icosphereGeometry(1, detail)
  const p = sphere.positions
  const planes: number[][] = []
  for (let i = 0; i < carving.cuts; i++) {
    // Mostly about the sides: a cut straight down from the top would make a slab of every rock.
    const angle = next() * Math.PI * 2
    const lift = (next() - 0.35) * 1.1
    const length = Math.hypot(1, lift)
    planes.push([Math.cos(angle) / length, lift / length, Math.sin(angle) / length, carving.depth[0] + next() * (carving.depth[1] - carving.depth[0])])
  }
  if (carving.top !== undefined) planes.push([0, 1, 0, carving.top])
  // Bumps from a few random waves over the sphere, smooth so faces stay faces.
  const waves = Array.from({ length: 4 }, () => [next() * 2 - 1, next() * 2 - 1, next() * 2 - 1, 2 + next() * 3, next() * 6])
  for (let i = 0; i < p.length; i += 3) {
    let x = p[i]!, y = p[i + 1]!, z = p[i + 2]!
    let bump = 0
    for (const [wx, wy, wz, f, o] of waves) bump += Math.sin((x * wx! + y * wy! + z * wz!) * f! + o!)
    const r = 1 + (bump / waves.length) * carving.bump
    x *= r
    y *= r
    z *= r
    for (const [nx, ny, nz, d] of planes) {
      const over = x * nx! + y * ny! + z * nz! - d!
      if (over > 0) {
        x -= nx! * over
        y -= ny! * over
        z -= nz! * over
      }
    }
    p[i] = x
    p[i + 1] = y * carving.squash
    p[i + 2] = z
  }
  // Fitted to the unit space: widest radius 1, top at 1, a quarter of its height under the ground.
  let low = Infinity, high = -Infinity, wide = 0
  for (let i = 0; i < p.length; i += 3) {
    low = Math.min(low, p[i + 1]!)
    high = Math.max(high, p[i + 1]!)
    wide = Math.max(wide, Math.hypot(p[i]!, p[i + 2]!))
  }
  for (let i = 0; i < p.length; i += 3) {
    p[i] = p[i]! / wide
    p[i + 1] = ((p[i + 1]! - low) / (high - low)) * 1.25 - 0.25
    p[i + 2] = p[i + 2]! / wide
  }
  const geometry = sphere.toNonIndexed()
  geometry.computeNormals()
  const q = geometry.positions
  const colors = new Float32Array(geometry.vertexCount * 3)
  for (let i = 0; i < geometry.vertexCount; i++) {
    // Shaded toward the foot, where the ground and the grass keep the light off it.
    const t = Math.min(1, Math.max(0, (q[i * 3 + 1]! + 0.1) / 0.6))
    colors.fill(0.6 + 0.4 * t * t * (3 - 2 * t), i * 3, i * 3 + 3)
  }
  geometry.colors = colors
  return geometry
}

/** The stones a rock is of: grey granite most, then pink granite, dark gneiss, a pale one. */
const STONES = [
  { color: Color.hex(0x8e8d89), weight: 0.5 },
  { color: Color.hex(0x9c8b80), weight: 0.25 },
  { color: Color.hex(0x76777a), weight: 0.15 },
  { color: Color.hex(0xa8a79f), weight: 0.1 },
]

/**
 * A rock instance's colour alpha: its moss (0 to 1) and the level of the water by it (NaN for none),
 * packed in one float. The whole part is the level in steps of 5 cm over -1000 m, 0 for none; the rest
 * is the moss, a quarter up so the interpolated value cannot slip into the next whole number.
 */
export function packRockAlpha(moss: number, level: number): number {
  return 0.25 + moss + 2 * (Number.isNaN(level) ? 0 : Math.max(1, Math.round((level + 1000) * 20)))
}

const UNPACK_WGSL = /* wgsl */ `
  // The alpha packs the moss and the water's level by the rock (\`packRockAlpha\`).
  let packed = floor(s.color.a * 0.5);
  let moss = clamp(s.color.a - 2.0 * packed - 0.25, 0.0, 1.0);
  let water = select(-1.0e9, packed / 20.0 - 1000.0, packed > 0.5);`

/**
 * The shader: the cel look of the trees (four bands of sun, the sky's fill), but in the sun's shadow,
 * and with the stone's own detail in world space. The instance's colour is the stone's tint in rgb and
 * its moss and the water by it in alpha (\`packRockAlpha\`), or with `bare` neither, the alpha left to
 * the vertex code (`render/stones.ts`), which darkens its wet stones itself.
 */
export function rockWgsl(bare = false): string {
  return ROCK_WGSL.replace('ROCK_ALPHA', bare ? 'let moss = 0.0;\n  let water = -1.0e9;' : UNPACK_WGSL)
}

const ROCK_WGSL = /* wgsl */ `
const BANDS_AT = array<f32, 4>(0.376, 0.588, 0.804, 1.0);
const LICHEN = vec3f(0.78, 0.8, 0.7);
const LICHEN_RUST = vec3f(0.72, 0.5, 0.26);
const MOSS_DARK = vec3f(0.17, 0.27, 0.09);
const MOSS_LIGHT = vec3f(0.36, 0.47, 0.16);

// An integer hash: a sine hash loses its bits this far from the origin.
fn rockHash(c: vec3i) -> f32 {
  var h = (bitcast<u32>(c.x) * 73856093u) ^ (bitcast<u32>(c.y) * 19349663u) ^ (bitcast<u32>(c.z) * 83492791u);
  h = (h ^ (h >> 16u)) * 0x7feb352du;
  h = (h ^ (h >> 15u)) * 0x846ca68bu;
  h = h ^ (h >> 16u);
  return f32(h) / 4294967295.0;
}

fn rockNoise(p: vec3f) -> f32 {
  let i = vec3i(floor(p));
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  let a = mix(mix(rockHash(i), rockHash(i + vec3i(1, 0, 0)), u.x), mix(rockHash(i + vec3i(0, 1, 0)), rockHash(i + vec3i(1, 1, 0)), u.x), u.y);
  let b = mix(mix(rockHash(i + vec3i(0, 0, 1)), rockHash(i + vec3i(1, 0, 1)), u.x), mix(rockHash(i + vec3i(0, 1, 1)), rockHash(i + vec3i(1, 1, 1)), u.x), u.y);
  return mix(a, b, u.z);
}

fn surface(s: Surface) -> vec4f {
  let p = s.world;
  let n = s.normal;
  ROCK_ALPHA
  // Wet below the waterline and a little over it, where the waves wash: dark, and nothing grows on it.
  let line = water + ${WET_HEIGHT.toFixed(2)} + (rockNoise(vec3f(p.x, 0.0, p.z) * 1.7) - 0.5) * 0.12;
  let wet = 1.0 - smoothstep(line - 0.03, line + 0.03, p.y);
  // Mottled: a broad blotch and a fine grain, so the faces are not flat colour.
  let mottle = rockNoise(p * 1.3) * 0.6 + rockNoise(p * 6.1) * 0.4;
  var c = s.color.rgb * (0.84 + 0.32 * mottle);
  // Lichen in crusts, pale and now and then rusty, on what is not turned to the ground.
  let crust = rockNoise(p * 9.0 + vec3f(13.0, 7.0, 3.0)) * 0.65 + rockNoise(p * 23.0) * 0.35;
  let lichen = smoothstep(0.7, 0.73, crust) * smoothstep(-0.4, 0.2, n.y) * (1.0 - wet);
  let crustColor = mix(LICHEN, LICHEN_RUST, step(0.8, rockNoise(p * 2.1 + vec3f(5.0))));
  c = mix(c, mix(c, crustColor, 0.6) * (0.9 + 0.2 * mottle), lichen * 0.6);
  // Moss over the top, further down the sides the more there is, its edge broken by noise.
  let up = n.y + (rockNoise(p * 2.3 + vec3f(31.0)) - 0.5) * 0.8;
  let cover = smoothstep(0.0, 0.08, up - (1.05 - moss * 1.35)) * step(0.01, moss) * (1.0 - wet);
  c = mix(c, mix(MOSS_DARK, MOSS_LIGHT, rockNoise(p * 3.7)), cover);
  c *= mix(1.0, 0.5, wet);
  ${
    winter
      ? `
  // Snow on what faces the sky, in clumps, over the moss.
  let q = p * vec3f(2.3, 3.1, 2.3);
  let clumps = sin(q.x + sin(q.y * 1.3)) * sin(q.z + sin(q.y * 0.9)) * 0.5 + sin(q.y * 2.1 + q.x * 1.7) * 0.2;
  c = mix(c * 0.8, vec3f(0.94, 0.96, 0.99), smoothstep(0.25, 0.4, n.y + clumps * 0.35));`
      : ''
  }
  let band = u32(clamp(dot(n, frame.sunDir.xyz) * 0.5 + 0.5, 0.0, 0.999) * 4.0);
  let hemi = mix(frame.hemiGround.rgb, frame.hemiSky.rgb, 0.5 * n.y + 0.5);
  var bands = BANDS_AT;
  let shadow = sunShadow(p, n);
  let lit = frame.sunColor.rgb * bands[band] * shadow + hemi + pointLight(p, n);
  // Wet stone shines: the sky at a grazing look, and the sun's glint.
  var shine = vec3f(0.0);
  if (wet > 0.0) {
    let view = normalize(frame.cameraPos.xyz - p);
    let fresnel = 0.04 + 0.96 * pow(1.0 - clamp(dot(n, view), 0.0, 1.0), 5.0);
    let glint = pow(max(dot(reflect(-view, n), frame.sunDir.xyz), 0.0), 40.0);
    shine = wet * (frame.fogColor.rgb * fresnel * 0.3 + frame.sunColor.rgb * glint * 0.4 * shadow);
  }
  return vec4f(c / PI * lit + shine, 1.0);
}
`

let shared: { shapes: Map<number, Geometry[]>; material: Material } | undefined

function prototypes() {
  if (!shared) {
    const shapes = new Map<number, Geometry[]>()
    for (const [kind, carvings] of Object.entries(CARVINGS)) shapes.set(Number(kind), carvings.map((carving, i) => carveRock(carving, Number(kind) * 977 + i * 131 + 7)))
    shared = { shapes, material: new ShaderMaterial({ wgsl: rockWgsl(), vertexColors: true, shadows: true }) }
  }
  return shared
}

/** A number 0..1 from a rock's position and `salt`, the same every time it is built. */
function hashAt(x: number, z: number, salt: number): number {
  let h = Math.imul(Math.round(x * 100) | 0, 0x27d4eb2d) ^ Math.imul(Math.round(z * 100) | 0, 0x165667b1) ^ Math.imul(salt, 0x9e3779b1)
  h = Math.imul(h ^ (h >>> 15), 0x85ebca6b)
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35)
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296
}

/** Metres a side of the cells trees are counted in, for how much forest stands about a rock. */
const FOREST_CELL = 8

/**
 * How mossy each rock is, 0..1: by the trees within a cell or so of it, so a rock in the spruce is green
 * over the top and one on the shore or in a field is bare stone.
 */
function mossOf(trees: readonly Tree[], rocks: readonly Tree[]): number[] {
  const counts = new Map<number, number>()
  const key = (x: number, z: number) => (Math.floor(z / FOREST_CELL) + 32768) * 65536 + Math.floor(x / FOREST_CELL) + 32768
  for (const tree of trees) {
    if (isRock(tree.kind) || tree.kind === TREE_BUSH) continue
    const k = key(tree.x, tree.z)
    counts.set(k, (counts.get(k) ?? 0) + 1)
  }
  return rocks.map((rock) => {
    let around = 0
    for (let dz = -1; dz <= 1; dz++) for (let dx = -1; dx <= 1; dx++) around += counts.get(key(rock.x + dx * FOREST_CELL, rock.z + dz * FOREST_CELL)) ?? 0
    const forest = Math.min(1, around / 8)
    // Slabs hold more on their flat tops; every rock a little more or less than its neighbours.
    const moss = forest * (rock.kind === TREE_ROCK_SLAB ? 0.95 : 0.8) + (hashAt(rock.x, rock.z, 5) - 0.5) * 0.3
    return Math.min(1, Math.max(0, moss))
  })
}

/**
 * One tile's rocks (those of `trees` that are rocks), one instanced mesh a shape. `trees` is the whole
 * tile's list, for the forest about each rock.
 */
export function buildRocks(trees: readonly Tree[], rocks: readonly { tree: Tree; index: number }[], shore: Shore | null = null): InstancedMesh[] {
  if (rocks.length === 0) return []
  const { shapes, material } = prototypes()
  const moss = mossOf(
    trees,
    rocks.map(({ tree }) => tree),
  )
  const byShape = new Map<Geometry, number[]>()
  rocks.forEach(({ tree }, i) => {
    const list = shapes.get(tree.kind) ?? shapes.get(TREE_ROCK_BOULDER)!
    const shape = list[Math.floor(hashAt(tree.x, tree.z, 1) * list.length)]!
    const at = byShape.get(shape) ?? []
    at.push(i)
    byShape.set(shape, at)
  })
  const matrix = mat4.create()
  const meshes: InstancedMesh[] = []
  for (const [shape, list] of byShape) {
    const mesh = new InstancedMesh(shape, material, list.length)
    const slots = new Map<number, number>()
    mesh.userData.treeSlots = slots
    list.forEach((r, i) => {
      const { tree, index } = rocks[r]!
      slots.set(index, i)
      const h = (salt: number) => hashAt(tree.x, tree.z, salt)
      // Turned about the vertical, then tipped over a level axis: slabs lie flatter than boulders.
      const yaw = h(2) * Math.PI * 2
      const tilt = (h(3) - 0.5) * (tree.kind === TREE_ROCK_SLAB ? 0.3 : 0.5)
      const axis = h(4) * Math.PI * 2
      const [ys, yc] = [Math.sin(yaw / 2), Math.cos(yaw / 2)]
      const [ts, tc] = [Math.sin(tilt / 2), Math.cos(tilt / 2)]
      const [ax, az] = [Math.cos(axis) * ts, Math.sin(axis) * ts]
      // tilt × yaw, both as quaternions: (ax, 0, az, tc) × (0, ys, 0, yc).
      const qx = ax * yc - az * ys, qy = tc * ys, qz = az * yc + ax * ys, qw = tc * yc
      // Stretched a little one way, and sunk a little deeper or shallower.
      const sx = tree.crown * (0.85 + h(6) * 0.3), sz = tree.crown * (0.85 + h(7) * 0.3)
      const sink = (h(8) - 0.3) * 0.15 * tree.height
      mesh.setMatrixAt(i, mat4.compose(matrix, tree.x, tree.base - sink, tree.z, qx, qy, qz, qw, sx, tree.height, sz))
      let pick = h(9)
      const stone = STONES.find((s) => (pick -= s.weight) < 0) ?? STONES[0]!
      const shade = 0.92 + h(10) * 0.16
      const water = shore ? shoreLevelAt(shore, tree.x, tree.z) : NaN
      mesh.setColorAt(i, stone.color.r * shade, stone.color.g * shade, stone.color.b * shade, packRockAlpha(moss[r]!, water))
    })
    mesh.needsUpdate()
    // No coarser shapes, but each rock is culled on its own.
    mesh.levels = []
    mesh.castShadow = true
    mesh.receiveShadow = true
    mesh.userData.sharedGeometry = true
    meshes.push(mesh)
  }
  return meshes
}
