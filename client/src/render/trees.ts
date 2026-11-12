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
import { IMPOSTOR_ATLAS_PX, IMPOSTOR_FRAMES, IMPOSTOR_FRAME_PX, IMPOSTOR_MARGIN, IMPOSTOR_TILES, bakeImpostor, emptyAtlas, type ImpostorFrame } from './impostors'
import { buildRocks, isRock } from './rocks'
import type { Shore } from './shore'
import { winter } from './season'

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
  [TREE_BUSH]: [
    { shape: 'ellipsoid', color: 0x3f8f45, y0: 0.4, rx: 1, ry: 0.45 },
    { shape: 'ellipsoid', color: 0x4fa352, y0: 0.62, rx: 0.65, ry: 0.35, x: 0.15, z: -0.1 },
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
export const TREE_LEVELS_FROM = [40, 130]
/** Metres before the impostor takes over in which the solid dissolves into it, pixel by pixel. */
export const TREE_FADE = 12
/** A tree's levels: the solid, and the impostor twice over. */
export const TREE_LEVELS = TREE_LEVELS_FROM.length + 1
/** The finest level a tree's shadow is cast from: trees take no shadow, so the two are never compared. */
const TREE_SHADOW_LEVEL = 1

/** How finely a tree is built: as seen near, for a middling tree's shadow, or for a far tree's. */
type Detail = 'full' | 'coarse' | 'shadow'

/** Sides of a cone and of a trunk at each detail. Only the full shape has its ends closed. */
const CONE_SIDES: Record<Detail, number> = { full: 12, coarse: 5, shadow: 4 }
const TRUNK_SIDES: Record<Detail, number> = { full: 6, coarse: 4, shadow: 3 }
/**
 * How much wider a coarse crown is made, so its outline covers about what the round one did: an
 * icosahedron's faces and an octahedron's lie well inside the sphere their corners are on.
 */
const CROWN_SWELL: Record<Detail, number> = { full: 1, coarse: 1.08, shadow: 1.2 }

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
    // Further off 20 triangles, and in a far tree's shadow 8.
    const swell = CROWN_SWELL[detail]
    const ball = detail === 'shadow' ? octahedronGeometry() : icosphereGeometry(1, detail === 'full' ? 1 : 0)
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
 * The impostor's quad: `uv` is the corner (-1 to 1 across and up), and `custom` the half width of its
 * pictures (unit space), the kind's tile in the atlas, and the radius and half height of the upright
 * cylinder about the middle that holds the tree, which the quad is fitted to (w, never 0, also says it
 * is an impostor). A quad any bigger is pixels the shader works on only to throw away.
 */
function impostorGeometry(frame: ImpostorFrame, tile: number, solid: Geometry): Geometry {
  const [cx, cy, cz] = frame.center
  let radius = 0, half = 0
  for (let i = 0; i < solid.vertexCount; i++) {
    const p = solid.positions
    radius = Math.max(radius, Math.hypot(p[i * 3]! - cx, p[i * 3 + 2]! - cz))
    half = Math.max(half, Math.abs(p[i * 3 + 1]! - cy))
  }
  const positions: number[] = []
  const uvs: number[] = []
  const custom: number[] = []
  for (const [u, v] of [[-1, -1], [1, -1], [1, 1], [-1, 1]] as const) {
    positions.push(...frame.center)
    uvs.push(u, v)
    custom.push(frame.radius * IMPOSTOR_MARGIN, tile, radius, half)
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

/**
 * The shapes of tree `kind`. The solid's bounds are widened to hold the swollen shadow crowns too,
 * since every level is culled by them.
 */
export function treeGeometries(kind: number): TreeShapes {
  const parts = PROTOTYPES[kind] ?? PROTOTYPES[TREE_BIRCH]!
  const tile = Math.max(0, KIND_TILES.indexOf(PROTOTYPES[kind] ? kind : TREE_BIRCH))
  const shape = (detail: Detail) => Geometry.merge(partsAt(parts, detail).map((part) => partGeometry(part, detail)))
  const full = shape('full')
  const coarse = shape('coarse')
  const shadow = shape('shadow')
  const bounds = full.bounds
  const frame: ImpostorFrame = { center: [bounds.x, bounds.y, bounds.z], radius: bounds.radius }
  for (const other of [coarse, shadow]) {
    const b = other.bounds
    bounds.radius = Math.max(bounds.radius, Math.hypot(b.x - bounds.x, b.y - bounds.y, b.z - bounds.z) + b.radius)
  }
  full.custom = new Float32Array(full.vertexCount * 4)
  for (let i = 0; i < full.vertexCount; i++) full.custom.set([bounds.x, bounds.y, bounds.z, 0], i * 4)
  return { full, impostor: impostorGeometry(frame, tile, full), coarse, shadow, frame }
}

/** Metres from the eye inside which foliage dissolves, so a view from inside a crown stays clear. */
const NEAR_FADE = 2.5

/**
 * The foliage's own shader: the cel look of the standard one (four bands, sun and hemisphere), and a 4×4
 * ordered dither that removes pixels within `NEAR_FADE` of the camera and fades a tree from its solid to
 * its impostor. It is a material of its own so the shared mesh shader, and every other material, stays
 * as it was. Trees take no shadow.
 *
 * The fade: `extra[0].w` is how far into the band the tree is (0 to 1, the same for both, worked out from
 * the middle of its bounds), and of the 16 places of the dither the solid leaves that share and the
 * impostor draws only that share, so each pixel is one or the other.
 *
 * The impostor: `extra[0].xyz` is the pixel's place in the tree's unit space, from the middle of its
 * bounds, and `extra[1].xyz` the direction to the camera from there. The three pictures nearest that
 * direction are each met where the pixel's line of sight crosses the picture's plane, and blended by
 * how near they are (a dither picking one of them a pixel was cheaper, but the pictures are far enough
 * apart that it speckled); the surface found is lit as the solid's would be. The quad keeps its own depth: writing the surface's cost more than a millisecond in a
 * forest, as it stops the GPU skipping hidden pixels.
 */
const FOLIAGE_WGSL = /* wgsl */ `
const BANDS_AT = array<f32, 4>(0.376, 0.588, 0.804, 1.0);
const BAYER_AT = array<u32, 16>(0u, 8u, 2u, 10u, 12u, 4u, 14u, 6u, 3u, 11u, 1u, 9u, 15u, 7u, 13u, 5u);
const FRAMES = ${IMPOSTOR_FRAMES.toFixed(1)};
const FRAME_PX = ${IMPOSTOR_FRAME_PX.toFixed(1)};
const ATLAS_PX = ${IMPOSTOR_ATLAS_PX.toFixed(1)};
const TILES = ${IMPOSTOR_TILES.toFixed(1)};

/** The dither's place (0 to 15) at the pixel under \`world\`. */
fn ditherAt(world: vec3f) -> u32 {
  let clip = frame.viewProj * vec4f(world, 1.0);
  let pixel = vec2u((clip.xy / clip.w * 0.5 + 0.5) * frame.viewport.xy);
  // Copied to a variable first: WGSL only indexes a constant array with a constant.
  var bayer = BAYER_AT;
  return bayer[(pixel.y & 3u) * 4u + (pixel.x & 3u)];
}

/** The direction picture \`cell\` of a kind's grid was taken from (\`decodeView\` in impostors.ts). */
fn frameView(cell: vec2f) -> vec3f {
  let ab = cell / (FRAMES - 1.0) * 2.0 - 1.0;
  let p = vec2f(ab.x + ab.y, ab.x - ab.y) * 0.5;
  return normalize(vec3f(p.x, max(0.0, 1.0 - abs(p.x) - abs(p.y)), p.y));
}

/** One picture's colour and coverage, normal and surface point (unit space, from the middle), weighted. */
struct Picture {
  albedo: vec4f,
  normal: vec3f,
  hit: vec3f,
}

fn picture(cell: vec2f, weight: f32, q: vec3f, sight: vec3f, extent: f32, tile: f32, lod: f32) -> Picture {
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
  let local = vec2f(dot(onPlane, right), dot(onPlane, up)) / extent;
  if (weight <= 0.0 || abs(local.x) > 1.0 || abs(local.y) > 1.0) {
    return out;
  }
  let inside = clamp(vec2f(local.x, -local.y) * 0.5 + 0.5, vec2f(0.5 / FRAME_PX), vec2f(1.0 - 0.5 / FRAME_PX));
  let tileAt = vec2f(tile % TILES, floor(tile / TILES)) * FRAMES;
  let uv = (tileAt + cell + inside) * (FRAME_PX / ATLAS_PX);
  let albedo = textureSampleLevel(impostorAlbedo, materialSampler, uv, lod);
  // Well off the tree: nothing more to read. Most of the quad is.
  if (albedo.a <= 0.0) {
    return out;
  }
  let nd = textureSampleLevel(impostorNormalDepth, materialSampler, uv, lod);
  let share = weight * albedo.a;
  out.albedo = vec4f(albedo.rgb * share, share);
  out.normal = (nd.rgb * 2.0 - 1.0) * share;
  out.hit = (onPlane + v * ((nd.a * 2.0 - 1.0) * extent)) * share;
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
  let dither = (f32(ditherAt(given.world)) + 0.5) / 16.0;
  let impostor = s.custom.w > 0.0;
  if (impostor) {
    if (dither >= fade) {
      discard;
    }
    let q = s.extra[0].xyz;
    let e = s.extra[2];
    // The pictures are picked by the direction to the camera from the tree's middle, the same over the
    // whole tree, but looked up along this pixel's own line of sight: a near tree is seen in perspective.
    let toward = normalize(s.extra[1].xyz);
    let sight = normalize(worldToTree(frame.cameraPos.xyz - given.world, e));
    let extent = s.custom.x;
    // The three pictures around the direction to the camera, on the grid of \`encodeView\`.
    let sum = abs(toward.x) + max(toward.y, 0.0) + abs(toward.z);
    let p = toward.xz / max(sum, 1e-6);
    let g = (vec2f(p.x + p.y, p.x - p.y) * 0.5 + 0.5) * (FRAMES - 1.0);
    let cell = clamp(floor(g), vec2f(0.0), vec2f(FRAMES - 2.0));
    let f = g - cell;
    var a: Picture;
    var b: Picture;
    var c: Picture;
    if (f.x + f.y < 1.0) {
      a = picture(cell, 1.0 - f.x - f.y, q, sight, extent, s.custom.y, lod);
      b = picture(cell + vec2f(1.0, 0.0), f.x, q, sight, extent, s.custom.y, lod);
      c = picture(cell + vec2f(0.0, 1.0), f.y, q, sight, extent, s.custom.y, lod);
    } else {
      a = picture(cell + vec2f(1.0, 1.0), f.x + f.y - 1.0, q, sight, extent, s.custom.y, lod);
      b = picture(cell + vec2f(0.0, 1.0), 1.0 - f.x, q, sight, extent, s.custom.y, lod);
      c = picture(cell + vec2f(1.0, 0.0), 1.0 - f.y, q, sight, extent, s.custom.y, lod);
    }
    let cover = a.albedo.a + b.albedo.a + c.albedo.a;
    // The coverage is a distance to the silhouette, a half on it.
    if (cover < 0.5) {
      discard;
    }
    let n = (a.normal + b.normal + c.normal) / cover;
    s.normal = normalize(treeToWorld(n / vec3f(e.w * e.w, e.z * e.z, e.w * e.w), e));
    s.world = given.world + treeToWorld((a.hit + b.hit + c.hit) / cover - q, e);
    s.color = vec4f(s.color.rgb * (a.albedo.rgb + b.albedo.rgb + c.albedo.rgb) / cover, s.color.a);
  } else {
    if (dither < fade) {
      discard;
    }
    // Where a fading solid really is: its corners were pressed toward the impostor's card.
    s.world = given.world + s.extra[1].xyz;
  }
  let eye = distance(s.world, frame.cameraPos.xyz) / ${NEAR_FADE.toFixed(1)};
  if (eye < dither) {
    discard;
  }
  let band = u32(clamp(dot(s.normal, frame.sunDir.xyz) * 0.5 + 0.5, 0.0, 0.999) * 4.0);
  let hemi = mix(frame.hemiGround.rgb, frame.hemiSky.rgb, 0.5 * s.normal.y + 0.5);
  var bands = BANDS_AT;
  let lit = frame.sunColor.rgb * bands[band] + hemi + pointLight(s.world, s.normal);
  return vec4f(under(s.color.rgb, s) / PI * lit, s.color.a);
}
`

/**
 * Opens an impostor's quad to face the camera and hands the fragment what it needs to find the tree
 * in its pictures; a solid stays where it is. Both work out how far into the fade they are.
 */
const IMPOSTOR_WGSL = /* wgsl */ `
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
  vertexExtra[0] = vec4f(0.0, 0.0, 0.0, fade);
  let toward = normalize(frame.cameraPos.xyz - middle);
  if (!impostor) {
    // Fading, the solid's pixels lie among the impostor's, which are on a flat card through the tree's
    // middle: the ink would draw a line at every step between the two. So the solid is pressed toward
    // that card, each corner slid along its own line of sight (it stays where it is on the screen), to a
    // twentieth of its depth either side, over the third of a band before the fade starts. Its light is
    // worked out where it really is.
    let press = clamp(into / (band / 3.0) + 1.0, 0.0, 1.0);
    if (press <= 0.0) {
      return world;
    }
    let sight = world - frame.cameraPos.xyz;
    let onCard = dot(middle - frame.cameraPos.xyz, toward) / dot(sight, toward);
    let pressed = frame.cameraPos.xyz + sight * mix(1.0, onCard, press * 0.95);
    vertexExtra[1] = vec4f(world - pressed, 0.0);
    return pressed;
  }
  // The mirror on the water draws every tree as its impostor, whole.
  if (frame.lightCount.z > 0.5) {
    fade = 1.0;
  }
  let crown = length(c0);
  let height = length(c1);
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
  let corner = middle + right * (vertexUv.x * wide) + up * (vertexUv.y * tall);
  // Into unit space: the model's 3×3 is a turn and a scale, so its inverse is its transpose over the scales squared.
  let inverse = vec3f(1.0 / (crown * crown), 1.0 / (height * height), 1.0 / dot(c2, c2));
  let offset = corner - middle;
  vertexExtra[0] = vec4f(vec3f(dot(c0, offset), dot(c1, offset), dot(c2, offset)) * inverse, fade);
  vertexExtra[1] = vec4f(vec3f(dot(c0, toward), dot(c1, toward), dot(c2, toward)) * inverse, 0.0);
  vertexExtra[2] = vec4f(c0.x, c0.z, height, crown);
  return corner;
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

let shared: { geometries: Map<number, Shapes>; material: ShaderMaterial; standalone: ShaderMaterial } | undefined
/** The quality's `treeDetail`, for the fade's distances. */
let treeDetail = 1

function foliage(atlas: { albedo: Texture; normalDepth: Texture }, fadeFrom: number, fadeBand: number) {
  return new ShaderMaterial({
    wgsl: FOLIAGE_WGSL + (winter ? WINTER_WGSL : SUMMER_WGSL),
    vertex: IMPOSTOR_WGSL,
    vertexColors: true,
    uniforms: { fadeFrom, fadeBand },
    textures: { impostorAlbedo: atlas.albedo, impostorNormalDepth: atlas.normalDepth },
  })
}

function prototypes() {
  if (!shared) {
    const geometries = new Map<number, Shapes>()
    const pixels = emptyAtlas()
    for (const kind of KIND_TILES) {
      const { full, impostor, coarse, shadow, frame } = treeGeometries(kind)
      bakeImpostor(full, frame, KIND_TILES.indexOf(kind), pixels)
      // The impostor is turned in the vertex shader, which the shadow pass does not run: it casts from solids.
      const levels: DetailLevel[] = [
        { geometry: impostor, from: TREE_LEVELS_FROM[0]!, fade: TREE_FADE, shadow: coarse },
        { geometry: impostor, from: TREE_LEVELS_FROM[1]!, shadow },
      ]
      geometries.set(kind, { full, levels })
    }
    const texture = (data: Uint8Array, srgb: boolean) =>
      new Texture({ data, width: IMPOSTOR_ATLAS_PX, height: IMPOSTOR_ATLAS_PX }, { srgb, wrap: 'clamp', mipmaps: true })
    const atlas = { albedo: texture(pixels.albedo, true), normalDepth: texture(pixels.normalDepth, false) }
    shared = {
      geometries,
      material: foliage(atlas, TREE_LEVELS_FROM[0]! * treeDetail, TREE_FADE * treeDetail),
      // A felled tree is a mesh of its own, drawn as a solid however far off.
      standalone: foliage(atlas, 1e9, 1),
    }
  }
  return shared
}

/** Moves the fade with the quality's `treeDetail`, which the renderer scales the levels by. */
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
