import { Color, Geometry, Group, Mesh, ShaderMaterial, Texture } from '@ragnarok/engine'
import type { TileGround } from '@/sim/terrain'
import type { TileData } from '@/world'
import type { HeightSource } from './shade'
import { waterBuffers, type WaterBuffers } from './groundmesh'
import { GROUND_LIGHT_WGSL } from './groundlight'

/**
 * Water: lakes and rivers are a surface mesh per tile at the importer's water level, streams are
 * ribbons draped on the ground. Both share one toy-style shader: a tint that deepens with depth,
 * soft light bands drifting over the surface, a foam line along the shore, and a flow along a
 * stream's length. Depth is a vertex attribute (`level - ground`), not a depth-texture read, so the
 * look needs no extra pass.
 */

/** Shore foam reaches this deep, metres. */
const FOAM_DEPTH = 0.35
/** Along-stream speed of the flow, metres per second. */
const FLOW_SPEED = 1.4

const WGSL = /* wgsl */ `
${GROUND_LIGHT_WGSL}

fn bands(p: vec2f, t: f32) -> f32 {
  let a = sin(p.x * 0.9 + p.y * 0.35 + t * 0.9);
  let b = sin(p.x * -0.4 + p.y * 1.1 - t * 0.7);
  let c = sin((p.x + p.y) * 0.25 + t * 0.4);
  return (a + b + 0.6 * c) / 2.6;
}

// Five crossing swells, long to short. \`fine\` (0 to 1) fades the short ones so far water doesn't shimmer.
fn swell(p: vec2f, t: f32, fine: f32) -> f32 {
  return 0.45 * sin(p.x * 0.9 + p.y * 0.35 + t * 0.9)
       + 0.35 * sin(p.x * -0.4 + p.y * 1.1 - t * 0.7)
       + 0.2 * sin((p.x + p.y) * 1.7 + t * 1.3)
       + fine * 0.14 * sin(p.x * 3.1 - p.y * 2.3 + t * 2.1)
       + fine * 0.1 * sin(p.x * -2.2 - p.y * 3.7 - t * 2.6);
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
  let wave = bands(p, select(time, 0.0, stream));
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

  let e = 0.15;
  let q = select(world * 0.8, p, stream);
  let t = select(time, 0.0, stream);
  let to = frame.cameraPos.xyz - s.world;
  let fine = 1.0 - smoothstep(25.0, 110.0, length(to));
  let h = swell(q, t, fine);
  let grad = vec2f(swell(q + vec2f(e, 0.0), t, fine) - h, swell(q + vec2f(0.0, e), t, fine) - h) / e;
  var n = normalize(vec3f(-grad.x * 0.2, 1.0, -grad.y * 0.2));
  if (!s.front) {
    n.y = -n.y;
  }
  let view = normalize(to);

  // Schlick: little reflection looking straight down, nearly a mirror at a grazing angle.
  let ndv = clamp(abs(dot(n, view)), 0.0, 1.0);
  let fresnel = 0.03 + 0.97 * pow(1.0 - ndv, 5.0);
  let lit = floor(clamp(dot(n, frame.sunDir.xyz) * 0.5 + 0.5, 0.0, 1.0) * 3.0) / 3.0;
  color *= 0.9 + 0.2 * lit;
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
  let level = params.plane.y * (1.0 - smoothstep(0.2, 0.5, abs(s.world.y - params.plane.x)));
  mirror = mix(mirror, seen.rgb, seen.a * level);
  color = mix(color, mirror, clamp(fresnel * 1.3 + 0.12, 0.0, 0.95));

  let glint = pow(max(dot(reflect(-frame.sunDir.xyz, n), view), 0.0), 90.0);
  let spark = smoothstep(0.35, 0.6, glint);
  color += params.sun * (spark * 0.8 + pow(max(dot(reflect(-frame.sunDir.xyz, n), view), 0.0), 24.0) * 0.12);

  let shore = ${FOAM_DEPTH.toFixed(2)} + 0.08 * sin(time * 1.3 + world.x * 0.6 + world.y * 0.45);
  let foam = 1.0 - smoothstep(shore * 0.55, shore, depth);
  let lace = smoothstep(0.1, 0.55, bands(world * 2.1, time * 1.6) * 0.5 + 0.5);
  color = mix(color, params.foam * ground, foam * mix(0.55, 1.0, lace));

  var alpha = smoothstep(0.0, 0.1, depth) * mix(0.62, 0.9, deep);
  alpha = clamp(alpha - clear * 0.12 + fresnel * 0.35 + spark * 0.4, 0.0, 1.0);
  return vec4f(color, alpha);
}
`


/** What the water samples until the renderer's mirror is handed over: transparent, so it shows sky. */
const blank = new Texture({ data: new Uint8Array(4), width: 1, height: 1 }, { srgb: false, wrap: 'clamp', mipmaps: false })

/** One shared material, so every tile draws with the same pipeline and a sky change reaches them all. */
let shared: ShaderMaterial | null = null
function material(): ShaderMaterial {
  shared ??= new ShaderMaterial({
    wgsl: WGSL,
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
    },
    textures: { reflection: blank },
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
