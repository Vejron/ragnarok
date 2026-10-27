import { CRATER_BUCKETS, CRATER_CELL, CRATER_EMBER_LIFE, CRATER_FADE, CRATER_REACH, CRATER_SLOTS } from '../scene/craters'

/**
 * The craters' marks on the ground (SCORCH.md), for any scene shader: the crater list and its hash
 * grid (scene/craters.ts) at group 0, and `craterMarks`, which works out at a point of the ground the
 * slope the craters there add and how much of each mark covers it. Needs COMMON_WGSL's group 0 before it.
 */
export const CRATERS_WGSL = /* wgsl */ `
const CRATER_CELL = ${CRATER_CELL.toFixed(1)};
const CRATER_BUCKETS = ${CRATER_BUCKETS}u;
const CRATER_SLOTS = ${CRATER_SLOTS}u;
const CRATER_REACH = ${CRATER_REACH.toFixed(2)};
// Seconds a fresh crater's embers take to go out.
const CRATER_EMBER_LIFE = ${CRATER_EMBER_LIFE.toFixed(1)};
// Seconds a crater's marks last, fading out over the last 30%.
const CRATER_FADE = ${CRATER_FADE.toFixed(1)};

// Two vec4s a crater: x, z, radius, depth (metres); birth time (seconds), unused. Marks fade with age.
@group(0) @binding(5) var<storage, read> craters: array<vec4f>;
// Per bucket: a count, then that many crater indices.
@group(0) @binding(6) var<storage, read> craterGrid: array<u32>;

struct CraterMark {
  // The ground's height gradient the craters add (dh/dx, dh/dz).
  slope: vec2f,
  // Bare earth, char, thrown-out soil and the ink line on the rim, 0 to 1 each.
  earth: f32,
  char: f32,
  soil: f32,
  ink: f32,
  // How much of the sun reaches past the craters' rims (1 all of it), and of the sky into their bowls.
  sun: f32,
  sky: f32,
  // Embers still glowing in a fresh crater, 0 to 1.
  heat: f32,
}

fn craterBucket(cell: vec2i) -> u32 {
  return ((bitcast<u32>(cell.x) * 73856093u) ^ (bitcast<u32>(cell.y) * 19349663u)) & (CRATER_BUCKETS - 1u);
}

fn craterHash(p: vec2i) -> f32 {
  var h = (bitcast<u32>(p.x) * 1664525u) ^ (bitcast<u32>(p.y) * 1013904223u);
  h ^= h >> 16u;
  h *= 2246822519u;
  h ^= h >> 13u;
  h *= 3266489917u;
  h ^= h >> 16u;
  return f32(h) * (1.0 / 4294967296.0);
}

// Value noise, 0 to 1, a lattice point a unit.
fn craterNoise(p: vec2f) -> f32 {
  let i = vec2i(floor(p));
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  let a = craterHash(i);
  let b = craterHash(i + vec2i(1, 0));
  let c = craterHash(i + vec2i(0, 1));
  let d = craterHash(i + vec2i(1, 1));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// The height of crater \`c\`'s bowl at \`xz\`, deepened by \`scale\`: 0 at and past the rim.
fn craterHeight(c: vec4f, xz: vec2f, scale: f32) -> f32 {
  let t2 = min(dot(xz - c.xy, xz - c.xy) / (c.z * c.z), 1.0);
  return -c.w * scale * (1.0 - t2) * (1.0 - t2);
}

// Whether the sun reaches \`xz\` in crater \`c\`'s bowl (deepened by \`scale\`) past its rim: the ray
// toward the sun, marched to the far side, against the bowl's own heights. Softened near the edge of the shadow.
fn craterSun(c: vec4f, xz: vec2f, scale: f32) -> f32 {
  let sun = frame.sunDir.xyz;
  let across = length(sun.xz);
  if (sun.y <= 0.0 || across < 1e-3) {
    return 1.0;
  }
  let toward = sun.xz / across;
  let rise = sun.y / across;
  let here = craterHeight(c, xz, scale);
  var lit = 1.0;
  for (var i = 1u; i <= 6u; i++) {
    let along = c.z * 2.0 * f32(i) / 6.0;
    let above = here + along * rise - craterHeight(c, xz + toward * along, scale);
    lit = min(lit, smoothstep(-0.05, 0.1, above));
  }
  return lit;
}

// How much of crater \`index\`'s marks are left at its age: 1, then fading to 0 at CRATER_FADE.
fn craterFade(index: u32) -> f32 {
  return 1.0 - smoothstep(0.7 * CRATER_FADE, CRATER_FADE, frame.fog.w - craters[index * 2u + 1u].x);
}

// The seed of crater \`c\`'s noise: from where it is, so every player's crater looks the same.
fn craterSeed(c: vec4f) -> vec2f {
  return vec2f(craterHash(vec2i(c.xy * 10.0)), craterHash(vec2i(c.yx * 10.0))) * 100.0;
}

// How far the rim is pushed in or out here, as a share of the radius: a ragged edge, not a circle.
fn craterRagged(off: vec2f, seed: vec2f) -> f32 {
  return (craterNoise(off * 1.3 + seed) - 0.5) * 0.25;
}

// The char at \`off\` from a crater's middle, \`t\` radii out along \`dir\`: a black heart, then streaks
// thrown out past the rim, some long, some short, most none.
fn craterChar(off: vec2f, t: f32, dir: vec2f, seed: vec2f, ragged: f32) -> f32 {
  let core = (1.0 - smoothstep(0.2, 0.5, t + ragged)) * 0.85;
  let streak = smoothstep(0.55, 0.8, craterNoise(dir * 13.0 + seed.yx));
  let reach = 1.0 + 0.45 * craterNoise(dir * 4.0 + seed);
  let rays = streak * smoothstep(0.3, 0.6, t) * (1.0 - smoothstep(0.7 * reach, reach, t + ragged)) * 0.6;
  let speckle = 0.75 + 0.25 * craterNoise(off * 2.7 + seed.yx);
  return max(core, rays) * speckle;
}

// What the craters do to a blade of grass rooted at \`root\`: xy its lean away from them (tip offset
// per metre of blade), laid flattest just past the rim; z how burnt it is, as the ground under it.
fn craterGrass(root: vec2f) -> vec3f {
  var g = vec3f(0.0);
  let base = craterBucket(vec2i(floor(root / CRATER_CELL))) * CRATER_SLOTS;
  let count = min(craterGrid[base], CRATER_SLOTS - 1u);
  for (var k = 0u; k < count; k++) {
    let index = craterGrid[base + 1u + k];
    let c = craters[index * 2u];
    let off = root - c.xy;
    let d = length(off);
    let t = d / c.z;
    let fade = craterFade(index);
    if (t >= CRATER_REACH || fade <= 0.0) {
      continue;
    }
    let dir = off / max(d, 1e-4);
    let seed = craterSeed(c);
    g += vec3f(dir * 1.2 * smoothstep(0.7, 0.9, t) * (1.0 - smoothstep(0.95, 1.5, t)) * fade, 0.0);
    g.z = max(g.z, craterChar(off, t, dir, seed, craterRagged(off, seed)) * fade);
  }
  return g;
}

// The marks at ground point \`xz\`, where a pixel spans \`pixel\` metres. Every crater listed in the
// point's cell counts: slopes add (as their dents do), each mark takes the strongest.
fn craterMarks(xz: vec2f, pixel: f32, slope: f32) -> CraterMark {
  var m = CraterMark(vec2f(0.0), 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0);
  let base = craterBucket(vec2i(floor(xz / CRATER_CELL))) * CRATER_SLOTS;
  let count = min(craterGrid[base], CRATER_SLOTS - 1u);
  for (var k = 0u; k < count; k++) {
    let index = craterGrid[base + 1u + k];
    let c = craters[index * 2u];
    let r = c.z;
    let off = xz - c.xy;
    let d = length(off);
    let fade = craterFade(index);
    if (d >= r * CRATER_REACH || fade <= 0.0) {
      continue;
    }
    let t = d / r;
    let dir = off / max(d, 1e-4);
    let seed = craterSeed(c);

    // The bowl, depth (1 - t²)²: its slope down toward the middle.
    if (t < 1.0) {
      m.slope += dir * (slope * 4.0 * c.w * t * (1.0 - t * t) / r) * fade;
      m.sun = min(m.sun, mix(1.0, craterSun(c, xz, slope), fade));
      // Less sky reaches the bottom of a bowl: darkest in the middle.
      m.sky = min(m.sky, 1.0 - 0.35 * (1.0 - t * t) * (1.0 - t * t) * fade);
    }
    // A low lip just past the rim, drawn only (the ground has none): a bump a tenth of the depth.
    let lip = (t - 1.08) / 0.22;
    m.slope += dir * (-slope * 0.12 * c.w * 2.0 * lip * exp(-lip * lip) / (0.22 * r)) * fade;

    // Noise on the direction rather than the angle, so it has no seam where the angle wraps.
    let ragged = craterRagged(off, seed);
    m.char = max(m.char, craterChar(off, t, dir, seed, ragged) * fade);
    // Embers: patches of the char in the middle glowing, flickering, cooling to nothing.
    let age = frame.fog.w - craters[index * 2u + 1u].x;
    if (age < CRATER_EMBER_LIFE && t < 0.7) {
      let cool = 1.0 - age / CRATER_EMBER_LIFE;
      let patches = smoothstep(0.55, 0.8, craterNoise(off * 2.4 + seed.yx));
      let flicker = 0.6 + 0.4 * craterNoise(off * 0.8 + seed + vec2f(frame.fog.w * 1.7, 0.0));
      m.heat = max(m.heat, cool * cool * patches * flicker * (1.0 - smoothstep(0.35, 0.7, t + ragged)));
    }
    m.earth = max(m.earth, (1.0 - smoothstep(0.85, 1.0, t + ragged * 0.5)) * fade);
    m.soil = max(m.soil, smoothstep(0.8, 0.97, t) * (1.0 - smoothstep(1.05, 1.3, t + ragged)) * fade);
    // A line about a pixel and a half wide on the rim, broken in places as an inked stroke is, and gone
    // where the crater is too small on screen to carry it.
    let line = 1.0 - smoothstep(0.75 * pixel, 1.5 * pixel, abs(d - r * (1.0 + ragged * 0.2)));
    let stroke = smoothstep(0.3, 0.55, craterNoise(dir * 5.0 + seed));
    m.ink = max(m.ink, line * stroke * smoothstep(4.0, 10.0, r / pixel) * fade);
  }
  return m;
}
`
