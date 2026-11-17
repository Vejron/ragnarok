import { CRATER_BUCKETS, CRATER_CELL, CRATER_DRY, CRATER_EMBER_LIFE, CRATER_EMBER_REACH, CRATER_FADE, CRATER_REACH, CRATER_SLOTS } from '../scene/craters'

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
// Seconds the earth a crater turned up stays dark and damp.
const CRATER_DRY = ${CRATER_DRY.toFixed(1)};
// How far from a crater's heart its embers glow, in radii.
const CRATER_EMBER_REACH = ${CRATER_EMBER_REACH.toFixed(2)};
// Seconds a crater's marks last, fading out over the last 30%.
const CRATER_FADE = ${CRATER_FADE.toFixed(1)};

// Two vec4s a crater: x, z, radius, depth (metres); then birth time (seconds), its lean (x, z: the way the
// shell flew over the ground, as long as its share of the flight that lay along the ground) and its
// contact with the ground (0 to 1, plus 2 for a solid shot). Marks fade with age.
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
  // The light those embers throw on the walls of its bowl, 0 to 1: faint by day, plain at night.
  glow: f32,
  // How much of the bare earth is freshly turned up, still dark and damp, 0 to 1.
  damp: f32,
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

// How the shell that dug a crater struck (scene/craters.ts, \`CraterStrike\`), and what sets this
// crater apart from the next one of its size.
struct CraterStrike {
  // The way it flew over the ground, as long as the share of its flight that lay along the ground:
  // nothing for one from straight above, a unit for one skimming in.
  lean: vec2f,
  // How squarely it burst on the ground: 1 in it, towards 0 above it or on a vehicle.
  contact: f32,
  // 1 for a charge that burns the ground, 0 for a solid shot.
  burnt: f32,
  // Four numbers 0 to 1 from the crater's seed: how big its heart is, how many rays it throws, how
  // wide its ring of soil lies and how ragged its edge is.
  vary: vec4f,
}

fn craterStrike(index: u32, seed: vec2f) -> CraterStrike {
  let s = craters[index * 2u + 1u];
  let shot = step(1.5, s.w);
  return CraterStrike(s.yz, s.w - 2.0 * shot, 1.0 - shot, fract(vec4f(seed, seed.yx * 7.31)));
}

// How far the rim is pushed in or out here, as a share of the radius: a ragged edge, not a circle.
fn craterRagged(off: vec2f, seed: vec2f, k: CraterStrike) -> f32 {
  return (craterNoise(off * 1.3 + seed) - 0.5) * mix(0.15, 0.4, k.vary.w);
}

// Where a crater's heart is from \`off\`: a little way on from its middle, where a leaning shell dug in.
fn craterHeart(off: vec2f, r: f32, k: CraterStrike) -> vec2f {
  return off - k.lean * (0.18 * r);
}

// What the burst threw over the ground at \`off\` from a crater's middle, \`t\` radii out along \`dir\`,
// 0 to 1 each. x: its heart, small and black where the shell went into the ground, wide and thin where
// it burst above it. y: streaks thrown out past the rim, a fan of them the way a leaning shell was
// going and few behind it. z: the furrow a shell skimming in ploughed on its way to the middle.
fn craterSplash(off: vec2f, r: f32, t: f32, dir: vec2f, seed: vec2f, ragged: f32, k: CraterStrike) -> vec3f {
  let ahead = dot(dir, k.lean);
  let above = 1.0 - k.contact;
  let size = mix(0.75, 1.3, k.vary.x);
  let th = length(craterHeart(off, r, k)) / r;
  let heart = (1.0 - smoothstep(0.2 * size, mix(0.5, 1.1, above) * size, th + ragged)) * mix(0.85, 0.45, above);
  let streak = smoothstep(0.55 - 0.3 * ahead, 0.8 - 0.3 * ahead, craterNoise(dir * mix(9.0, 17.0, k.vary.y) + seed.yx));
  // Most of the ground round a crater has no streak over it, and most shells plough no furrow: only
  // where there is one is the rest worked out.
  var rays = 0.0;
  if (streak > 0.0 && t > 0.3) {
    // Never past CRATER_REACH, where the crater stops being looked at.
    let reach = min((0.85 + 0.35 * craterNoise(dir * 4.0 + seed)) * (1.0 + 0.45 * ahead), 1.45);
    rays = streak * smoothstep(0.3, 0.6, t) * (1.0 - smoothstep(0.7 * reach, reach, t + ragged)) * 0.6;
  }
  var furrow = 0.0;
  let low = dot(k.lean, k.lean);
  if (low > 0.56) {
    let way = k.lean * inverseSqrt(low);
    let along = dot(off, way) / r;
    let across = abs(off.x * way.y - off.y * way.x) / r;
    furrow = smoothstep(0.56, 0.94, low) * (1.0 - smoothstep(0.04, 0.13, across + ragged * 0.3)) * smoothstep(-1.4, -1.0, along) * (1.0 - smoothstep(-0.1, 0.2, along));
  }
  return vec3f(heart, rays, furrow);
}

// How dark the ground is under \`splash\` (\`craterSplash\`): burnt, or for a solid shot only the
// shade in its furrow.
fn craterChar(off: vec2f, seed: vec2f, splash: vec3f, k: CraterStrike) -> f32 {
  let dark = max(max(splash.x, splash.y) * k.burnt, splash.z * mix(0.45, 0.7, k.burnt));
  if (dark <= 0.0) {
    return 0.0;
  }
  return dark * (0.75 + 0.25 * craterNoise(off * 2.7 + seed.yx));
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
    let k = craterStrike(index, seed);
    g.z = max(g.z, craterChar(off, seed, craterSplash(off, c.z, t, dir, seed, craterRagged(off, seed, k), k), k) * fade);
  }
  return g;
}

// The marks at ground point \`xz\`, where a pixel spans \`pixel\` metres. Every crater listed in the
// point's cell counts: slopes add (as their dents do), each mark takes the strongest.
fn craterMarks(xz: vec2f, pixel: f32, slope: f32) -> CraterMark {
  var m = CraterMark(vec2f(0.0), 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0);
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
    let k = craterStrike(index, seed);
    let ragged = craterRagged(off, seed, k);
    let splash = craterSplash(off, r, t, dir, seed, ragged, k);
    // A crater ages (all from its birth time): its earth dries, then the rain washes its soot and soil
    // thin and the grass creeps in from the rim, before the last of it fades.
    let age = frame.fog.w - craters[index * 2u + 1u].x;
    let weathered = smoothstep(0.25 * CRATER_FADE, 0.7 * CRATER_FADE, age);
    m.char = max(m.char, craterChar(off, seed, splash, k) * mix(1.0, 0.45, weathered) * fade);
    // Embers: a few coals in the crater's heart and nowhere else, glowing, flickering, cooling to
    // nothing, and their light on the walls of the bowl round them.
    if (age < CRATER_EMBER_LIFE && t < 1.0 && k.burnt > 0.0) {
      let cool = 1.0 - age / CRATER_EMBER_LIFE;
      let warm = cool * cool * mix(0.4, 1.0, k.contact);
      let hot = craterHeart(off, r, k) / (CRATER_EMBER_REACH * r);
      let s2 = dot(hot, hot);
      m.glow = max(m.glow, warm * exp(-s2 * 0.35) * (1.0 - t * t));
      if (s2 < 1.0) {
        // Sampled in radii, so a big crater has the same few coals as a small one.
        let coals = smoothstep(0.45, 0.7, craterNoise(hot * 2.6 + seed.yx));
        let flicker = 0.6 + 0.4 * craterNoise(hot * 1.5 + seed + vec2f(frame.fog.w * 1.7, 0.0));
        m.heat = max(m.heat, warm * flicker * exp(-s2 * 5.0) * (1.0 - s2) * mix(coals, 1.0, exp(-s2 * 30.0)));
      }
    }
    // A burst above the ground bares less of it, throws up no ring of soil and leaves no sharp rim.
    let dug = smoothstep(0.2, 0.7, k.contact) * (1.0 - weathered);
    let grown = 0.35 * weathered;
    let bare = (1.0 - smoothstep(0.85 - grown, 1.0 - grown, t + ragged * 0.5)) * mix(0.6, 1.0, k.contact);
    let earth = max(bare, splash.z * (1.0 - weathered)) * fade;
    m.earth = max(m.earth, earth);
    m.damp = max(m.damp, earth * (1.0 - smoothstep(0.0, CRATER_DRY, age)));
    // The soil lies furthest the way the shell was going, and hardly at all behind it. A solid shot's
    // streaks are soil too.
    let far = min(mix(1.15, 1.45, k.vary.z) * (1.0 + 0.3 * dot(dir, k.lean)), 1.5);
    let ring = smoothstep(0.8, 0.97, t) * (1.0 - smoothstep(far - 0.25, far, t + ragged)) * dug;
    m.soil = max(m.soil, max(ring, splash.y * 1.6 * (1.0 - k.burnt)) * fade);
    // A line about a pixel and a half wide on the rim, broken in places as an inked stroke is, and gone
    // where the crater is too small on screen to carry it.
    let line = 1.0 - smoothstep(0.75 * pixel, 1.5 * pixel, abs(d - r * (1.0 + ragged * 0.2)));
    if (line > 0.0) {
      let stroke = smoothstep(0.3, 0.55, craterNoise(dir * 5.0 + seed));
      m.ink = max(m.ink, line * stroke * smoothstep(4.0, 10.0, r / pixel) * dug * fade);
    }
  }
  return m;
}
`
