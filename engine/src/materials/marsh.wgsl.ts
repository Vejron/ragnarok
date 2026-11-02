/**
 * Marsh on the ground (`MAT_MARSH`): tussocks, peat and still pools grown from a noise field over the
 * wet parts of a tile. `marshField` must stay the same as the client's `marshField`
 * (`client/src/render/marshnoise.ts`), which places the sedge and splashes in the same pools.
 */
export const MARSH_WGSL = /* wgsl */ `
// Linear colours of a marsh: sedge, its dry straw tops, wet moss at the pools' edges, peat and still water.
const MARSH_SEDGE = vec3f(0.2, 0.3, 0.075);
const MARSH_STRAW = vec3f(0.45, 0.4, 0.13);
const MARSH_MOSS = vec3f(0.3, 0.42, 0.1);
const MARSH_PEAT = vec3f(0.075, 0.05, 0.026);
const MARSH_WATER = vec3f(0.016, 0.014, 0.008);

fn marshHash(ix: i32, iz: i32) -> f32 {
  var h = (bitcast<u32>(ix) * 0x8da6b343u) ^ (bitcast<u32>(iz) * 0xd8163841u);
  h = (h ^ (h >> 13u)) * 0x5bd1e995u;
  h = h ^ (h >> 15u);
  return f32(h >> 8u) / 16777216.0;
}

fn marshValue(p: vec2f) -> f32 {
  let cell = floor(p);
  let f = p - cell;
  let u = f * f * (3.0 - 2.0 * f);
  let ix = i32(cell.x);
  let iz = i32(cell.y);
  let a = marshHash(ix, iz);
  let b = marshHash(ix + 1, iz);
  let c = marshHash(ix, iz + 1);
  let d = marshHash(ix + 1, iz + 1);
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// The marsh's lie at world (x, z), 0 to 1: pools in the lows, tussocks on the highs.
fn marshField(p: vec2f) -> f32 {
  return 0.65 * marshValue(p * 0.6) + 0.35 * marshValue(p * 1.8 + vec2f(17.0, 31.0));
}

// The level under which the field is open water, for ground this wet (0 to 1).
fn marshPoolLevel(wet: f32) -> f32 {
  return wet * 0.5;
}

struct MarshLook {
  rgb: vec3f,
  // Open water, 0 to 1.
  pool: f32,
  // Wet sheen on peat and moss, 0 to 1.
  sheen: f32,
}

// \`pixel\` is the metres a pixel spans: the edges soften with it, so far marsh does not shimmer.
fn marshLook(rgb: vec3f, world: vec2f, wet: f32, pixel: f32) -> MarshLook {
  let h = marshField(world);
  let level = marshPoolLevel(wet);
  let aa = max(0.015, pixel * 0.35);
  let pool = 1.0 - smoothstep(level - aa, level + aa, h);
  var ground = mix(MARSH_SEDGE, MARSH_STRAW, smoothstep(0.58, 0.78, h));
  ground = mix(MARSH_MOSS, ground, smoothstep(level + 0.04, level + 0.1 + aa, h));
  let peat = 1.0 - smoothstep(level, level + 0.05 + aa, h);
  ground = mix(ground, MARSH_PEAT, peat);
  // The cover's own colour fades in and out at the marsh's edge, where the wetness ramps.
  var look = mix(rgb, ground * (0.7 + 0.6 * rgb.g), smoothstep(0.0, 0.2, wet) * 0.9);
  look = mix(look, MARSH_WATER, pool);
  return MarshLook(look, pool * smoothstep(0.05, 0.25, wet), max(peat, 0.4 * (1.0 - smoothstep(level + 0.04, level + 0.12, h))) * smoothstep(0.0, 0.2, wet));
}

// A still pool's face: flat, stirred by a breeze.
fn marshRipple(world: vec2f, t: f32) -> vec3f {
  let a = sin(world.x * 5.1 + world.y * 1.7 + t * 1.6 + 2.0 * sin(world.y * 2.3 + t * 0.4));
  let b = sin(world.x * -2.3 + world.y * 4.6 - t * 1.2 + 1.5 * sin(world.x * 1.9));
  return normalize(vec3f(a * 0.035, 1.0, b * 0.035));
}

// What the wet ground mirrors: the horizon's haze toward a deeper zenith, and the sun's glint.
fn marshShine(n: vec3f, world: vec3f, pool: f32, sheen: f32, shadow: f32) -> vec3f {
  let view = normalize(frame.cameraPos.xyz - world);
  let ndv = clamp(dot(n, view), 0.0, 1.0);
  let fresnel = 0.03 + 0.97 * pow(1.0 - ndv, 5.0);
  var r = reflect(-view, n);
  r.y = abs(r.y);
  let sky = mix(frame.fogColor.rgb, frame.fogColor.rgb * vec3f(0.55, 0.75, 1.05), pow(clamp(r.y, 0.0, 1.0), 0.55));
  let s = max(dot(r, frame.sunDir.xyz), 0.0);
  let glint = frame.sunColor.rgb * (smoothstep(0.35, 0.6, pow(s, 90.0)) * 0.8 + pow(s, 24.0) * 0.1) * shadow;
  // Peat-brown water: it mirrors less of the sky than a lake does, and browns what it mirrors.
  return pool * (sky * vec3f(0.7, 0.66, 0.55) * clamp(fresnel * 0.9 + 0.05, 0.0, 0.65) + glint) + sheen * (1.0 - pool) * (sky * fresnel * 0.35 + glint * 0.15);
}
`
