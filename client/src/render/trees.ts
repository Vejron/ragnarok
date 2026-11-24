import { Color, Geometry, Group, InstancedMesh, Mesh, ShaderMaterial, Texture, type DetailLevel, type Node, cylinderGeometry, icosphereGeometry, mat4 } from '@ragnarok/engine'
import {
  TREE_BIRCH,
  TREE_BROADLEAF,
  TREE_BUSH,
  TREE_COLUMNAR,
  TREE_FLAG,
  TREE_LOWWIDE,
  TREE_ORNAMENTAL,
  TREE_PINE,
  TREE_SPRUCE,
  type Tree,
} from '@/world'
import { IMPOSTOR_ATLAS_PX, IMPOSTOR_FRAMES, IMPOSTOR_FRAME_PX, IMPOSTOR_MARGIN, IMPOSTOR_SPREAD, IMPOSTOR_TILES, bakeImpostor, emptyAtlas, type ImpostorFrame } from './impostors'
import { buildRocks, isRock } from './rocks'
import type { Shore } from './shore'
import { winter } from './season'
import { WIND_HEADING } from '@/sim/wind'

/**
 * The tree archetypes. Each prototype is built in unit space (foot at the origin, `y` 0..1 over the
 * tree's height, crown radius 1) and placed per tree by scaling `(crown, height, crown)`, so the
 * trunk radii here are the real ones divided by the archetype's crown radius.
 */
const TRUNK_BARK = 0x8a5a3c
const TRUNK_BIRCH = 0xe9e5d9

export interface Part {
  /** `ellipsoid`: a low-poly sphere scaled by `(rx, ry, rz)`; `cone` and `trunk` span `y0..y1`. */
  shape: 'ellipsoid' | 'cone' | 'trunk'
  color: number
  y0: number
  y1?: number
  /** Ellipsoid radii in unit space; radius of the cone's base or the trunk. */
  rx?: number
  ry?: number
  x?: number
  z?: number
}

const PROTOTYPES: Record<number, Part[]> = {
  [TREE_BIRCH]: [
    { shape: 'trunk', color: TRUNK_BIRCH, y0: 0, y1: 0.7, rx: 0.07 },
    { shape: 'ellipsoid', color: 0x9bd36b, y0: 0.64, rx: 1, ry: 0.38 },
  ],
  [TREE_PINE]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.85, rx: 0.08 },
    { shape: 'ellipsoid', color: 0x4f8f4a, y0: 0.74, rx: 0.9, ry: 0.12 },
    { shape: 'ellipsoid', color: 0x5a9a50, y0: 0.9, rx: 0.6, ry: 0.1, x: 0.15 },
  ],
  // Three tiers, each tall enough to reach well into the next, so the outline is a soft stack rather
  // than deep notches: the impostor's pictures round notches off, and the solid should look as they do.
  [TREE_SPRUCE]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.2, rx: 0.09 },
    { shape: 'cone', color: 0x2f6b4a, y0: 0.12, y1: 0.7, rx: 1 },
    { shape: 'cone', color: 0x2f6b4a, y0: 0.42, y1: 0.86, rx: 0.76 },
    { shape: 'cone', color: 0x35764f, y0: 0.66, y1: 1, rx: 0.52 },
  ],
  [TREE_ORNAMENTAL]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.55, rx: 0.05 },
    { shape: 'ellipsoid', color: 0xa7d65c, y0: 0.68, rx: 1, ry: 0.32 },
  ],
  [TREE_BROADLEAF]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.55, rx: 0.06 },
    { shape: 'ellipsoid', color: 0x4fae4a, y0: 0.68, rx: 1, ry: 0.32 },
    { shape: 'ellipsoid', color: 0x5cbc54, y0: 0.82, rx: 0.65, ry: 0.2, x: 0.2, z: 0.1 },
  ],
  [TREE_COLUMNAR]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.3, rx: 0.13 },
    { shape: 'ellipsoid', color: 0x6fbf5a, y0: 0.6, rx: 1, ry: 0.4 },
  ],
  [TREE_LOWWIDE]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.35, rx: 0.045 },
    { shape: 'ellipsoid', color: 0x9bbf7a, y0: 0.58, rx: 1, ry: 0.45 },
  ],
  // One wide, low mound on the ground; near, its leafy crown gives it the lumps of a shrub.
  [TREE_BUSH]: [
    { shape: 'ellipsoid', color: 0x3f8f45, y0: 0.45, rx: 1, ry: 0.5 },
  ],
  // Over a crown of 0.3 m: a thin white pole, a red pennant at its top, and the dark cup it stands in.
  [TREE_FLAG]: [
    { shape: 'trunk', color: 0xf4f4f0, y0: 0, y1: 1, rx: 0.04 },
    { shape: 'ellipsoid', color: 0xe0302a, y0: 0.9, rx: 0.55, ry: 0.06, x: 0.55 },
    { shape: 'trunk', color: 0x1a1a18, y0: 0, y1: 0.004, rx: 0.4 },
  ],
}

/**
 * Metres from the camera at which a tree changes how it is drawn: first from its solid to its impostor
 * (a picture of the solid from about where the camera is, `impostors.ts`), then to the same impostor
 * casting a cruder shadow. The view changes only at the first, and fades into it over `TREE_FADE`.
 * A forest is mostly far trees: drawn whole they were nine tenths of the frame's triangles. The
 * quality's `treeDetail` scales these.
 */
export const TREE_LEVELS_FROM = [80, 160]
/** Metres before the impostor takes over in which the solid dissolves into it, pixel by pixel. */
export const TREE_FADE = 20
/** A tree's levels: the solid, and the impostor twice over. */
export const TREE_LEVELS = TREE_LEVELS_FROM.length + 1
/** The finest level a tree's shadow is cast from: trees take no shadow, so the two are never compared. */
const TREE_SHADOW_LEVEL = 1

/** How finely a tree is built: as seen near, for a middling tree's shadow, or for a far tree's. */
type Detail = 'full' | 'coarse' | 'shadow'

/** Sides of a cone and of a trunk at each detail. Only the full shape has its ends closed. */
const CONE_SIDES: Record<Detail, number> = { full: 12, coarse: 5, shadow: 6 }
const TRUNK_SIDES: Record<Detail, number> = { full: 6, coarse: 4, shadow: 3 }
/**
 * How much wider a coarse crown is made, so its outline covers about what the round one did: an
 * icosahedron's faces and a bipyramid's lie well inside the sphere their corners are on.
 */
const CROWN_SWELL: Record<Detail, number> = { full: 1, coarse: 1.08, shadow: 1.12 }

/** Eight triangles on the unit sphere, wound as the icosphere is. */
function octahedronGeometry(): Geometry {
  const positions = new Float32Array([1, 0, 0, -1, 0, 0, 0, 1, 0, 0, -1, 0, 0, 0, 1, 0, 0, -1])
  const indices: number[] = []
  for (const sx of [0, 1]) for (const sy of [2, 3]) for (const sz of [4, 5]) {
    const outward = (sx === 0) === ((sy === 2) === (sz === 4))
    indices.push(sx, outward ? sy : sz, outward ? sz : sy)
  }
  return new Geometry({ positions, indices: new Uint16Array(indices) })
}

/**
 * Twelve triangles on the unit sphere: a hexagon round the middle with a point above and below, wound
 * as the cone is. Seen from the sun it is a hexagon, round enough for a far tree's shadow, where the
 * octahedron it replaced cast a diamond.
 */
function bipyramidGeometry(): Geometry {
  const sides = 6
  const positions: number[] = []
  for (let i = 0; i < sides; i++) {
    const t = (i / sides) * Math.PI * 2
    positions.push(Math.sin(t), 0, Math.cos(t))
  }
  positions.push(0, 1, 0, 0, -1, 0)
  const indices: number[] = []
  for (let i = 0; i < sides; i++) {
    const next = (i + 1) % sides
    indices.push(sides, i, next, sides + 1, next, i)
  }
  return new Geometry({ positions: new Float32Array(positions), indices: new Uint16Array(indices) })
}

/** The side of a cylinder along Y, centred on the origin, without its ends; a cone when `top` is 0. */
function tubeGeometry(top: number, bottom: number, height: number, sides: number): Geometry {
  const positions: number[] = []
  const indices: number[] = []
  const half = height / 2
  for (let i = 0; i < sides; i++) {
    const t = (i / sides) * Math.PI * 2
    positions.push(bottom * Math.sin(t), -half, bottom * Math.cos(t))
  }
  if (top <= 0) {
    positions.push(0, half, 0)
    for (let i = 0; i < sides; i++) indices.push(sides, i, (i + 1) % sides)
  } else {
    for (let i = 0; i < sides; i++) {
      const t = (i / sides) * Math.PI * 2
      positions.push(top * Math.sin(t), half, top * Math.cos(t))
    }
    for (let i = 0; i < sides; i++) {
      const next = (i + 1) % sides
      indices.push(sides + i, i, next, sides + i, next, sides + next)
    }
  }
  return new Geometry({ positions: new Float32Array(positions), indices: new Uint16Array(indices) })
}

/**
 * The parts a tree is made of at `detail`. For a far tree's shadow its crowns are one crown round them
 * all, and its cones one cone, in the colour of the widest.
 */
function partsAt(parts: readonly Part[], detail: Detail): Part[] {
  if (detail !== 'shadow') return [...parts]
  const out = parts.filter((part) => part.shape === 'trunk')
  const crowns = parts.filter((part) => part.shape === 'ellipsoid')
  if (crowns.length) {
    const widest = crowns.reduce((a, b) => (b.rx! > a.rx! ? b : a))
    const low = Math.min(...crowns.map((part) => part.y0 - part.ry!))
    const high = Math.max(...crowns.map((part) => part.y0 + part.ry!))
    out.push({ ...widest, y0: (low + high) / 2, ry: (high - low) / 2 })
  }
  const cones = parts.filter((part) => part.shape === 'cone')
  if (cones.length) {
    const widest = cones.reduce((a, b) => (b.rx! > a.rx! ? b : a))
    out.push({ ...widest, y0: Math.min(...cones.map((part) => part.y0)), y1: Math.max(...cones.map((part) => part.y1!)) })
  }
  return out
}

function partGeometry(part: Part, detail: Detail): Geometry {
  let geometry: Geometry
  if (part.shape === 'ellipsoid') {
    // A low-poly sphere: 42 welded vertices and 80 triangles, so the crown reads as a few bold shapes.
    // Further off 20 triangles, and in a far tree's shadow 12.
    const swell = CROWN_SWELL[detail]
    const ball = detail === 'shadow' ? bipyramidGeometry() : icosphereGeometry(1, detail === 'full' ? 1 : 0)
    geometry = ball.scale(part.rx! * swell, part.ry! * swell, part.rx! * swell).translate(part.x ?? 0, part.y0, part.z ?? 0)
  } else {
    const height = part.y1! - part.y0
    const cone = part.shape === 'cone'
    // A cone is a cylinder that closes to a point at the top.
    const sides = cone ? CONE_SIDES[detail] : TRUNK_SIDES[detail]
    const top = cone ? 0 : part.rx! * 0.8
    geometry = detail === 'full' ? cylinderGeometry(top, part.rx!, height, sides) : tubeGeometry(top, part.rx!, height, sides)
    geometry.translate(0, part.y0 + height / 2, 0)
  }
  // Indexed and welded, so the parts merge and each vertex is shared: position + normal + colour.
  geometry.uvs = undefined
  geometry.computeNormals()
  const color = Color.hex(part.color)
  const count = geometry.vertexCount
  const colors = new Float32Array(count * 3)
  for (let i = 0; i < count; i++) colors.set([color.r, color.g, color.b], i * 3)
  geometry.colors = colors
  return geometry
}

/**
 * The kinds whose near solid has a leafy crown (`leafyCrown`), each with its usual height over its
 * crown radius (`trees::size` in osm-import). A tree is placed by scaling its unit space by `(crown,
 * height, crown)`, so the crown is built at that ratio for its clumps to be round in metres.
 */
const LEAFY: Partial<Record<number, { aspect: number; every?: boolean; style?: Partial<CrownStyle> }>> = {
  [TREE_BROADLEAF]: { aspect: 2.8 },
  [TREE_ORNAMENTAL]: { aspect: 3 },
  [TREE_LOWWIDE]: { aspect: 2.3 },
  // A birch's crown is airy: smaller clumps, gaps to its white branches, strands of twigs hanging down.
  [TREE_BIRCH]: { aspect: 5.5, style: { clump: 0.32, most: 16, tufts: 6, core: 0.65, limbs: 9, hang: 4 } },
  [TREE_COLUMNAR]: { aspect: 10.7 },
  // A pine's two flat crowns both turn leafy, a layered umbrella of clumps on its tall trunk.
  // Pines are many in a forest, so each of the two takes fewer clumps.
  [TREE_PINE]: { aspect: 6.4, every: true, style: { most: 7 } },
  // A bush is about as tall as it is wide: a dense mound of big clumps with no trunk or branches.
  // Bushes are many under the trees, so each takes about as few clumps as a pine's crown.
  [TREE_BUSH]: { aspect: 1, style: { clump: 0.6, most: 9, tufts: 10, limbs: 0 } },
}

/** A spruce's usual height over its crown radius, for its tiers (`spruceTiers`). */
const SPRUCE_ASPECT = 6.4
/** How many tiers of branches a spruce's crown is, and how many branch tips round each tier's hem. */
const SPRUCE_TIERS = 6
const SPRUCE_TIPS = 14

/** How a leafy crown is made: its clumps, tufts and branches. */
interface CrownStyle {
  /** A clump's radius over the crown's smaller radius. */
  clump: number
  /** The most clumps, besides the core. */
  most: number
  /** Tufts tried over each clump (those facing inward are left out). */
  tufts: number
  /** The core's size over what fills the crown: under 1 leaves gaps between the clumps to see through. */
  core: number
  /** Branches from the trunk, into the lowest clumps. */
  limbs: number
  /** Strands of leaves hanging under each clump, as a birch's twigs do. */
  hang: number
}

/** The usual crown: clumps about a fifth of its width, a full core, tufts all over, no hanging strands. */
const CROWN: CrownStyle = { clump: 0.42, most: 12, tufts: 14, core: 1, limbs: 4, hang: 0 }
/** A tuft's reach over its clump's radius. */
const TUFT_REACH = 0.45
/** How far a clump's surface is pushed in and out, over its radius: lumpy rather than round. */
const CLUMP_LUMPS = 0.14
/**
 * How much of a leaf's light comes from its own clump's round shape, the rest from the whole crown's:
 * the many small faces shade as a few soft balls, as painted foliage does, rather than as facets.
 */
const CLUMP_SHADING = 0.55

/** A small seeded random source, so every build of a kind is the same tree. */
function seeded(seed: number): () => number {
  let s = seed >>> 0
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0
    return s / 4294967296
  }
}

/** `n` directions spread evenly over the sphere (a Fibonacci lattice), the first near the top. */
function spread(n: number): [number, number, number][] {
  const out: [number, number, number][] = []
  const turn = Math.PI * (3 - Math.sqrt(5))
  for (let i = 0; i < n; i++) {
    const y = 1 - ((i + 0.5) / n) * 2
    const r = Math.sqrt(1 - y * y)
    out.push([Math.cos(i * turn) * r, y, Math.sin(i * turn) * r])
  }
  return out
}

const normalise = (v: [number, number, number]): [number, number, number] => {
  const l = Math.hypot(v[0], v[1], v[2]) || 1
  return [v[0] / l, v[1] / l, v[2] / l]
}

/** Triangles with their own normals and colours, gathered into one geometry. */
class Builder {
  positions: number[] = []
  normals: number[] = []
  colors: number[] = []
  indices: number[] = []

  /** Adds a shape's triangles; `vertex` gives each of its points its normal and colour. */
  add(points: ArrayLike<number>, indices: ArrayLike<number>, vertex: (p: [number, number, number]) => { normal: [number, number, number]; color: [number, number, number] }) {
    const first = this.positions.length / 3
    for (let i = 0; i < points.length; i += 3) {
      const p: [number, number, number] = [points[i]!, points[i + 1]!, points[i + 2]!]
      const { normal, color } = vertex(p)
      this.positions.push(...p)
      this.normals.push(...normal)
      this.colors.push(...color)
    }
    for (let i = 0; i < indices.length; i++) this.indices.push(first + indices[i]!)
  }

  /**
   * Adds triangles whose points come with their normals and colours, each triangle wound to face the
   * way its points' normals do (counter-clockwise seen from outside, as every part of a tree is).
   */
  addFaced(points: readonly number[], normals: readonly number[], colors: readonly number[], indices: readonly number[]) {
    const first = this.positions.length / 3
    this.positions.push(...points)
    this.normals.push(...normals)
    this.colors.push(...colors)
    for (let t = 0; t < indices.length; t += 3) {
      const a = indices[t]!, b = indices[t + 1]!, c = indices[t + 2]!
      const e1 = [points[b * 3]! - points[a * 3]!, points[b * 3 + 1]! - points[a * 3 + 1]!, points[b * 3 + 2]! - points[a * 3 + 2]!]
      const e2 = [points[c * 3]! - points[a * 3]!, points[c * 3 + 1]! - points[a * 3 + 1]!, points[c * 3 + 2]! - points[a * 3 + 2]!]
      const face = [e1[1]! * e2[2]! - e1[2]! * e2[1]!, e1[2]! * e2[0]! - e1[0]! * e2[2]!, e1[0]! * e2[1]! - e1[1]! * e2[0]!]
      let along = 0
      for (const v of [a, b, c]) for (let k = 0; k < 3; k++) along += face[k]! * normals[v * 3 + k]!
      this.indices.push(first + a, first + (along < 0 ? c : b), first + (along < 0 ? b : c))
    }
  }

  geometry(): Geometry {
    return new Geometry({
      positions: new Float32Array(this.positions),
      normals: new Float32Array(this.normals),
      colors: new Float32Array(this.colors),
      indices: new Uint16Array(this.indices),
    })
  }
}

/** A point in a tree's space. */
type Vec3 = [number, number, number]

/**
 * A tube of bark along `path` (at least two points), `radii[i]` wide at each, with closed ends: a trunk
 * or a branch. Each ring has its first point twice, so its `uv.x` runs 0 to 1 round the tube without a
 * seam between, and the shader draws bark round the tube's own line however it leans or bends.
 */
function barkTube(path: readonly Vec3[], radii: readonly number[], sides: number, color: number): Geometry {
  const positions: number[] = []
  const normals: number[] = []
  const uvs: number[] = []
  const indices: number[] = []
  const ring = sides + 1
  const tangents = path.map((_, i) => {
    const a = path[Math.max(0, i - 1)]!, b = path[Math.min(path.length - 1, i + 1)]!
    return normalise([b[0] - a[0], b[1] - a[1], b[2] - a[2]])
  })
  path.forEach((p, i) => {
    const axis = tangents[i]!
    const side = normalise(Math.abs(axis[1]) < 0.9 ? [axis[2], 0, -axis[0]] : [1, 0, 0])
    const other: Vec3 = [axis[1] * side[2] - axis[2] * side[1], axis[2] * side[0] - axis[0] * side[2], axis[0] * side[1] - axis[1] * side[0]]
    for (let k = 0; k <= sides; k++) {
      const t = (k / sides) * Math.PI * 2
      const n: Vec3 = [side[0] * Math.cos(t) + other[0] * Math.sin(t), side[1] * Math.cos(t) + other[1] * Math.sin(t), side[2] * Math.cos(t) + other[2] * Math.sin(t)]
      positions.push(p[0] + n[0] * radii[i]!, p[1] + n[1] * radii[i]!, p[2] + n[2] * radii[i]!)
      normals.push(...n)
      uvs.push(k / sides, 0)
    }
  })
  for (let i = 0; i + 1 < path.length; i++) {
    for (let k = 0; k < sides; k++) {
      const a = i * ring + k, b = a + 1, c = a + ring, d = c + 1
      indices.push(a, c, b, b, c, d)
    }
  }
  // The ends: a point in the middle of each end ring, facing along the tube.
  for (const [i, sign] of [[0, -1], [path.length - 1, 1]] as const) {
    const middle = positions.length / 3
    const p = path[i]!, axis = tangents[i]!
    positions.push(...p)
    normals.push(axis[0] * sign, axis[1] * sign, axis[2] * sign)
    uvs.push(0, 0)
    for (let k = 0; k < sides; k++) indices.push(middle, i * ring + k, i * ring + k + 1)
  }
  // Each triangle wound to face the way its points' normals do.
  for (let t = 0; t < indices.length; t += 3) {
    const [a, b, c] = [indices[t]!, indices[t + 1]!, indices[t + 2]!]
    const e1 = [positions[b * 3]! - positions[a * 3]!, positions[b * 3 + 1]! - positions[a * 3 + 1]!, positions[b * 3 + 2]! - positions[a * 3 + 2]!]
    const e2 = [positions[c * 3]! - positions[a * 3]!, positions[c * 3 + 1]! - positions[a * 3 + 1]!, positions[c * 3 + 2]! - positions[a * 3 + 2]!]
    const face = [e1[1]! * e2[2]! - e1[2]! * e2[1]!, e1[2]! * e2[0]! - e1[0]! * e2[2]!, e1[0]! * e2[1]! - e1[1]! * e2[0]!]
    let along = 0
    for (const v of [a, b, c]) for (let k = 0; k < 3; k++) along += face[k]! * normals[v * 3 + k]!
    if (along < 0) [indices[t + 1], indices[t + 2]] = [c, b]
  }
  const col = Color.hex(color)
  const count = positions.length / 3
  const colors = new Float32Array(count * 3)
  for (let i = 0; i < count; i++) colors.set([col.r, col.g, col.b], i * 3)
  return new Geometry({ positions: new Float32Array(positions), normals: new Float32Array(normals), uvs: new Float32Array(uvs), colors, indices: new Uint16Array(indices) })
}

/** Sides of a near tree's trunk and of its branches. */
const TRUNK_TUBE_SIDES = 7
const LIMB_SIDES = 5

/**
 * Where a near trunk's middle line is at `t` of its height (0 at the foot, 1 at its top), off the tree's
 * axis: a birch's bows gently to one side, others stand straight.
 */
type TrunkLine = (t: number) => [number, number]
const STRAIGHT: TrunkLine = () => [0, 0]

/** A near tree's trunk from `part`, along `line`, reaching up to `top` (unit space) if that is above its own. */
function trunkTube(part: Part, line: TrunkLine, top = part.y1!): Geometry {
  const segments = line === STRAIGHT ? 1 : 6
  const path: Vec3[] = []
  const radii: number[] = []
  for (let i = 0; i <= segments; i++) {
    const t = i / segments
    const [x, z] = line(t)
    path.push([x, part.y0 + (top - part.y0) * t, z])
    // It narrows toward the top, a leader reaching on into the crown more than the rest.
    radii.push(part.rx! * (1 - t * (top > part.y1! ? 0.6 : 0.2)))
  }
  return barkTube(path, radii, TRUNK_TUBE_SIDES, part.color)
}

/**
 * A leafy crown in place of ellipsoid `part`, with branches from the trunk's top into it, in unit space.
 * It is built in metres over the crown radius (the tree's height is `aspect` of them) and squeezed into
 * unit space at the end, so the clumps are round on a tree of the usual size.
 *
 * The crown is a dark core with lumpy clumps over it, and each clump wears small flattened tufts on its
 * outward side, turned every way, whose points break its outline as leaves do. All of it is opaque, so
 * the GPU still shades each pixel once. Its light is painted: each point's normal is mostly its clump's
 * round one and partly the whole crown's, and its colour darkens toward the crown's middle and its
 * underside, so the clumps read as soft balls with shadowed gaps between them.
 */
/** The trunk a crown's branches grow from: its top (unit space), radius and colour, and its line. */
interface Trunk {
  top: number
  radius: number
  color: number
  line: TrunkLine
}

function leafyCrown(part: Part, trunk: Trunk | null, aspect: number, seed: number, style: CrownStyle = CROWN): Geometry {
  const random = seeded(seed)
  const rx = part.rx!
  const ry = part.ry! * aspect
  const centre: [number, number, number] = [part.x ?? 0, part.y0 * aspect, part.z ?? 0]
  const base = Color.hex(part.color)
  const clump = style.clump * Math.min(rx, ry)
  const build = new Builder()

  /** How far out in the crown `p` is (0 at its middle, 1 on its ellipsoid) and the crown's normal there. */
  const crownAt = (p: [number, number, number]) => {
    const d: [number, number, number] = [(p[0] - centre[0]) / rx, (p[1] - centre[1]) / ry, (p[2] - centre[2]) / rx]
    const out = Math.hypot(d[0], d[1], d[2])
    return { out, normal: normalise([d[0] / rx, d[1] / ry, d[2] / rx]), up: d[1] }
  }
  /** A leaf's colour: darker toward the crown's middle and underneath, a little warmer on top. */
  const leaf = (p: [number, number, number], shade: number): [number, number, number] => {
    const { out, up } = crownAt(p)
    const light = (0.38 + 0.62 * Math.min(1, Math.max(0, (out - 0.35) / 0.7))) * (0.8 + 0.2 * Math.max(-1, Math.min(1, up))) * shade
    const warm = Math.max(0, up) * 0.06
    return [base.r * light * (1 + warm), base.g * light, base.b * light * (1 - warm * 2)]
  }
  /** A point's normal: mostly its clump's round one, the rest the crown's. */
  const blended = (p: [number, number, number], middle: [number, number, number]) => {
    const own = normalise([p[0] - middle[0], p[1] - middle[1], p[2] - middle[2]])
    const crown = crownAt(p).normal
    return normalise([own[0] * CLUMP_SHADING + crown[0] * (1 - CLUMP_SHADING), own[1] * CLUMP_SHADING + crown[1] * (1 - CLUMP_SHADING), own[2] * CLUMP_SHADING + crown[2] * (1 - CLUMP_SHADING)])
  }

  // The core fills the gaps between the clumps, deep inside and dark; a smaller one leaves some open.
  const core = icosphereGeometry(1, 1)
  const corePoints = core.positions.slice()
  for (let i = 0; i < corePoints.length; i += 3) {
    corePoints[i] = corePoints[i]! * (rx - clump * 0.7) * style.core + centre[0]
    corePoints[i + 1] = corePoints[i + 1]! * (ry - clump * 0.7) * style.core + centre[1]
    corePoints[i + 2] = corePoints[i + 2]! * (rx - clump * 0.7) * style.core + centre[2]
  }
  build.add(corePoints, core.indices!, (p) => ({ normal: crownAt(p).normal, color: leaf(p, 0.75) }))

  // As many clumps as cover the crown's surface about once, the bottom left to the core.
  const area = 4 * Math.PI * Math.pow((rx * rx * rx * rx + 2 * rx * rx * ry * ry) / 3, 0.5)
  const count = Math.min(style.most, Math.max(5, Math.round((area / (Math.PI * clump * clump)) * 0.7)))
  const middles: [number, number, number][] = []
  const ball = icosphereGeometry(1, 1)
  const tuft = octahedronGeometry()
  for (const dir of spread(count)) {
    if (dir[1] < -0.7) continue
    const r = clump * (0.85 + 0.3 * random())
    const lean = normalise([dir[0] + (random() - 0.5) * 0.3, dir[1] + (random() - 0.5) * 0.3, dir[2] + (random() - 0.5) * 0.3])
    const middle: [number, number, number] = [
      centre[0] + lean[0] * (rx - r * 0.8),
      centre[1] + lean[1] * (ry - r * 0.8),
      centre[2] + lean[2] * (rx - r * 0.8),
    ]
    middles.push(middle)
    const shade = 0.92 + 0.16 * random()
    // The clump: a sphere pushed in and out at each of its corners.
    const points = ball.positions.slice()
    for (let i = 0; i < points.length; i += 3) {
      const lump = r * (1 + CLUMP_LUMPS * (random() * 2 - 1))
      points[i] = points[i]! * lump + middle[0]
      points[i + 1] = points[i + 1]! * lump + middle[1]
      points[i + 2] = points[i + 2]! * lump + middle[2]
    }
    build.add(points, ball.indices!, (p) => ({ normal: blended(p, middle), color: leaf(p, shade) }))
    // Its tufts, on the side away from the crown's middle: the rest are hidden.
    const outward = crownAt(middle).normal
    for (const t of spread(style.tufts)) {
      if (t[0] * outward[0] + t[1] * outward[1] + t[2] * outward[2] < -0.25) continue
      const reach = r * TUFT_REACH * (0.8 + 0.4 * random())
      // A little flattened along the clump's surface and turned about it, so each shows its own points,
      // and stands half out of it: seen at the outline, its points break the edge as leaves do.
      const across = normalise(Math.abs(t[1]) < 0.9 ? [t[2], 0, -t[0]] : [1, 0, 0])
      const spin = random() * Math.PI
      const u: [number, number, number] = [
        across[0] * Math.cos(spin) + (t[1] * across[2] - t[2] * across[1]) * Math.sin(spin),
        across[1] * Math.cos(spin) + (t[2] * across[0] - t[0] * across[2]) * Math.sin(spin),
        across[2] * Math.cos(spin) + (t[0] * across[1] - t[1] * across[0]) * Math.sin(spin),
      ]
      const v: [number, number, number] = [t[1] * u[2] - t[2] * u[1], t[2] * u[0] - t[0] * u[2], t[0] * u[1] - t[1] * u[0]]
      const at: [number, number, number] = [middle[0] + t[0] * r, middle[1] + t[1] * r, middle[2] + t[2] * r]
      const flat = 0.6
      const tp = tuft.positions
      const placed = new Float32Array(tp.length)
      for (let i = 0; i < tp.length; i += 3) {
        const x = tp[i]! * reach, y = tp[i + 1]! * reach * flat, z = tp[i + 2]! * reach
        placed[i] = at[0] + u[0] * x + t[0] * y + v[0] * z
        placed[i + 1] = at[1] + u[1] * x + t[1] * y + v[1] * z
        placed[i + 2] = at[2] + u[2] * x + t[2] * y + v[2] * z
      }
      build.add(placed, tuft.indices!, (p) => ({ normal: blended(p, middle), color: leaf(p, shade * 1.04) }))
    }
    // Strands hanging from the clump's underside: long thin tufts, straight down.
    for (let k = 0; k < style.hang; k++) {
      const a = random() * Math.PI * 2
      const out = r * (0.3 + 0.45 * random())
      const long = r * (0.9 + 0.7 * random())
      const thin = r * 0.2
      const top: Vec3 = [middle[0] + Math.cos(a) * out, middle[1] - r * 0.55, middle[2] + Math.sin(a) * out]
      const tp = tuft.positions
      const placed = new Float32Array(tp.length)
      for (let i = 0; i < tp.length; i += 3) {
        placed[i] = top[0] + tp[i]! * thin
        placed[i + 1] = top[1] - long * 0.5 + tp[i + 1]! * long * 0.5
        placed[i + 2] = top[2] + tp[i + 2]! * thin
      }
      build.add(placed, tuft.indices!, (p) => ({ normal: blended(p, middle), color: leaf(p, shade) }))
    }
  }

  const parts: Geometry[] = [build.geometry()]
  // Branches from the trunk up and out into the lowest clumps, each leaving the trunk somewhat below its
  // clump, as branches climb.
  if (trunk) {
    const top = trunk.top * aspect
    const low = [...middles].sort((a, b) => a[1] - b[1]).slice(0, style.limbs)
    for (const m of low) {
      const end: Vec3 = [centre[0] + (m[0] - centre[0]) * 0.8, centre[1] + (m[1] - centre[1]) * 0.8, centre[2] + (m[2] - centre[2]) * 0.8]
      const y = Math.min(top - trunk.radius * 2, Math.max(top * 0.4, end[1] - Math.hypot(end[0], end[2]) * 0.6))
      const [x, z] = trunk.line(y / top)
      parts.push(barkTube([[x, y, z], end], [trunk.radius * 0.6, trunk.radius * 0.25], LIMB_SIDES, trunk.color))
    }
  }
  return Geometry.merge(parts).scale(1, 1 / aspect, 1)
}

/** The solid a near tree of `kind` is drawn as: its parts, with each crown made leafy where the kind has it. */
function fullShape(kind: number, parts: readonly Part[]): Geometry {
  if (kind === TREE_SPRUCE) return spruceShape(parts)
  const leafy = LEAFY[kind]
  const trunkPart = parts.find((part) => part.shape === 'trunk')
  const others = parts.filter((part) => part.shape !== 'ellipsoid' && part !== trunkPart).map((part) => partGeometry(part, 'full'))
  if (!leafy) return Geometry.merge([...(trunkPart ? [trunkTube(trunkPart, STRAIGHT)] : []), ...others, ...parts.filter((part) => part.shape === 'ellipsoid').map((part) => partGeometry(part, 'full'))])
  const { aspect } = leafy
  const style = { ...CROWN, ...leafy.style }
  // One leafy crown, in place of the widest: its clumps already give it the lumps the smaller ones did.
  // A kind marked `every` makes each of its crowns leafy, the branches only into the widest.
  const ellipsoids = parts.filter((part) => part.shape === 'ellipsoid')
  const widest = ellipsoids.reduce((a, b) => (b.rx! > a.rx! ? b : a))
  const crowns = leafy.every ? ellipsoids : [widest]
  // A birch's trunk bows a little to one side, and its leader runs on up into the crown.
  const birch = kind === TREE_BIRCH
  const line: TrunkLine = birch ? (t) => [Math.sin(t * Math.PI) * BIRCH_BOW, Math.sin(t * Math.PI) * BIRCH_BOW * 0.4] : STRAIGHT
  const trunk: Trunk | null = trunkPart ? { top: trunkPart.y1!, radius: trunkPart.rx!, color: trunkPart.color, line } : null
  return Geometry.merge([
    ...(trunkPart ? [trunkTube(trunkPart, line, birch ? BIRCH_LEADER : trunkPart.y1!)] : []),
    ...others,
    ...crowns.map((crown, i) => leafyCrown(crown, crown === widest ? trunk : null, aspect, kind * 7919 + 104729 + i * 31337, style)),
  ])
}

/** How far a birch's trunk bows off its axis at half its height, in crown radii. */
const BIRCH_BOW = 0.12
/** How high a birch's leader reaches up into its crown, of the tree's height. */
const BIRCH_LEADER = 0.9

/**
 * A spruce's near solid: its trunk, and in place of its cones `SPRUCE_TIERS` tiers of branches inside
 * the same outline, each a drooping skirt whose hem is cut into `SPRUCE_TIPS` branch tips, so the outline
 * is jagged as a spruce's is and not three smooth cones. Each tier is turned and its tips varied so no
 * two tiers line up. Built in metres at `SPRUCE_ASPECT` and squeezed into unit space, as a leafy crown is.
 *
 * Its light is painted as the leafy crown's: a tier's normals lean outward more than its slope does, so
 * it shades as one soft skirt, and its colour darkens up under the hem, toward the notches between the
 * tips and on the lower tiers, so each tier stands out from the shade of the one above.
 */
function spruceShape(parts: readonly Part[]): Geometry {
  const random = seeded(TREE_SPRUCE * 7919 + 104729)
  const aspect = SPRUCE_ASPECT
  const cones = parts.filter((part) => part.shape === 'cone')
  const bottom = Math.min(...cones.map((part) => part.y0)) * aspect
  const top = Math.max(...cones.map((part) => part.y1!)) * aspect
  const width = Math.max(...cones.map((part) => part.rx!))
  const base = Color.hex(cones[0]!.color)
  const build = new Builder()
  const tall = top - bottom
  for (let tier = 0; tier < SPRUCE_TIERS; tier++) {
    const up = tier / SPRUCE_TIERS
    // The hem's height and reach: the tiers climb a little faster than they narrow, so they overlap.
    const hem = bottom + tall * up * 0.86
    const reach = width * (1 - up * 0.82) * (0.95 + 0.1 * random())
    const apex = tier === SPRUCE_TIERS - 1 ? top : Math.min(top, hem + tall * 0.34)
    const rise = apex - hem
    const droop = rise * 0.12
    const turn = random() * Math.PI * 2
    const shade = (0.78 + 0.22 * up) * (0.94 + 0.12 * random())
    const color = (light: number): [number, number, number] => [base.r * light * shade, base.g * light * shade, base.b * light * shade]
    const n = SPRUCE_TIPS
    const points: number[] = []
    const normals: number[] = []
    const colors: number[] = []
    const at = (r: number, y: number, a: number, normal: [number, number, number], light: number) => {
      points.push(Math.cos(a) * r, y, Math.sin(a) * r)
      normals.push(...normal)
      colors.push(...color(light))
    }
    // The hem: tips (even) out and down, notches (odd) in and up.
    const hemAt: [number, number, number][] = []
    for (let k = 0; k < n * 2; k++) {
      const a = turn + (k / (n * 2)) * Math.PI * 2
      const tip = k % 2 === 0
      const r = tip ? reach * (0.92 + 0.16 * random()) : reach * 0.74
      const y = tip ? hem - droop * (0.7 + 0.6 * random()) : hem + rise * 0.06
      hemAt.push([r, y, a])
      at(r, y, a, normalise([Math.cos(a), 0.55, Math.sin(a)]), tip ? 1 : 0.8)
    }
    // The middle ring, sagging below the straight line from hem to apex as a branch's weight bends it.
    for (let k = 0; k < n; k++) {
      const a = turn + ((k * 2 + 1) / (n * 2)) * Math.PI * 2
      at(reach * 0.48, hem + rise * 0.4, a, normalise([Math.cos(a), 0.9, Math.sin(a)]), 0.95)
    }
    const apexAt = n * 3
    at(0, apex, 0, [0, 1, 0], 1)
    // The underside: the hem again, facing down and in shade, closing to a point under the tier.
    const belowAt = apexAt + 1
    for (const [r, y, a] of hemAt) at(r, y, a, normalise([Math.cos(a) * 0.4, -1, Math.sin(a) * 0.4]), 0.5)
    const under = belowAt + n * 2
    at(0, hem + rise * 0.18, 0, [0, -1, 0], 0.4)
    const indices: number[] = []
    for (let k = 0; k < n; k++) {
      const tip = 2 * k, notch = 2 * k + 1, next = (2 * k + 2) % (n * 2)
      const mid = n * 2 + k, midNext = n * 2 + ((k + 1) % n)
      indices.push(mid, notch, tip, midNext, notch, mid, midNext, next, notch, apexAt, midNext, mid)
      indices.push(under, belowAt + tip, belowAt + notch, under, belowAt + notch, belowAt + next)
    }
    build.addFaced(points, normals, colors, indices)
  }
  const crown = build.geometry().scale(1, 1 / aspect, 1)
  return Geometry.merge([...parts.filter((part) => part.shape === 'trunk').map((part) => trunkTube(part, STRAIGHT)), crown])
}

/** The archetypes in the order of their tiles in the impostor atlas. */
const KIND_TILES = Object.keys(PROTOTYPES).map(Number)

/** A kind's shapes and what its impostor's pictures frame. */
export interface TreeShapes {
  /** The solid, with the middle of its bounds in every vertex's `custom` (w 0), so the shader can fade it. */
  full: Geometry
  /** One quad, all four corners at the middle, which the foliage shader opens to face the camera. */
  impostor: Geometry
  /** What a tree casts its shadow with, at the impostor's two levels: the shadow pass runs no vertex code. */
  coarse: Geometry
  shadow: Geometry
  frame: ImpostorFrame
}

/**
 * The impostor's quad: `uv` is the corner (-1 to 1 across and up), and `custom` the scale of its
 * pictures' depth (unit space), the kind's tile in the atlas, and the radius and half height of the
 * upright cylinder about the middle that holds the tree, which the quad and the pictures are fitted to
 * (w, never 0, also says it is an impostor). A quad any bigger is pixels the shader works on only to
 * throw away.
 */
function impostorGeometry(frame: ImpostorFrame, tile: number): Geometry {
  const positions: number[] = []
  const uvs: number[] = []
  const custom: number[] = []
  for (const [u, v] of [[-1, -1], [1, -1], [1, 1], [-1, 1]] as const) {
    positions.push(...frame.center)
    uvs.push(u, v)
    custom.push(frame.radius, tile, frame.across, frame.half)
  }
  return new Geometry({
    positions: new Float32Array(positions),
    normals: new Float32Array([0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0]),
    uvs: new Float32Array(uvs),
    custom: new Float32Array(custom),
    colors: new Float32Array(12).fill(1),
    indices: new Uint16Array([0, 1, 2, 0, 2, 3]),
    // Its corners move: it is culled by the solid's bounds.
    bounds: { x: frame.center[0], y: frame.center[1], z: frame.center[2], radius: frame.radius },
  })
}

/** What an impostor's pictures of `solid` frame: the upright cylinder about the middle of its bounds that holds it. */
export function frameAround(solid: Geometry): ImpostorFrame {
  const bounds = solid.bounds
  let across = 0, half = 0
  for (let i = 0; i < solid.vertexCount; i++) {
    const p = solid.positions
    across = Math.max(across, Math.hypot(p[i * 3]! - bounds.x, p[i * 3 + 2]! - bounds.z))
    half = Math.max(half, Math.abs(p[i * 3 + 1]! - bounds.y))
  }
  return { center: [bounds.x, bounds.y, bounds.z], radius: bounds.radius, across, half }
}

/** Tree `kind` built plainly from its parts (smooth crowns and cones), as the near solid was before leafy crowns. */
export function plainShape(kind: number): Geometry {
  return Geometry.merge((PROTOTYPES[kind] ?? PROTOTYPES[TREE_BIRCH]!).map((part) => partGeometry(part, 'full')))
}

/**
 * The shapes of tree `kind`. The solid's bounds are widened to hold the swollen shadow crowns too,
 * since every level is culled by them.
 */
export function treeGeometries(kind: number): TreeShapes {
  const parts = PROTOTYPES[kind] ?? PROTOTYPES[TREE_BIRCH]!
  const tile = Math.max(0, KIND_TILES.indexOf(PROTOTYPES[kind] ? kind : TREE_BIRCH))
  const shape = (detail: Detail) => Geometry.merge(partsAt(parts, detail).map((part) => partGeometry(part, detail)))
  const full = fullShape(kind, parts)
  const coarse = shape('coarse')
  const shadow = shape('shadow')
  const bounds = full.bounds
  const frame = frameAround(full)
  for (const other of [coarse, shadow]) {
    const b = other.bounds
    bounds.radius = Math.max(bounds.radius, Math.hypot(b.x - bounds.x, b.y - bounds.y, b.z - bounds.z) + b.radius)
  }
  full.custom = new Float32Array(full.vertexCount * 4)
  // Bark is told by its colour, trunk and branches alike, and marked for the shader's pattern.
  const brown = Color.hex(TRUNK_BARK), white = Color.hex(TRUNK_BIRCH)
  const colors = full.colors!
  // The colours are kept as 32-bit floats, so they are compared to within a hair.
  const same = (i: number, c: Color) => Math.abs(colors[i * 3]! - c.r) + Math.abs(colors[i * 3 + 1]! - c.g) + Math.abs(colors[i * 3 + 2]! - c.b) < 1e-5
  for (let i = 0; i < full.vertexCount; i++) {
    const wood = same(i, brown) ? BARK_FURROWED : same(i, white) ? BARK_BIRCH : 0
    full.custom.set([bounds.x, bounds.y, bounds.z, wood], i * 4)
  }
  return { full, impostor: impostorGeometry(frame, tile), coarse, shadow, frame }
}

/**
 * What a solid's `custom.w` says of its surface: 0 leaves and everything else, or one of the barks the
 * foliage shader draws a pattern on (`bark`). An impostor's is above 0.
 */
export const BARK_FURROWED = -1
export const BARK_BIRCH = -2
/** Metres from the eye over which the bark's pattern fades out, before it is too fine to draw cleanly. */
const BARK_NEAR = 18
const BARK_FAR = 40

/** How far the wind leans a treetop, metres per metre of the tree's height, at a middling gust. */
const TREE_SWAY = 0.014
/** How far a leaf shivers in the wind at the top of a crown, metres. */
const LEAF_SHIVER = 0.03

/** Metres from the eye inside which foliage dissolves, so a view from inside a crown stays clear. */
const NEAR_FADE = 2.5

/**
 * The foliage's own shader: the cel look of the standard one (four bands, sun and hemisphere, the band
 * edges softened), and on leaves more between light and shade, a cool shadow side and a warm lit one, and
 * the sun showing through a crown's edges from behind it. Then a 4×4
 * ordered dither that removes pixels within `NEAR_FADE` of the camera and fades a tree from its solid to
 * its impostor. It is a material of its own so the shared mesh shader, and every other material, stays
 * as it was. Trees take no shadow.
 *
 * The fade: `extra[0].w` is how far into the band the tree is (0 to 1, the same for both, worked out from
 * the middle of its bounds). The impostor draws that share of each pixel's samples and the solid the
 * rest (`impostorSamples`), so the two blend as the frame resolves; with one sample a pixel, the dither
 * gives each pixel to one or the other.
 *
 * The impostor: `extra[0].xyz` is the pixel's place in the tree's unit space, from the middle of its
 * bounds, and `extra[1].xy` where the direction to the camera from there falls on the grid of pictures. The three pictures nearest that
 * direction are each met where the pixel's line of sight crosses the picture's plane, and blended by
 * how near they are (a dither picking one of them a pixel was cheaper, but the pictures are far enough
 * apart that it speckled); the surface found is lit as the solid's would be. The pictures' coverage says
 * how much of the pixel lies inside the outline, and that many of its samples are drawn, so the edge is
 * as smooth as a solid's. The quad keeps its own depth: writing the surface's cost more than a
 * millisecond in a forest, as it stops the GPU skipping hidden pixels.
 */
const FOLIAGE_WGSL = /* wgsl */ `
const BANDS_AT = array<f32, 4>(0.376, 0.588, 0.804, 1.0);
// The share of each light band over which it blends into the next.
const BAND_SOFT = 0.3;
// How strongly sunlight from behind shows through a crown's edges, as a share of the sun's light.
const THROUGH = 0.45;
// The power the light bands are raised to on leaves, for more between a crown's lit and shaded sides.
const LEAF_CONTRAST = 1.8;
// Ridges round a trunk of furrowed bark, and the plates they break into a metre up it.
const BARK_RIDGES = 9.0;
const BARK_PLATES = 1.6;
const BAYER_AT = array<u32, 16>(0u, 8u, 2u, 10u, 12u, 4u, 14u, 6u, 3u, 11u, 1u, 9u, 15u, 7u, 13u, 5u);
const FRAMES = ${IMPOSTOR_FRAMES.toFixed(1)};
const FRAME_PX = ${IMPOSTOR_FRAME_PX.toFixed(1)};
const ATLAS_PX = ${IMPOSTOR_ATLAS_PX.toFixed(1)};
const TILES = ${IMPOSTOR_TILES.toFixed(1)};
const MARGIN = ${IMPOSTOR_MARGIN.toFixed(6)};
const SPREAD = ${IMPOSTOR_SPREAD.toFixed(1)};
// How much of a pixel must lie inside an impostor's outline for each of its four samples to be the tree's.
const EDGE_AT = array<f32, 4>(0.375, 0.125, 0.875, 0.625);

/** A value 0 to 1 that changes at random from one cell of a grid to the next. */
fn cellHash(p: vec2f) -> f32 {
  return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453);
}

/**
 * Bark drawn over \`color\` at \`around\` (0 to 1 round its tube, \`barkTube\`) and \`up\` (metres up the tree
 * from its foot), for bark \`kind\` (\`BARK_FURROWED\` or \`BARK_BIRCH\`), fading out \`far\` metres from the eye.
 * Hard-edged, as the cel bands are. The colour is linear, so a mark meant to read as dark is mixed nearly
 * all the way to black.
 *
 * Furrowed bark is ridges up the trunk, wandering side to side and broken across into plates of their
 * own shade, with dark furrows between.
 *
 * Birch is white, faintly banded where its bark peels, with dark lenticels across it: short dashes of
 * many lengths in uneven rows, each row turned its own way round. Here and there a dark scar droops either
 * side of where a branch was, and toward the foot dark, deeply fissured bark breaks through in patches,
 * closing over it at the ground.
 */
fn bark(color: vec3f, around: f32, up: f32, kind: f32, far: f32) -> vec3f {
  let near = 1.0 - smoothstep(${BARK_NEAR.toFixed(1)}, ${BARK_FAR.toFixed(1)}, far);
  if (near <= 0.0) {
    return color;
  }
  if (kind > -1.5) {
    let u = around * BARK_RIDGES + 0.25 * sin(up * 1.3 + around * 6.2832) + 0.12 * sin(up * 3.7);
    let ridge = floor(u);
    // 0 in the middle of a ridge, 1 at the furrow either side of it.
    let across = abs(fract(u) - 0.5) * 2.0;
    let furrow = smoothstep(0.6, 0.78, across);
    let v = up * BARK_PLATES + cellHash(vec2f(ridge, 3.0)) * 7.0;
    let crack = smoothstep(0.86, 0.93, fract(v)) * (1.0 - furrow);
    let plate = 0.9 + 0.2 * cellHash(vec2f(ridge, floor(v)));
    let shade = plate * mix(1.0, 0.45, furrow) * (1.0 - 0.4 * crack);
    return mix(color, color * shade, near);
  }
  // Lenticels: rows a fifth of a metre apart, each at its own height in its row and its own thickness.
  let rows = up * 5.5;
  let row = floor(rows);
  let inRow = fract(rows);
  let seed = cellHash(vec2f(row, 1.0));
  let middle = 0.25 + 0.5 * cellHash(vec2f(row, 2.0));
  let thick = 0.05 + 0.07 * seed;
  let across = 1.0 - smoothstep(thick, thick + 0.05, abs(inRow - middle));
  // Round the trunk a whole number of places for dashes, so the row meets itself where it closes.
  let places = 5.0 + floor(seed * 4.0);
  let along = around * places + cellHash(vec2f(row, 3.0));
  let place = floor(along);
  let h = cellHash(vec2f(place, row));
  let start = 0.5 * cellHash(vec2f(row, place + 7.0));
  let dash = across * step(0.3, h) * step(start, fract(along)) * step(fract(along), start + 0.15 + 0.45 * h);
  // Scars: a few cells up the trunk hold one, a dark brow drooping either side of a middle point.
  let scarCell = vec2f(floor(around * 3.0), floor(up * 0.9));
  let scarAt = fract(vec2f(around * 3.0, up * 0.9)) - 0.5;
  let brow = abs(scarAt.y + 0.18 - 0.45 * abs(scarAt.x) * 2.0 + 0.35 * scarAt.x * scarAt.x * 4.0);
  let scar = step(0.72, cellHash(scarCell + vec2f(5.0, 9.0))) * (1.0 - smoothstep(0.03, 0.06, brow)) * step(abs(scarAt.x), 0.42) * step(1.0, up);
  // The foot: patches of dark bark, more of them lower, all of it at the ground.
  let foot = 1.0 - smoothstep(0.4, 2.4, up + 0.35 * sin(around * 25.1327 + up * 3.0));
  let broken = cellHash(vec2f(floor(around * 6.0), floor(up * 3.0 + 0.6 * sin(around * 12.5664))));
  let darkBark = step(1.0 - foot, broken * 0.8 + 0.1) * step(0.12, foot);
  // In the dark bark, grey ridges between its deep fissures, running up.
  let fissure = abs(fract(around * 14.0 + 0.2 * sin(up * 4.1 + around * 31.4159)) - 0.5) * 2.0;
  let footColor = mix(vec3f(0.03, 0.028, 0.025), vec3f(0.16, 0.15, 0.13), smoothstep(0.55, 0.8, fissure));
  // Faint bands where the bark peels: some a touch warmer, some greyer.
  let band = cellHash(vec2f(floor(up * 2.2 + 0.3 * sin(around * 6.2832)), 4.0));
  var white = color * vec3f(0.94 + 0.08 * band, 0.94 + 0.06 * band, 0.92 + 0.04 * band);
  white = mix(white, vec3f(0.04, 0.035, 0.03), max(dash, scar));
  return mix(color, mix(white, footColor, darkBark), near);
}

/** The dither's place (0 to 15) at \`pixel\`. */
fn ditherAt(pixel: vec2u) -> u32 {
  // Copied to a variable first: WGSL only indexes a constant array with a constant.
  var bayer = BAYER_AT;
  return bayer[(pixel.y & 3u) * 4u + (pixel.x & 3u)];
}

/**
 * Which of a pixel's samples (a bit each) a tree \`fade\` of the way into its impostor draws as the
 * impostor; its solid draws the rest. With four samples a pixel the share is counted out in samples, the
 * dither rounding it either way and which sample goes first turning from pixel to pixel, so the two are
 * blended as the frame resolves. With one, the dither gives the whole pixel to one or the other.
 */
fn impostorSamples(fade: f32, pixel: vec2u, dither: f32) -> u32 {
  if (SAMPLES == 1u) {
    return select(0u, 1u, dither < fade);
  }
  let turn = pixel.x ^ (pixel.y << 1u);
  var mask = 0u;
  for (var i = 0u; i < 4u; i++) {
    if ((f32((i + turn) & 3u) + dither) * 0.25 < fade) {
      mask |= 1u << i;
    }
  }
  return mask;
}

/** The direction picture \`cell\` of a kind's grid was taken from (\`decodeView\` in impostors.ts). */
fn frameView(cell: vec2f) -> vec3f {
  let ab = cell / (FRAMES - 1.0) * 2.0 - 1.0;
  let p = vec2f(ab.x + ab.y, ab.x - ab.y) * 0.5;
  // How far in from the grid's edge, squared, is the direction's share of height: the rings crowd the horizon.
  let inward = max(0.0, 1.0 - abs(p.x) - abs(p.y));
  let rise = inward * inward;
  let level = p * ((1.0 - rise) / max(1.0 - inward, 1e-6));
  return normalize(vec3f(level.x, rise, level.y));
}

/** One picture's colour and coverage, normal and surface point (unit space, from the middle), weighted. */
struct Picture {
  albedo: vec4f,
  normal: vec3f,
  hit: vec3f,
}

/**
 * Picture \`cell\` as seen along \`sight\` from \`q\`, weighted by \`weight\` and its coverage there. \`whole\`
 * reads it wherever the line of sight falls and weighs it by \`weight\` alone: a texel off the tree holds
 * the colour, normal and depth of the nearest on it, so a pixel whose middle misses the tree but some of
 * whose samples are the tree's still finds the tree's surface (\`blend\`).
 */
fn picture(cell: vec2f, weight: f32, q: vec3f, sight: vec3f, fit: vec4f, lod: f32, whole: bool) -> Picture {
  var out: Picture;
  let v = frameView(cell);
  var right = vec3f(1.0, 0.0, 0.0);
  let level = vec2f(v.z, -v.x);
  if (dot(level, level) > 1e-12) {
    let r = normalize(level);
    right = vec3f(r.x, 0.0, r.y);
  }
  let up = cross(v, right);
  // Where the line of sight through the pixel crosses the picture's plane.
  let onPlane = q - sight * (dot(q, v) / max(dot(sight, v), 0.05));
  // The picture is stretched over the outline, from there, of the cylinder that holds the tree (\`frameExtent\`).
  let span = vec2f(fit.z, fit.z * v.y + fit.w * length(v.xz)) * MARGIN;
  let local = vec2f(dot(onPlane, right), dot(onPlane, up)) / span;
  if (weight <= 0.0 || (!whole && (abs(local.x) > 1.0 || abs(local.y) > 1.0))) {
    return out;
  }
  let inside = clamp(vec2f(local.x, -local.y) * 0.5 + 0.5, vec2f(0.5 / FRAME_PX), vec2f(1.0 - 0.5 / FRAME_PX));
  // The kind's tile reaches here interpolated, and some GPUs (NVIDIA's) bring a constant through a
  // hair off: 3 as 2.9999998, whose row and column are another kind's. Rounded, it is exact.
  let tile = round(fit.y);
  let tileAt = vec2f(tile % TILES, floor(tile / TILES)) * FRAMES;
  let uv = (tileAt + cell + inside) * (FRAME_PX / ATLAS_PX);
  let albedo = textureSampleLevel(impostorAlbedo, materialSampler, uv, lod);
  // Well off the tree: nothing more to read. Most of the quad is.
  if (albedo.a <= 0.0 && !whole) {
    return out;
  }
  let share = select(weight * albedo.a, weight, whole);
  // The depth pre-pass needs only the coverage.
  if (PREPASS == 1u) {
    out.albedo = vec4f(0.0, 0.0, 0.0, share);
    return out;
  }
  let nd = textureSampleLevel(impostorNormalDepth, materialSampler, uv, lod);
  out.albedo = vec4f(albedo.rgb * share, share);
  out.normal = (nd.rgb * 2.0 - 1.0) * share;
  out.hit = (onPlane + v * ((nd.a * 2.0 - 1.0) * fit.x)) * share;
  return out;
}

/**
 * The three pictures around the direction to the camera (\`g\`, where it falls on their grid), each
 * weighted by how near the direction is to it, added up: the albedo's alpha is the coverage.
 */
fn blend(g: vec2f, q: vec3f, sight: vec3f, fit: vec4f, lod: f32, whole: bool) -> Picture {
  let cell = clamp(floor(g), vec2f(0.0), vec2f(FRAMES - 2.0));
  let f = g - cell;
  var a: Picture;
  var b: Picture;
  var c: Picture;
  if (f.x + f.y < 1.0) {
    a = picture(cell, 1.0 - f.x - f.y, q, sight, fit, lod, whole);
    b = picture(cell + vec2f(1.0, 0.0), f.x, q, sight, fit, lod, whole);
    c = picture(cell + vec2f(0.0, 1.0), f.y, q, sight, fit, lod, whole);
  } else {
    a = picture(cell + vec2f(1.0, 1.0), f.x + f.y - 1.0, q, sight, fit, lod, whole);
    b = picture(cell + vec2f(0.0, 1.0), 1.0 - f.x, q, sight, fit, lod, whole);
    c = picture(cell + vec2f(1.0, 0.0), 1.0 - f.y, q, sight, fit, lod, whole);
  }
  var out: Picture;
  out.albedo = a.albedo + b.albedo + c.albedo;
  out.normal = a.normal + b.normal + c.normal;
  out.hit = a.hit + b.hit + c.hit;
  return out;
}

/** A world vector put into the tree's unit space: the inverse of \`treeToWorld\`. */
fn worldToTree(w: vec3f, e: vec4f) -> vec3f {
  return vec3f(e.x * w.x + e.y * w.z, e.z * w.y, e.x * w.z - e.y * w.x) / vec3f(e.w * e.w, e.z * e.z, e.w * e.w);
}

/** A vector in the tree's unit space put into the world: \`e\` is the model's first column's x and z, then height and crown. */
fn treeToWorld(u: vec3f, e: vec4f) -> vec3f {
  return vec3f(e.x * u.x - e.y * u.z, e.z * u.y, e.y * u.x + e.x * u.z);
}

fn surface(given: Surface) -> vec4f {
  // Before anything is discarded: how many texels of a picture a pixel spans, for the mip level.
  let texels = fwidth(given.uv) * (FRAME_PX * 0.5);
  let lod = clamp(log2(max(max(texels.x, texels.y), 1e-4)), 0.0, 3.0);
  var s = given;
  let fade = s.extra[0].w;
  let pixel = vec2u(given.pixel);
  let dither = (f32(ditherAt(pixel)) + 0.5) / 16.0;
  let impostor = s.custom.w > 0.0;
  // After the depth pre-pass, a sample drawn at all is one the pre-pass kept: nothing to discard.
  let keep = PREPASS == 2u;
  let theirs = impostorSamples(fade, pixel, dither);
  if (impostor) {
    if (theirs == 0u && !keep) {
      discard;
    }
    let q = s.extra[0].xyz;
    let e = s.extra[2];
    // The pictures are picked by the direction to the camera from the tree's middle, the same over the
    // whole tree, but looked up along this pixel's own line of sight: a near tree is seen in perspective.
    let sight = normalize(worldToTree(frame.cameraPos.xyz - given.world, e));
    // The three pictures around the direction to the camera: where it falls on their grid.
    let g = s.extra[1].xy;
    var p = blend(g, q, sight, s.custom, lod, false);
    let cover = p.albedo.a;
    // The coverage is a distance to the silhouette, a half on it and \`SPREAD\` texels from none to whole:
    // how much of the pixel is inside the outline, and so which of its samples are the tree's.
    var mine = theirs;
    if (SAMPLES == 1u) {
      mine = select(mine, 0u, cover < 0.5);
    } else {
      let within = (cover - 0.5) * (2.0 * SPREAD) / max(0.5 * (texels.x + texels.y), 1e-3) + 0.5;
      var edges = EDGE_AT;
      for (var i = 0u; i < 4u; i++) {
        if (edges[i] >= within) {
          mine &= ~(1u << i);
        }
      }
    }
    if (!keep) {
      if (mine == 0u) {
        discard;
      }
      surfaceMask = mine;
    }
    if (PREPASS == 1u) {
      return vec4f(0.0);
    }
    // A small far tree spans many texels of its picture in a pixel, so the outline's distance can give it
    // samples of a pixel whose middle is off it, where every picture's coverage is nothing. Lit from
    // nothing, its normal would be no direction at all and its colour not a number (black on NVIDIA's
    // GPUs, a speck along every far treeline and blots in the water's mirror): it takes the nearest of the
    // tree instead.
    if (cover <= 0.0) {
      p = blend(g, q, sight, s.custom, lod, true);
    }
    let weight = max(p.albedo.a, 1e-4);
    let n = treeToWorld(p.normal / weight / vec3f(e.w * e.w, e.z * e.z, e.w * e.w), e);
    // Normals that cancel out face the camera rather than nowhere.
    s.normal = select(normalize(frame.cameraPos.xyz - given.world), normalize(n), dot(n, n) > 1e-12);
    s.world = given.world + treeToWorld(p.hit / weight - q, e);
    s.color = vec4f(s.color.rgb * p.albedo.rgb / weight, s.color.a);
  } else {
    if (!keep) {
      let mine = ~theirs & ((1u << SAMPLES) - 1u);
      if (mine == 0u) {
        discard;
      }
      surfaceMask = mine;
    }
    // Where a fading solid really is: its corners were pressed toward the impostor's card.
    s.world = given.world + s.extra[1].xyz;
  }
  let eye = distance(s.world, frame.cameraPos.xyz) / ${NEAR_FADE.toFixed(1)};
  if (eye < dither && !keep) {
    discard;
  }
  if (PREPASS == 1u) {
    return vec4f(0.0);
  }
  // The four cel bands, each edge softened over the last \`BAND_SOFT\` of its band, so a crown's clumps
  // read as soft painted balls rather than hard facets.
  let wrap = clamp(dot(s.normal, frame.sunDir.xyz) * 0.5 + 0.5, 0.0, 0.999);
  let at = wrap * 4.0;
  let band = u32(at);
  var bands = BANDS_AT;
  let sun = mix(bands[band], bands[min(band + 1u, 3u)], smoothstep(1.0 - BAND_SOFT, 1.0, fract(at)));
  let hemi = mix(frame.hemiGround.rgb, frame.hemiSky.rgb, 0.5 * s.normal.y + 0.5);
  if (!impostor && s.custom.w < -0.5) {
    s.color = vec4f(bark(s.color.rgb, given.uv.x, s.extra[0].y, s.custom.w, distance(s.world, frame.cameraPos.xyz)), s.color.a);
  }
  let albedo = under(s.color.rgb, s);
  // How much of a leaf this is, by its colour: green, not bark, snow or a flag.
  let leaf = smoothstep(0.0, 0.03, albedo.g - max(albedo.r, albedo.b));
  // Leaves in shade cool toward blue, leaves in the sun warm toward yellow.
  let tint = mix(vec3f(0.86, 0.96, 1.1), vec3f(1.05, 1.02, 0.88), smoothstep(0.35, 0.75, wrap));
  // Sun through the leaves: toward a sun behind the crown, its edges glow.
  let view = normalize(frame.cameraPos.xyz - s.world);
  let behind = pow(max(dot(-view, frame.sunDir.xyz), 0.0), 3.0);
  let edge = 1.0 - max(dot(s.normal, view), 0.0);
  let through = behind * (0.35 + 0.65 * edge) * THROUGH;
  // Leaves take the bands with more between light and shade: a crown's lit side bright, its far side deep.
  let banded = mix(sun, pow(sun, LEAF_CONTRAST), leaf);
  let lit = frame.sunColor.rgb * (banded + through * leaf) + hemi + pointLight(s.world, s.normal);
  return vec4f(albedo * mix(vec3f(1.0), tint, leaf) / PI * lit, s.color.a);
}
`

/**
 * Opens an impostor's quad to face the camera and hands the fragment what it needs to find the tree
 * in its pictures; a solid stays where it is. Both work out how far into the fade they are.
 */
const IMPOSTOR_WGSL = /* wgsl */ `
/** Where on the grid of a kind's pictures (0 to its last, each way) a direction in unit space falls: \`encodeView\` in impostors.ts. */
fn viewCell(toward: vec3f) -> vec2f {
  let level = abs(toward.x) + abs(toward.z);
  let rise = max(toward.y, 0.0);
  let p = toward.xz * ((1.0 - sqrt(rise / max(level + rise, 1e-9))) / max(level, 1e-9));
  return (vec2f(p.x + p.y, p.x - p.y) * 0.5 + 0.5) * ${(IMPOSTOR_FRAMES - 1).toFixed(1)};
}

/**
 * How far the wind moves a point \`up\` of the way up a tree \`height\` metres tall standing at \`foot\`,
 * metres. The tree bends downwind, stiff at the foot and freest at the top, as a gust rolls downwind
 * across the trees and each tree swings on its own time, rocking a little across the wind too; the top
 * dips as it leans, so the trunk keeps about its length. A leaf (\`leaf\` 1) shivers on top of that,
 * each point its own way. \`params.wind\` is the wind's direction along x and z and how far it leans a
 * treetop, metres per metre of height.
 */
fn sway(at: vec3f, foot: vec3f, height: f32, up: f32, leaf: f32) -> vec3f {
  let strength = params.wind.z;
  if (strength <= 0.0) {
    return vec3f(0.0);
  }
  let wind = params.wind.xy;
  let time = frame.fog.w;
  let phase = dot(foot.xz, vec2f(0.37, 0.61));
  let gust = sin(dot(foot.xz, wind) * 0.06 - time * 0.8) * 0.5 + 0.5;
  let swing = sin(time * 1.1 + phase) * 0.35 + sin(time * 1.9 + phase * 1.7) * 0.15;
  let rock = vec2f(-wind.y, wind.x) * sin(time * 1.4 + phase * 2.3) * 0.25;
  let bend = clamp(up, 0.0, 1.0);
  let lean = (wind * (0.35 + 0.65 * gust + swing) + rock) * (bend * bend * height * strength);
  let dip = 0.5 * dot(lean, lean) / max(height * bend, 0.5);
  let shiver = sin(time * 7.0 + dot(at, vec3f(1.7, 2.3, 1.3))) * leaf * bend * ${LEAF_SHIVER.toFixed(3)};
  return vec3f(lean.x + shiver * 0.6, shiver * 0.3 - dip, lean.y + shiver * 0.5);
}

fn displace(world: vec3f, custom: vec4f, color: vec4f) -> vec3f {
  let c0 = vertexModel[0].xyz;
  let c1 = vertexModel[1].xyz;
  let c2 = vertexModel[2].xyz;
  let impostor = custom.w > 0.0;
  // The middle of the tree's bounds: an impostor's corners all start there, a solid carries it.
  var middle = world;
  if (!impostor) {
    middle = (vertexModel * vec4f(custom.xyz, 1.0)).xyz;
  }
  let band = max(params.fadeBand, 1e-3);
  let into = distance(middle, frame.cameraPos.xyz) - params.fadeFrom + band;
  var fade = clamp(into / band, 0.0, 1.0);
  // A solid's point in the tree's own frame, in metres from its foot: its height up the tree is the bark's.
  let fromFoot = world - vertexModel[3].xyz;
  vertexExtra[0] = vec4f(dot(c0, fromFoot) / length(c0), dot(c1, fromFoot) / length(c1), dot(c2, fromFoot) / length(c2), fade);
  let toward = normalize(frame.cameraPos.xyz - middle);
  let foot = vertexModel[3].xyz;
  let height = length(c1);
  if (!impostor) {
    // Swayed first, the bark's pattern above taken where the point stands still, so it stays on the trunk.
    // A leaf is told by its colour, as the fragment tells it.
    let leaf = smoothstep(0.0, 0.03, color.g - max(color.r, color.b));
    let moved = world + sway(world, foot, height, dot(c1, fromFoot) / (height * height), leaf);
    // Fading, the solid's pixels lie among the impostor's, which are on a flat card through the tree's
    // middle: the ink would draw a line at every step between the two. So the solid is pressed toward
    // that card, each corner slid along its own line of sight (it stays where it is on the screen), to a
    // twentieth of its depth either side, over the third of a band before the fade starts. Its light is
    // worked out where it really is.
    let press = clamp(into / (band / 3.0) + 1.0, 0.0, 1.0);
    if (press <= 0.0) {
      return moved;
    }
    let sight = moved - frame.cameraPos.xyz;
    let onCard = dot(middle - frame.cameraPos.xyz, toward) / dot(sight, toward);
    let pressed = frame.cameraPos.xyz + sight * mix(1.0, onCard, press * 0.95);
    vertexExtra[1] = vec4f(moved - pressed, 0.0);
    return pressed;
  }
  // The mirror on the water draws every tree as its impostor, whole.
  if (frame.lightCount.z > 0.5) {
    fade = 1.0;
  }
  let crown = length(c0);
  var right = vec3f(1.0, 0.0, 0.0);
  let level = vec2f(toward.z, -toward.x);
  if (dot(level, level) > 1e-12) {
    let r = normalize(level);
    right = vec3f(r.x, 0.0, r.y);
  }
  let up = cross(toward, right);
  // The outline of the upright cylinder that holds the tree. A near one, seen in perspective, spreads
  // past its outline on the plane through its middle.
  let reach = custom.x * max(crown, height);
  let near = reach / max(distance(middle, frame.cameraPos.xyz), reach * 1.2);
  let spread = 1.03 / sqrt(1.0 - near * near);
  let wide = custom.z * crown * spread;
  let tall = (custom.z * crown * length(up.xz) + custom.w * height * abs(up.y)) * spread;
  var corner = middle + right * (vertexUv.x * wide) + up * (vertexUv.y * tall);
  // Into unit space: the model's 3×3 is a turn and a scale, so its inverse is its transpose over the scales squared.
  let inverse = vec3f(1.0 / (crown * crown), 1.0 / (height * height), 1.0 / dot(c2, c2));
  let offset = corner - middle;
  vertexExtra[0] = vec4f(vec3f(dot(c0, offset), dot(c1, offset), dot(c2, offset)) * inverse, fade);
  vertexExtra[1] = vec4f(viewCell(vec3f(dot(c0, toward), dot(c1, toward), dot(c2, toward)) * inverse), 0.0, 0.0);
  vertexExtra[2] = vec4f(c0.x, c0.z, height, crown);
  // The card sways as the solid does, sheared by how high each corner is, so the fade between them holds.
  return corner + sway(corner, foot, height, dot(c1, corner - foot) / (height * height), 0.0);
}
`

/** Summer's colours: nothing lies on the leaves. */
const SUMMER_WGSL = /* wgsl */ `
fn under(color: vec3f, s: Surface) -> vec3f {
  return color;
}
`

/**
 * Winter's: faded needles and leaves, and snow on what faces the sky. The noise, in world space so no
 * two trees wear the same pattern, breaks the snow's edge into clumps, leaving crowns half covered.
 */
const WINTER_WGSL = /* wgsl */ `
fn under(color: vec3f, s: Surface) -> vec3f {
  let grey = dot(color, vec3f(0.3, 0.59, 0.11));
  let faded = mix(color, vec3f(grey), 0.6) * 0.7;
  let p = s.world * vec3f(2.3, 3.1, 2.3);
  let clumps = sin(p.x + sin(p.y * 1.3)) * sin(p.z + sin(p.y * 0.9)) * 0.5 + sin(p.y * 2.1 + p.x * 1.7) * 0.2;
  let cover = smoothstep(-0.12, 0.1, s.normal.y + clumps * 0.4);
  return mix(faded, vec3f(0.94, 0.96, 0.99), cover);
}
`

/** A kind's solid, and the levels that take over from it with the distances they do. */
interface Shapes {
  full: Geometry
  levels: DetailLevel[]
}

let shared: { geometries: Map<number, Shapes>; frames: Map<number, ImpostorFrame>; atlas: { albedo: Texture; normalDepth: Texture }; material: ShaderMaterial; standalone: ShaderMaterial } | undefined
/** The quality's `treeDetail`, for the fade's distances. */
let treeDetail = 1

function foliage(atlas: { albedo: Texture; normalDepth: Texture }, fadeFrom: number, fadeBand: number, sways = true) {
  return new ShaderMaterial({
    wgsl: FOLIAGE_WGSL + (winter ? WINTER_WGSL : SUMMER_WGSL),
    vertex: IMPOSTOR_WGSL,
    vertexColors: true,
    uniforms: { fadeFrom, fadeBand, wind: [Math.cos(WIND_HEADING), Math.sin(WIND_HEADING), sways ? TREE_SWAY : 0, 0] },
    textures: { impostorAlbedo: atlas.albedo, impostorNormalDepth: atlas.normalDepth },
    // Impostors stack many deep in a forest, each pixel three pictures' reads: settle what is in front
    // first, then shade only that.
    depthPrepass: true,
    // The fade and the impostor's outline are counted out in samples.
    coverage: true,
  })
}

function prototypes() {
  if (!shared) {
    const geometries = new Map<number, Shapes>()
    const frames = new Map<number, ImpostorFrame>()
    const pixels = emptyAtlas()
    for (const kind of KIND_TILES) {
      const { full, impostor, coarse, shadow, frame } = treeGeometries(kind)
      frames.set(kind, frame)
      bakeImpostor(full, frame, KIND_TILES.indexOf(kind), pixels)
      // The impostor is turned in the vertex shader, which the shadow pass does not run: it casts from solids.
      const levels: DetailLevel[] = [
        { geometry: impostor, from: TREE_LEVELS_FROM[0]!, fade: TREE_FADE, shadow: coarse },
        { geometry: impostor, from: TREE_LEVELS_FROM[1]!, shadow },
      ]
      geometries.set(kind, { full, levels })
    }
    // Nine megabytes each, baked once: only the GPU keeps them (`rebakeTreeImpostors` bakes them again for a new one).
    const texture = (data: Uint8Array, srgb: boolean) =>
      new Texture({ data, width: IMPOSTOR_ATLAS_PX, height: IMPOSTOR_ATLAS_PX }, { srgb, wrap: 'clamp', mipmaps: true, uploadOnce: true })
    const atlas = { albedo: texture(pixels.albedo, true), normalDepth: texture(pixels.normalDepth, false) }
    shared = {
      geometries,
      frames,
      atlas,
      material: foliage(atlas, TREE_LEVELS_FROM[0]! * treeDetail, TREE_FADE * treeDetail),
      // A felled tree is a mesh of its own, drawn as a solid however far off.
      // A felled tree does not sway as it falls or lies.
      standalone: foliage(atlas, 1e9, 1, false),
    }
  }
  return shared
}

/** Moves the fade with the quality's `treeDetail`, which the renderer scales the levels by. */
/**
 * Bakes the trees' impostors again, for a new graphics device: the atlas's pixels were let go once
 * the old one had them.
 */
export function rebakeTreeImpostors() {
  if (!shared) return
  const pixels = emptyAtlas()
  for (const kind of KIND_TILES) bakeImpostor(shared.geometries.get(kind)!.full, shared.frames.get(kind)!, KIND_TILES.indexOf(kind), pixels)
  for (const [texture, data] of [[shared.atlas.albedo, pixels.albedo], [shared.atlas.normalDepth, pixels.normalDepth]] as const) {
    texture.source = { data, width: IMPOSTOR_ATLAS_PX, height: IMPOSTOR_ATLAS_PX }
    texture.needsUpdate()
  }
}

/** Which way the wind blows, a unit \`[x, z]\`: the trees sway along it. */
export function setTreeWind(direction: readonly [number, number]) {
  if (!shared) return
  const wind = shared.material.uniforms.wind as number[]
  wind[0] = direction[0]
  wind[1] = direction[1]
  shared.material.needsUpdate()
}

export function setTreeDetail(detail: number) {
  treeDetail = detail
  if (!shared) return
  shared.material.uniforms.fadeFrom = TREE_LEVELS_FROM[0]! * detail
  shared.material.uniforms.fadeBand = TREE_FADE * detail
  shared.material.needsUpdate()
}

/** A turn about the vertical, from the position, so neighbours of one kind do not all look alike. */
function spin(tree: Tree): number {
  return (Math.abs(Math.sin(tree.x * 12.9898 + tree.z * 78.233)) * 43758.5453) % (Math.PI * 2)
}

/**
 * One standalone mesh of `tree`, in a pivot at its foot, for a tree that is no longer an
 * instance: the pivot is where it turns as it falls. Geometry and material are the shared ones.
 */
export function standaloneTree(tree: Tree): { pivot: Group; mesh: Mesh } {
  const { geometries, standalone } = prototypes()
  const mesh = new Mesh((geometries.get(tree.kind) ?? geometries.get(TREE_BIRCH)!).full, standalone)
  mesh.quaternion.setFromAxisAngle(0, 1, 0, spin(tree))
  mesh.scale.set(tree.crown, tree.height, tree.crown)
  mesh.castShadow = true
  mesh.userData.sharedGeometry = true
  const pivot = new Group()
  pivot.position.set(tree.x, tree.base, tree.z)
  pivot.add(mesh)
  return { pivot, mesh }
}

/**
 * Takes tree number `index` of its tile (the `Destroyed` index) out of the tile's instanced
 * meshes, by shrinking it to nothing. False if it is not there, or already gone.
 */
export function removeTreeInstance(group: Node, index: number): boolean {
  for (const child of group.children) {
    const slots = child.userData.treeSlots as Map<number, number> | undefined
    const slot = slots?.get(index)
    if (slot === undefined || !(child instanceof InstancedMesh)) continue
    slots!.delete(index)
    // Scaled to nothing where it stood, so the mesh's bounds do not stretch to the origin.
    const at = child.matrices
    child.setMatrixAt(slot, mat4.compose(mat4.create(), at[slot * 16 + 12]!, at[slot * 16 + 13]!, at[slot * 16 + 14]!, 0, 0, 0, 1, 0, 0, 0))
    child.needsUpdate()
    return true
  }
  return false
}

/**
 * One tile's trees: one instanced mesh per archetype present, so at most ten draw calls a level of
 * detail however many trees there are. The geometries and material are shared by all tiles (`disposeTile` leaves
 * them), only the instance buffers belong to the tile.
 */
export function buildTrees(trees: readonly Tree[], shore: Shore | null = null): InstancedMesh[] {
  if (trees.length === 0) return []
  const { geometries, material } = prototypes()
  const byKind = new Map<number, { tree: Tree; index: number }[]>()
  const rocks: { tree: Tree; index: number }[] = []
  trees.forEach((tree, index) => {
    if (isRock(tree.kind)) return void rocks.push({ tree, index })
    const list = byKind.get(tree.kind) ?? []
    list.push({ tree, index })
    byKind.set(tree.kind, list)
  })
  const matrix = mat4.create()
  const meshes: InstancedMesh[] = buildRocks(trees, rocks, shore)
  for (const [kind, list] of byKind) {
    const shapes = geometries.get(kind) ?? geometries.get(TREE_BIRCH)!
    const mesh = new InstancedMesh(shapes.full, material, list.length)
    // Each tree is culled on its own and drawn in the shape its own distance picks.
    mesh.levels = shapes.levels
    mesh.shadowLevel = TREE_SHADOW_LEVEL
    // Which instance is which tree of the tile, so one can be felled.
    const slots = new Map<number, number>()
    mesh.userData.treeSlots = slots
    list.forEach(({ tree, index }, i) => {
      slots.set(index, i)
      const half = spin(tree) / 2
      mesh.setMatrixAt(i, mat4.compose(matrix, tree.x, tree.base, tree.z, 0, Math.sin(half), 0, Math.cos(half), tree.crown, tree.height, tree.crown))
    })
    mesh.needsUpdate()
    mesh.castShadow = true
    mesh.receiveShadow = false
    mesh.userData.sharedGeometry = true
    meshes.push(mesh)
  }
  return meshes
}
