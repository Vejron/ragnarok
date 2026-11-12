import { Color, Geometry, Group, Mesh, ShaderMaterial, Texture } from '@ragnarok/engine'
import type { TileGround } from '@/sim/terrain'
import type { TileData } from '@/world'
import type { HeightSource } from './shade'
import { waterBuffers, type WaterBuffers } from './groundmesh'
import { GROUND_LIGHT_WGSL } from './groundlight'
import { WAVES_WGSL } from '@/sim/waves'
import { WAKES_WGSL } from './wakes'

/**
 * Water: lakes and rivers are a surface mesh per tile at the importer's water level, streams are
 * ribbons draped on the ground. Both share one toy-style shader: a tint that deepens with depth,
 * soft light bands drifting over the surface, a foam line along the shore, and a flow along a
 * stream's length. Depth is a vertex attribute (`level - ground`), not a depth-texture read, so the
 * look needs no extra pass. Lakes and rivers carry the waves the sim rides vehicles on (`sim/waves.ts`):
 * the vertices rise and fall with them on the server's clock, they light the surface, and their crests
 * break white. They die away towards the shore; streams and ice have none. On top, whatever moves
 * through the water carves a wake into it (`wakes.ts`).
 */

/** Shore foam reaches this deep, metres. */
const FOAM_DEPTH = 0.35
/** Along-stream speed of the flow, metres per second. */
const FLOW_SPEED = 1.4

/** Moves each vertex of a lake or river with the waves. Its functions serve the fragment too. */
const VERTEX = /* wgsl */ `
${WAVES_WGSL}

// How much of the waves this water has: none on a stream or on ice, less towards the shore.
fn waveShare(custom: vec4f) -> f32 {
  return select(waveShore(custom.x), 0.0, custom.w > 0.5) * params.wave.y;
}

${WAKES_WGSL}

// How much of the wakes this water shows: none on ice, less where it is too shallow to stir.
fn wakeShare(custom: vec4f) -> f32 {
  return smoothstep(0.05, 0.4, custom.x) * params.wave.y;
}

fn displace(world: vec3f, custom: vec4f, color: vec4f) -> vec3f {
  let lift = waves(world.xz, params.wave.x).x * waveShare(custom) + wake(world.xz).x * wakeShare(custom);
  return world + vec3f(0.0, lift, 0.0);
}
`

const WGSL = /* wgsl */ `
${GROUND_LIGHT_WGSL}

fn bands(p: vec2f, t: f32) -> f32 {
  let a = sin(p.x * 0.9 + p.y * 0.35 + t * 0.9);
  let b = sin(p.x * -0.4 + p.y * 1.1 - t * 0.7);
  let c = sin((p.x + p.y) * 0.25 + t * 0.4);
  return (a + b + 0.6 * c) / 2.6;
}

// A hash of a lattice point, 0 to 1.
fn hash2(p: vec2f) -> f32 {
  let q = fract(p * vec2f(0.1031, 0.1030));
  let r = q + dot(q, q.yx + 33.33);
  return fract((r.x + r.y) * r.x);
}

// Smooth value noise, -1 to 1, and its gradient: (value, d/dx, d/dy).
fn vnoise(p: vec2f) -> vec3f {
  let i = floor(p);
  let f = p - i;
  let u = f * f * (3.0 - 2.0 * f);
  let du = 6.0 * f * (1.0 - f);
  let a = hash2(i);
  let b = hash2(i + vec2f(1.0, 0.0));
  let c = hash2(i + vec2f(0.0, 1.0));
  let d = hash2(i + vec2f(1.0, 1.0));
  let k = a - b - c + d;
  let v = a + (b - a) * u.x + (c - a) * u.y + k * u.x * u.y;
  let g = du * vec2f(b - a + k * u.y, c - a + k * u.x);
  return vec3f(v * 2.0 - 1.0, g * 2.0);
}

// One octave of chop: noise \`scale\` cells a metre, turned by \`angle\` and drifting at \`drift\` m/s, its gradient in world metres.
fn octave(p: vec2f, t: f32, scale: f32, angle: f32, drift: vec2f) -> vec3f {
  let cs = vec2f(cos(angle), sin(angle));
  let turn = mat2x2f(cs.x, cs.y, -cs.y, cs.x);
  let n = vnoise(turn * (p + drift * t) * scale);
  return vec3f(n.x, (transpose(turn) * n.yz) * scale);
}

// The wind's chop over the swell: four octaves, each turned and drifting its own way so nothing lines
// up into a pattern. Height (about -2 to 2) and its slope per metre. \`fine\` and \`mid\` (0 to 1) fade the
// small ones far off, so distant water does not shimmer.
fn chop(p: vec2f, t: f32, fine: f32, mid: f32) -> vec3f {
  return octave(p, t, 0.11, 0.0, vec2f(0.9, 0.35))
       + 0.6 * octave(p, t, 0.26, 1.1, vec2f(1.1, -0.5))
       + mid * 0.35 * octave(p, t, 0.63, 2.3, vec2f(-0.4, 1.3))
       + fine * 0.2 * octave(p, t, 1.45, 3.7, vec2f(1.5, 0.8));
}

// Where the wind is gusting, 0 (a glassy slick) to 1 (ruffled dark water): broad patches, tens of metres
// to a couple of hundred across, that drift slowly, so water seen from above is never the same twice.
fn gusts(p: vec2f, t: f32) -> f32 {
  let a = vnoise(p * 0.009 + vec2f(t * 0.0045, t * 0.002)).x;
  let b = vnoise(p * 0.027 + vec2f(17.3 - t * 0.007, 4.1 + t * 0.005)).x;
  return clamp(0.55 + 0.6 * a + 0.35 * b, 0.0, 1.0);
}

// What the water mirrors: a horizon-to-zenith sky with a few soft clouds and the sun's halo.
fn skyAt(r: vec3f, t: f32) -> vec3f {
  let up = clamp(r.y, 0.0, 1.0);
  var c = mix(params.sky, params.sky * vec3f(0.55, 0.75, 1.05), pow(up, 0.55));
  let cp = r.xz / (r.y + 0.3) * 0.9 + vec2f(t * 0.03, 0.0);
  let cloud = smoothstep(0.5, 0.95, 0.5 + 0.3 * sin(cp.x * 1.3 + sin(cp.y * 1.1) * 1.6) + 0.25 * sin(cp.y * 2.1 + sin(cp.x * 1.7) * 1.2));
  c = mix(c, params.cloud, cloud * 0.4 * smoothstep(0.02, 0.3, r.y));
  let s = max(dot(r, frame.sunDir.xyz), 0.0);
  return c + params.sun * (pow(s, 6.0) * 0.12 + pow(s, 200.0) * 1.2);
}

fn caustics(p: vec2f, t: f32) -> f32 {
  let a = abs(sin(p.x * 1.3 + sin(p.y * 0.9 + t * 0.8) * 1.5));
  let b = abs(sin(p.y * 1.1 + sin(p.x * 1.2 - t * 0.6) * 1.5));
  return pow(1.0 - a * b, 6.0);
}

fn surface(s: Surface) -> vec4f {
  let time = frame.fog.w;
  let depth = s.custom.x;
  let flow = s.custom.yzw;
  let world = s.world.xz;
  let stream = flow.z > 0.5;
  var p = world * 0.7;
  if (stream) {
    p = vec2f(flow.x - time * ${FLOW_SPEED.toFixed(2)}, flow.y * 3.0) * 2.2;
  }
  let to = frame.cameraPos.xyz - s.world;
  let away = length(to);
  let fine = 1.0 - smoothstep(25.0, 110.0, away);
  let mid = 1.0 - smoothstep(80.0, 320.0, away);
  // Lakes and rivers: the wind's chop, stronger in the gusts. A stream keeps its flow's bands.
  let gust = select(gusts(world, time), 0.5, stream);
  let chopped = select(chop(world, time, fine, mid), vec3f(0.0), stream);
  let wave = select(clamp(chopped.x * 0.5, -1.0, 1.0) * mix(0.7, 1.15, gust), bands(p, 0.0), stream);
  let band = smoothstep(0.35, 0.5, wave) * 0.5 + smoothstep(0.6, 0.7, wave) * 0.5;

  let deep = smoothstep(0.0, 2.2, depth);
  var color = mix(params.shallow, params.deep, deep);

  let clear = 1.0 - smoothstep(0.0, 1.4, depth);
  let bedP = select(world * 0.9, p * 0.8, stream);
  let web = caustics(bedP, select(time * 0.8, time, stream));
  color = mix(color, params.bed, clear * 0.45);
  color += params.foam * web * clear * 0.35;
  color = mix(color, params.foam, band * 0.12);
  // The water's own colour and its foam are lit as the flat ground is, so a lake darkens with the shore at night.
  let ground = groundLight(s.world, vec3f(0.0, 1.0, 0.0), 1.0) / PI;
  color *= ground;

  // The chop's slope: rougher in a gust, nearly flat on a slick. A stream ripples along its flow.
  let e = 0.15;
  let h = bands(p, 0.0);
  let flowRipple = vec2f(bands(p + vec2f(e, 0.0), 0.0) - h, bands(p + vec2f(0.0, e), 0.0) - h) / e * 0.5;
  let grad = select(chopped.yz * mix(0.35, 1.1, gust), flowRipple, stream);
  // The real waves' slope, with the small swell's ripples over it.
  let share = waveShare(s.custom);
  let big = waves(world, params.wave.x) * share;
  // And what has passed through it: the wakes' rings and trenches, and their churn.
  let churn = wake(world) * wakeShare(s.custom);
  var n = normalize(vec3f(-grad.x * 0.45 - big.y - churn.y, 1.0, -grad.y * 0.45 - big.z - churn.z));
  if (!s.front) {
    n.y = -n.y;
  }
  let view = normalize(to);

  if (!s.front && params.ice.x < 0.5) {
    // From under the surface (\`n\` faces down, at the camera): the sky shows through a window overhead,
    // bent by the surface; further out than about 49° from straight up the surface mirrors the dark
    // water instead. The scene's mirror is only for eyes above the water.
    let up = refract(-view, n, 1.33);
    let r = select(vec3f(0.0, 1.0, 0.0), normalize(up), dot(up, up) > 1e-4);
    let window = smoothstep(0.6, 0.72, clamp(abs(dot(n, view)), 0.0, 1.0));
    let below = params.deep * ground * 0.45;
    let above = skyAt(vec3f(r.x, abs(r.y), r.z), time);
    let c = mix(below, above, window) + params.foam * band * 0.08 * ground;
    return vec4f(c, mix(0.95, 0.55, window));
  }

  // Schlick: little reflection looking straight down, nearly a mirror at a grazing angle.
  let ndv = clamp(abs(dot(n, view)), 0.0, 1.0);
  let fresnel = 0.03 + 0.97 * pow(1.0 - ndv, 5.0);
  let lit = floor(clamp(dot(n, frame.sunDir.xyz) * 0.5 + 0.5, 0.0, 1.0) * 3.0) / 3.0;
  color *= 0.9 + 0.2 * lit;
  // Ruffled water is a shade darker than a slick.
  color *= mix(1.05, 0.9, gust);
  var bounce = reflect(-view, n);
  bounce.y = abs(bounce.y);
  var mirror = skyAt(bounce, time);
  // The scene mirrored in the water's plane, drawn from the same view and flipped across (see the renderer).
  // Water at another height than the plane, or without a mirror, keeps the sky.
  let clip = frame.viewProj * vec4f(s.world, 1.0);
  let ndc = clip.xy / max(clip.w, 0.001);
  let ripple = vec2f(n.x, n.z) * 0.05 * fine;
  let ruv = clamp(vec2f(0.5 - ndc.x * 0.5, 0.5 - ndc.y * 0.5) + ripple, vec2f(0.0), vec2f(1.0));
  let seen = textureSampleLevel(reflection, materialSampler, ruv, 0.0);
  // Measured at the water's level under the waves, so a crest still mirrors the scene.
  let level = params.plane.y * (1.0 - smoothstep(0.2, 0.5, abs(s.world.y - big.x - churn.x - params.plane.x)));
  mirror = mix(mirror, seen.rgb, seen.a * level);
  color = mix(color, mirror, clamp((fresnel * 1.3 + 0.12) * mix(1.1, 0.8, gust), 0.0, 0.95));

  let glint = pow(max(dot(reflect(-frame.sunDir.xyz, n), view), 0.0), 90.0);
  let spark = smoothstep(0.35, 0.6, glint);
  color += params.sun * (spark * 0.8 + pow(max(dot(reflect(-frame.sunDir.xyz, n), view), 0.0), 24.0) * 0.12);

  let shore = ${FOAM_DEPTH.toFixed(2)} + 0.08 * sin(time * 1.3 + world.x * 0.6 + world.y * 0.45);
  let foam = 1.0 - smoothstep(shore * 0.55, shore, depth);
  let lacy = vnoise(world * 0.9 + vec2f(time * 0.6, -time * 0.4)).x * 0.65 + vnoise(world * 2.3 - vec2f(time * 0.9, time * 0.5)).x * 0.35;
  let lace = smoothstep(0.1, 0.55, lacy * 0.5 + 0.5);
  color = mix(color, params.foam * ground, foam * mix(0.55, 1.0, lace));
  // Whitecaps: the high, steep crests break, in ragged patches.
  let steep = length(big.yz);
  let cap = smoothstep(0.45, 0.9, big.x) * smoothstep(0.15, 0.4, steep) * smoothstep(0.2, 0.6, lace) * mix(0.5, 1.2, gust);
  color = mix(color, params.foam * ground, cap * 0.75);
  // A wake churns white behind what made it, breaking up as it settles.
  let wash = clamp(churn.w, 0.0, 1.0) * mix(0.5, 1.0, lace);
  color = mix(color, params.foam * ground, wash * 0.8);

  if (params.ice.x > 0.5) {
    // Wind-scoured ice under drifts of snow, with the odd long crack: still, and opaque.
    let drift = smoothstep(-0.25, 0.35, bands(world * 0.09, 0.0) + bands(world * 0.31, 0.0) * 0.4);
    let crack = smoothstep(0.9, 0.99, 1.0 - abs(sin(world.x * 0.13 + 2.0 * sin(world.y * 0.08))));
    let ice = mix(vec3f(0.74, 0.86, 0.93), vec3f(0.92, 0.95, 0.98), drift) * (1.0 - crack * 0.14 * (1.0 - drift));
    return vec4f(ice * ground * 1.15 + mirror * fresnel * 0.2 * (1.0 - drift), 1.0);
  }
  var alpha = max(smoothstep(0.0, 0.1, depth) * mix(0.62, 0.9, deep), max(cap, wash) * 0.9);
  alpha = clamp(alpha - clear * 0.12 + fresnel * 0.35 + spark * 0.4, 0.0, 1.0);
  return vec4f(color, alpha);
}
`


/** What the water samples until the renderer's mirror is handed over: transparent, so it shows sky. */
const blank = new Texture({ data: new Uint8Array(4), width: 1, height: 1 }, { srgb: false, wrap: 'clamp', mipmaps: false })

/** A wake map with nothing on it. */
const still = new Texture({ data: new Uint8Array([128, 128, 128, 0]), width: 1, height: 1 }, { srgb: false, wrap: 'clamp', mipmaps: false })

/** One shared material, so every tile draws with the same pipeline and a sky change reaches them all. */
let shared: ShaderMaterial | null = null
function material(): ShaderMaterial {
  shared ??= new ShaderMaterial({
    wgsl: WGSL,
    vertex: VERTEX,
    uniforms: {
      shallow: Color.hex(0x7fd8f5),
      deep: Color.hex(0x2f8fd8),
      foam: Color.hex(0xf4fcff),
      bed: Color.hex(0xd9cf9a),
      sky: Color.hex(0xffffff),
      sun: Color.hex(0xffffff),
      cloud: Color.hex(0xffffff),
      // Height of the mirrored plane, and 1 while there is one.
      plane: [0, 0],
      // 1 while the water is frozen.
      ice: [0, 0],
      // The waves' clock, seconds into their loop, and 1 while they run (not on ice).
      wave: [0, 1],
      // Where the wake map's corner is (x, z) and how wide it is, metres (`Wakes.area`).
      wakeArea: [0, 0, 1, 0],
    },
    // Mid-grey until the wakes are handed over: no height, no slope.
    textures: { reflection: blank, wakeMap: still },
    blend: 'alpha',
    // Written, so the ink (which reads depth) outlines the surface and not the bed seen through it.
    depthWrite: true,
    // Seen from below too: a swimmer (or anyone on the bed) looks up at it.
    side: 'double',
    // Streams float centimetres over the terrain; a bias keeps their far end from sinking into it.
    depthBias: 12,
    depthBiasSlope: 2,
  })
  return shared
}

/** Freezes every lake, river and stream. */
export function freezeWater() {
  ;(material().uniforms.ice as [number, number])[0] = 1
  ;(material().uniforms.wave as [number, number])[1] = 0
  material().needsUpdate()
}

/** Hands the water the wake map (`Wakes.texture`) and where it lies (`Wakes.area`), after it was drawn again. */
export function setWaterWakes(map: Texture, area: Readonly<[number, number, number, number]>) {
  const m = material()
  m.textures.wakeMap = map
  ;(m.uniforms.wakeArea as [number, number, number, number]).splice(0, 4, ...area)
  m.needsUpdate()
}

/** Sets the waves' clock: `time` seconds into their loop (`waveTime` of the server's tick). */
export function setWaterTime(time: number) {
  ;(material().uniforms.wave as [number, number])[0] = time
  material().needsUpdate()
}

/** Has the water sample `texture`, the renderer's mirror image of the scene. */
export function setWaterReflection(texture: Texture) {
  material().textures.reflection = texture
  material().needsUpdate()
}

/** The height the mirror was drawn at this frame, or null for none (the water then reflects the sky alone). */
export function setWaterPlane(height: number | null) {
  const plane = material().uniforms.plane as [number, number]
  const on = height === null ? 0 : 1
  if (plane[1] === on && (height === null || plane[0] === height)) return
  plane[0] = height ?? plane[0]
  plane[1] = on
  material().needsUpdate()
}

/** Points the reflection and glint at the scene's sky and sun colours (sRGB hex), and the mirrored clouds at `cloud` (linear). */
export function setWaterLight(sky: number, sun: number, cloud: Readonly<Color>) {
  const uniforms = material().uniforms
  ;(uniforms.sky as Color).setHex(sky)
  ;(uniforms.sun as Color).setHex(sun)
  ;(uniforms.cloud as Color).copy(cloud)
  material().needsUpdate()
}

/**
 * One tile's water, or null if it has none: from `prepared` when `prepareTile` built it already. It
 * is translucent and added apart from the tile's group so the game can place it in the world apart from the tile.
 */
export function buildWater(tile: Pick<TileData, 'water' | 'streams'>, ground: TileGround, around?: HeightSource, prepared?: WaterBuffers[]): Group | null {
  const meshes = (prepared ?? waterBuffers(tile, ground, around)).map((buffers) => {
    const mesh = new Mesh(new Geometry(buffers), material())
    mesh.renderOrder = 1
    return mesh
  })
  if (meshes.length === 0) return null
  const group = new Group()
  group.add(...meshes)
  return group
}

/** Frees a tile's water geometry. The material is shared and stays. */
export function disposeWater(group: Group) {
  group.traverse((object) => {
    if (object instanceof Mesh) object.geometry.dispose()
  })
  group.removeFromParent()
}
