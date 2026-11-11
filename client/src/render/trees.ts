import { Color, Geometry, Group, InstancedMesh, Mesh, ShaderMaterial, type DetailLevel, type Material, type Node, cylinderGeometry, icosphereGeometry, mat4 } from '@ragnarok/engine'
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

interface Part {
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
  [TREE_SPRUCE]: [
    { shape: 'trunk', color: TRUNK_BARK, y0: 0, y1: 0.2, rx: 0.09 },
    { shape: 'cone', color: 0x2f6b4a, y0: 0.12, y1: 0.5, rx: 1 },
    { shape: 'cone', color: 0x2f6b4a, y0: 0.4, y1: 0.75, rx: 0.75 },
    { shape: 'cone', color: 0x35764f, y0: 0.65, y1: 1, rx: 0.5 },
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
 * Metres from the camera at which a tree changes to a coarser shape: first to a few-sided one, then to
 * billboards. A tree is drawn as built here up to the first, and a forest is mostly far trees: at full
 * detail they were nine tenths of the frame's triangles. The quality's `treeDetail` scales these.
 */
export const TREE_LEVELS_FROM = [70, 130]
/** A tree's shapes: the full one, the few-sided one and the billboards. */
export const TREE_LEVELS = TREE_LEVELS_FROM.length + 1
/** The finest level a tree's shadow is cast from: trees take no shadow, so the two are never compared. */
const TREE_SHADOW_LEVEL = 1

/** How finely a tree is built: as seen near, as seen from middling far, or for a far tree's shadow. */
type Detail = 'full' | 'coarse' | 'shadow'

/** Sides of a cone and of a trunk at each detail. Only the full shape has its ends closed. */
const CONE_SIDES: Record<Detail, number> = { full: 7, coarse: 5, shadow: 4 }
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

/** What a billboard's quad is drawn as, its `custom.z`; 0 is a part that is not a billboard. */
const BILLBOARD_CROWN = 1
const BILLBOARD_CONE = 2
const BILLBOARD_TRUNK = 3
/**
 * How far a billboard is brought toward the camera, in crown radii, so the parts of one tree lie in
 * front of each other as they would in the round: a crown by most of its own radius, in front of the
 * trunk it hides, and each higher tier of a spruce a little in front of the one below.
 */
const CROWN_PUSH = 0.8
const CONE_PUSH = 0.4
const CONE_TIER_PUSH = 0.05

/**
 * A far tree: one quad a part, all four corners at the part's centre. The foliage shader's `displace`
 * opens each quad to face the camera: `uv` is the corner (-1 to 1 across and up), and `custom` is the
 * half width (crown radii) and half height (tree heights), the `BILLBOARD_*` kind and the push toward
 * the camera. The fragment shader cuts out an ellipse, a triangle or a bar and lights it as if round.
 */
function billboardGeometry(parts: readonly Part[]): Geometry {
  const positions: number[] = []
  const normals: number[] = []
  const uvs: number[] = []
  const custom: number[] = []
  const colors: number[] = []
  const indices: number[] = []
  let tier = 0
  for (const part of parts) {
    const crown = part.shape === 'ellipsoid'
    const y = crown ? part.y0 : (part.y0 + part.y1!) / 2
    const half = crown ? part.ry! : (part.y1! - part.y0) / 2
    const kind = crown ? BILLBOARD_CROWN : part.shape === 'cone' ? BILLBOARD_CONE : BILLBOARD_TRUNK
    const push = crown ? part.rx! * CROWN_PUSH : part.shape === 'cone' ? CONE_PUSH + CONE_TIER_PUSH * tier++ : 0
    const color = Color.hex(part.color)
    const first = positions.length / 3
    for (const [u, v] of [[-1, -1], [1, -1], [1, 1], [-1, 1]] as const) {
      positions.push(crown ? (part.x ?? 0) : 0, y, crown ? (part.z ?? 0) : 0)
      normals.push(0, 1, 0)
      uvs.push(u, v)
      custom.push(part.rx!, half, kind, push)
      colors.push(color.r, color.g, color.b)
    }
    indices.push(first, first + 1, first + 2, first, first + 2, first + 3)
  }
  return new Geometry({
    positions: new Float32Array(positions),
    normals: new Float32Array(normals),
    uvs: new Float32Array(uvs),
    custom: new Float32Array(custom),
    colors: new Float32Array(colors),
    indices: new Uint16Array(indices),
  })
}

/**
 * The shapes of tree `kind`: `levels`, finest first (`TREE_LEVELS` of them, the last its billboards),
 * and `shadow`, what a tree drawn as billboards casts its shadow with. The full shape's bounds are
 * widened to hold the swollen coarse crowns too, since every level is culled by them.
 */
export function treeGeometries(kind: number): { levels: Geometry[]; shadow: Geometry } {
  const parts = PROTOTYPES[kind] ?? PROTOTYPES[TREE_BIRCH]!
  const shape = (detail: Detail) => Geometry.merge(partsAt(parts, detail).map((part) => partGeometry(part, detail)))
  const full = shape('full')
  const coarse = shape('coarse')
  const shadow = shape('shadow')
  const bounds = full.bounds
  for (const other of [coarse, shadow]) {
    const b = other.bounds
    bounds.radius = Math.max(bounds.radius, Math.hypot(b.x - bounds.x, b.y - bounds.y, b.z - bounds.z) + b.radius)
  }
  return { levels: [full, coarse, billboardGeometry(parts)], shadow }
}

/** Metres from the eye inside which foliage dissolves, so a view from inside a crown stays clear. */
const NEAR_FADE = 2.5

/**
 * The foliage's own shader: the cel look of the standard one (four bands, sun and hemisphere), plus
 * a 4×4 ordered dither that removes pixels within `NEAR_FADE` of the camera. It is a material of its
 * own so the shared mesh shader, and every other material, stays as it was. Trees take no shadow.
 */
const FOLIAGE_WGSL = /* wgsl */ `
const BANDS_AT = array<f32, 4>(0.376, 0.588, 0.804, 1.0);
const BAYER_AT = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);

/** The round surface under a billboard's pixel, facing the viewer; it cuts away what is outside the part. */
fn billboardNormal(s: Surface) -> vec3f {
  let u = s.uv.x;
  let v = s.uv.y;
  let toward = normalize(frame.cameraPos.xyz - s.world);
  let right = billboardRight(s.world);
  // Toward the camera, level with the ground.
  let level = vec3f(-right.z, 0.0, right.x);
  if (s.custom.z < 1.5) {
    let r2 = u * u + v * v;
    if (r2 > 1.0) {
      discard;
    }
    return normalize(right * u + cross(toward, right) * v + toward * sqrt(1.0 - r2));
  }
  if (s.custom.z < 2.5) {
    // A cone narrows from its base (v -1) to its tip (v 1), and its sides lean back toward the sky.
    let half = (1.0 - v) * 0.5;
    if (abs(u) > half) {
      discard;
    }
    let across = u / max(half, 0.001);
    return normalize(right * across + level * sqrt(max(0.0, 1.0 - across * across)) + vec3f(0.0, 0.5, 0.0));
  }
  return normalize(right * u + level * sqrt(max(0.0, 1.0 - u * u)));
}

fn surface(given: Surface) -> vec4f {
  var s = given;
  if (s.custom.z > 0.5) {
    s.normal = billboardNormal(s);
  }
  let eye = distance(s.world, frame.cameraPos.xyz) / ${NEAR_FADE.toFixed(1)};
  if (eye < 1.0) {
    let clip = frame.viewProj * vec4f(s.world, 1.0);
    let pixel = vec2u((clip.xy / clip.w * 0.5 + 0.5) * frame.viewport.xy);
    let at = (pixel.y & 3u) * 4u + (pixel.x & 3u);
    // Copied to a variable first: WGSL only indexes a constant array with a constant.
    var bayer = BAYER_AT;
    if (eye < (bayer[at] + 0.5) / 16.0) {
      discard;
    }
  }
  let band = u32(clamp(dot(s.normal, frame.sunDir.xyz) * 0.5 + 0.5, 0.0, 0.999) * 4.0);
  let hemi = mix(frame.hemiGround.rgb, frame.hemiSky.rgb, 0.5 * s.normal.y + 0.5);
  var bands = BANDS_AT;
  let lit = frame.sunColor.rgb * bands[band] + hemi + pointLight(s.world, s.normal);
  return vec4f(under(s.color.rgb, s) / PI * lit, s.color.a);
}
`

/**
 * Turns a far tree's quads (`billboardGeometry`) to the camera; any other vertex stays where it is.
 * Trunks and cones turn only about the vertical, so they stay upright. A crown faces the camera fully,
 * and grows from its side-on height to its width as the camera rises over it, as a flattened ball would.
 */
const BILLBOARD_WGSL = /* wgsl */ `
fn billboardRight(world: vec3f) -> vec3f {
  let to = frame.cameraPos.xyz - world;
  let side = vec2f(to.z, -to.x);
  let length2 = dot(side, side);
  if (length2 < 1e-8) {
    return vec3f(1.0, 0.0, 0.0);
  }
  let across = side * inverseSqrt(length2);
  return vec3f(across.x, 0.0, across.y);
}

fn displace(world: vec3f, custom: vec4f, color: vec4f) -> vec3f {
  if (custom.z < 0.5) {
    return world;
  }
  // A tree is scaled by its crown radius across and its height up.
  let crown = length(vertexModel[0].xyz);
  let wide = custom.x * crown;
  var tall = custom.y * length(vertexModel[1].xyz);
  let toward = normalize(frame.cameraPos.xyz - world);
  let right = billboardRight(world);
  var up = vec3f(0.0, 1.0, 0.0);
  if (custom.z < 1.5) {
    up = cross(toward, right);
    let rise = toward.y * toward.y;
    tall = sqrt(tall * tall * (1.0 - rise) + wide * wide * rise);
  }
  return world + right * (vertexUv.x * wide) + up * (vertexUv.y * tall) + toward * (custom.w * crown);
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

/** A kind's full shape, and its coarser ones with the distances they take over at. */
interface Shapes {
  full: Geometry
  levels: DetailLevel[]
}

let shared: { geometries: Map<number, Shapes>; material: Material } | undefined

function prototypes() {
  if (!shared) {
    const geometries = new Map<number, Shapes>()
    for (const kind of Object.keys(PROTOTYPES)) {
      const { levels: [full, ...coarser], shadow } = treeGeometries(Number(kind))
      // The billboards are turned in the vertex shader, which the shadow pass does not run.
      const levels = coarser.map((geometry, i): DetailLevel => ({ geometry, from: TREE_LEVELS_FROM[i]!, shadow: i === coarser.length - 1 ? shadow : undefined }))
      geometries.set(Number(kind), { full: full!, levels })
    }
    shared = {
      geometries,
      material: new ShaderMaterial({ wgsl: FOLIAGE_WGSL + (winter ? WINTER_WGSL : SUMMER_WGSL), vertex: BILLBOARD_WGSL, vertexColors: true }),
    }
  }
  return shared
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
  const { geometries, material } = prototypes()
  const mesh = new Mesh((geometries.get(tree.kind) ?? geometries.get(TREE_BIRCH)!).full, material)
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
