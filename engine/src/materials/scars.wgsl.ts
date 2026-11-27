import { SCAR_BLOOD, SCAR_COOL, SCAR_CRACK, SCAR_DENT, SCAR_GOUGE, SCAR_HOLE, SCAR_HOT, SCAR_SOOT, SCAR_STRIDE, SCAR_VEC4S } from '../scene/scars'

const f = (n: number) => n.toFixed(1)

/**
 * Damage drawn on a mesh's own surface (HULL-DAMAGE.md, scene/scars.ts): its slot's scars, each a point
 * in the mesh's frame with a reach, the way the round came and a style. `scarLook` is what a pixel shows: the dent's slope and shade, soot and
 * char, a hole with a tempered-steel ring, gouges to bare metal, cracked glass, blood, and the heat of a fresh hit. Analytic,
 * so it needs no texture and moves with the mesh. `scarDent` presses the vertices in too, where the
 * device lets the vertex stage read the list (`Renderer.vertexScars`); elsewhere the dents are in the
 * shading alone. Included in COMMON_WGSL.
 */
export const SCARS_WGSL = /* wgsl */ `
@group(0) @binding(13) var<storage, read> scars: array<vec4f>;

const SCAR_STRIDE = ${SCAR_STRIDE}u;
const SCAR_VEC4S = ${SCAR_VEC4S}u;
const SCAR_DENT = ${SCAR_DENT}u;
const SCAR_HOLE = ${SCAR_HOLE}u;
const SCAR_GOUGE = ${SCAR_GOUGE}u;
const SCAR_SOOT = ${SCAR_SOOT}u;
const SCAR_HOT = ${SCAR_HOT}u;
const SCAR_CRACK = ${SCAR_CRACK}u;
const SCAR_BLOOD = ${SCAR_BLOOD}u;
const SCAR_COOL = ${f(SCAR_COOL)};
// A hole's black middle and its tempered ring's outer edge, in the scar's reach.
const SCAR_HOLE_R = 0.3;
const SCAR_TEMPER_R = 0.72;
// How far soot reaches, in the scar's reach, and how much further it reaches upward.
const SCAR_SOOT_R = 2.1;
const SCAR_SOOT_RISE = 1.6;
// A gouge's length and width, in the scar's reach.
const SCAR_GOUGE_LONG = 3.2;
const SCAR_GOUGE_WIDE = 0.28;
// The raised lip round a dent, as a share of its depth, where it peaks and how wide it is, in the reach.
const SCAR_LIP = 0.22;
const SCAR_LIP_AT = 1.08;
const SCAR_LIP_WIDE = 0.2;
// Broken glass, in the scar's reach: the hole, the crushed white round it, how many cracks run out in
// its star, how wide one is at the hole, and where the ring of cracks round it lies.
const SCAR_CRACK_HOLE = 0.05;
const SCAR_CRACK_CRUSH = 0.2;
const SCAR_CRACK_RAYS = 9.0;
const SCAR_CRACK_WIDE = 0.045;
const SCAR_CRACK_RING = 0.42;
// Blood, in the scar's reach: the splat's middle, how many spikes are thrown out of it, how wide one
// is at its root, and how far apart the runs down the face may start and the longest of them.
const SCAR_BLOOD_CORE = 0.42;
const SCAR_BLOOD_RAYS = 13.0;
const SCAR_BLOOD_WIDE = 0.07;
const SCAR_BLOOD_RUN_EVERY = 0.13;
const SCAR_BLOOD_RUN_LONG = 2.3;

// A local point's own value noise, 0 to 1: steady on the surface, so soot and chips never shimmer. An
// integer hash (PCG), cheaper than the usual sine.
fn scarHash(p: vec3f) -> f32 {
  var v = vec3u(vec3i(p) + vec3i(32768));
  v = v * 1664525u + 1013904223u;
  v.x += v.y * v.z;
  v.y += v.z * v.x;
  v.z += v.x * v.y;
  v ^= v >> vec3u(16u);
  v.x += v.y * v.z;
  return f32(v.x >> 8u) / 16777216.0;
}

fn scarNoise(p: vec3f) -> f32 {
  let i = floor(p);
  let t = fract(p);
  let u = t * t * (3.0 - 2.0 * t);
  let a = mix(mix(scarHash(i), scarHash(i + vec3f(1.0, 0.0, 0.0)), u.x), mix(scarHash(i + vec3f(0.0, 1.0, 0.0)), scarHash(i + vec3f(1.0, 1.0, 0.0)), u.x), u.y);
  let b = mix(mix(scarHash(i + vec3f(0.0, 0.0, 1.0)), scarHash(i + vec3f(1.0, 0.0, 1.0)), u.x), mix(scarHash(i + vec3f(0.0, 1.0, 1.0)), scarHash(i + vec3f(1.0, 1.0, 1.0)), u.x), u.y);
  return mix(a, b, u.z);
}

// How far a vertex at local \`p\` on a mesh with slot \`handle\` (Mesh.scars) is pressed in, as a local
// offset along each dent's round's way: the same bowl \`scarLook\` shades. Vertex stage only, and only
// where the device lets it read the list (Renderer.vertexScars).
fn scarDent(handle: u32, p: vec3f) -> vec3f {
  let base = (handle - 1u) * SCAR_STRIDE;
  let count = min(u32(scars[base].x), ${SCAR_STRIDE - 1}u / SCAR_VEC4S);
  var off = vec3f(0.0);
  for (var i = 0u; i < count; i++) {
    let o = base + 1u + i * SCAR_VEC4S;
    let a = scars[o];
    let c = scars[o + 2u];
    if ((u32(c.x) & SCAR_DENT) == 0u || a.w <= 0.0) {
      continue;
    }
    let q = p - a.xyz;
    let s2 = dot(q, q) / (a.w * a.w);
    if (s2 >= 1.0) {
      continue;
    }
    let k = 1.0 - s2;
    off += scars[o + 1u].xyz * (c.z * k * k);
  }
  return off;
}

struct ScarLook {
  // The height's gradient in the mesh's frame: the normal is tipped against it.
  slope: vec3f,
  // How much of the light reaches into the dents (1 all of it).
  shade: f32,
  // Soot (darkens), char (near black), bare metal showing (bright, glinting) and the hole itself, each 0 to 1.
  soot: f32,
  char: f32,
  bare: f32,
  hole: f32,
  // Cracks in broken glass, 0 to 1: they are drawn pale and opaque.
  crack: f32,
  // Blood thrown onto it, 0 to 1: drawn dark red and opaque.
  blood: f32,
  // The tempered ring's colour and how much of it shows.
  temper: vec3f,
  temperAmount: f32,
  // A fresh hit's glow, linear rgb, bright enough to burn towards white.
  heat: vec3f,
}

// The colours steel takes as it is heated, from the coolest (straw) out from the hole to the hottest (blue) by it.
fn scarTemper(t: f32) -> vec3f {
  let straw = vec3f(0.7, 0.55, 0.25);
  let bronze = vec3f(0.5, 0.26, 0.11);
  let purple = vec3f(0.3, 0.12, 0.36);
  let blue = vec3f(0.1, 0.2, 0.48);
  if (t < 0.33) {
    return mix(blue, purple, t / 0.33);
  }
  if (t < 0.66) {
    return mix(purple, bronze, (t - 0.33) / 0.33);
  }
  return mix(bronze, straw, (t - 0.66) / 0.34);
}

// What the scars of slot \`handle\` do at local \`p\` on a face with local normal \`n\` (unit), where \`up\`
// is the world's up in the mesh's frame (unit) and \`now\` the renderer's clock, seconds.
fn scarLook(handle: u32, p: vec3f, n: vec3f, up: vec3f, now: f32) -> ScarLook {
  var look = ScarLook(vec3f(0.0), 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, vec3f(0.0), 0.0, vec3f(0.0));
  let base = (handle - 1u) * SCAR_STRIDE;
  let header = scars[base];
  let count = min(u32(header.x), ${SCAR_STRIDE - 1}u / SCAR_VEC4S);
  // Up along the face, for soot that rises; and the face's own noise, at two sizes, worked out only
  // where a scar or the grime reaches (most of a big part is clean).
  let rise = up - n * dot(up, n);
  var noisy = false;
  var lo = 0.5;
  var hi = 0.5;
  for (var i = 0u; i < count; i++) {
    let o = base + 1u + i * SCAR_VEC4S;
    let a = scars[o];
    let r = a.w;
    let q = p - a.xyz;
    let d2 = dot(q, q);
    let reach = r * max(SCAR_SOOT_R + SCAR_SOOT_RISE, SCAR_GOUGE_LONG);
    // An unused light scar has no reach.
    if (r <= 0.0 || d2 >= reach * reach) {
      continue;
    }
    if (!noisy) {
      noisy = true;
      lo = scarNoise(p * 1.7);
      hi = scarNoise(p * 4.0);
    }
    let dir = scars[o + 1u].xyz;
    let c = scars[o + 2u];
    let style = u32(c.x);
    let d = sqrt(d2);
    let s = d / r;
    // The dent: a bowl pressed in to \`depth\` with a lip pushed up round it. Its height along the face's
    // normal is h(s), so the gradient is h'(s) along q / (d r).
    if ((style & SCAR_DENT) != 0u && s < SCAR_LIP_AT + 3.0 * SCAR_LIP_WIDE) {
      let k = max(0.0, 1.0 - s * s);
      let bowl = c.z * 4.0 * k * s;
      let x = (s - SCAR_LIP_AT) / SCAR_LIP_WIDE;
      let lip = -c.z * SCAR_LIP * exp(-x * x) * 2.0 * x / SCAR_LIP_WIDE;
      look.slope += q / max(d * r, 1e-5) * (bowl + lip);
      look.shade *= 1.0 - 0.45 * k * k;
      // Paint flakes off round its rim, down to the metal.
      look.bare = max(look.bare, step(0.68, hi) * smoothstep(1.5, 0.9, s) * smoothstep(0.5, 0.85, s));
    }
    if ((style & SCAR_SOOT) != 0u) {
      // Soot reaches further up the face than down it, ragged at its edge.
      let above = max(0.0, dot(q, rise));
      let spread = length(q - rise * above * (SCAR_SOOT_RISE / (SCAR_SOOT_R + SCAR_SOOT_RISE))) / (r * SCAR_SOOT_R);
      let soot = (1.0 - smoothstep(0.2, 1.0, spread + (lo - 0.5) * 0.45)) * mix(0.7, 1.0, hi);
      look.soot = max(look.soot, soot * 0.85);
      look.char = max(look.char, 0.75 * (1.0 - smoothstep(0.06, 0.3, spread + (hi - 0.5) * 0.12)));
    }
    if ((style & SCAR_HOLE) != 0u) {
      let edge = (lo - 0.5) * 0.06;
      let hole = 1.0 - smoothstep(SCAR_HOLE_R - 0.03, SCAR_HOLE_R + 0.01, s + edge);
      look.hole = max(look.hole, hole);
      // The tempered ring: hottest (blue) by the hole, straw at its edge.
      let t = clamp((s - SCAR_HOLE_R) / (SCAR_TEMPER_R - SCAR_HOLE_R) + (lo - 0.5) * 0.3, 0.0, 1.0);
      let ring = (1.0 - hole) * (1.0 - smoothstep(0.75, 1.0, t)) * step(SCAR_HOLE_R, s + edge);
      if (ring > look.temperAmount) {
        look.temper = scarTemper(t);
        look.temperAmount = ring * 0.9;
      }
    }
    if ((style & SCAR_GOUGE) != 0u) {
      // Along the way the round went over the face, and across it.
      let way = dir - n * dot(dir, n);
      let wl = length(way);
      if (wl > 1e-3) {
        let t = way / wl;
        let along = dot(q, t) / r;
        let across = dot(q, cross(n, t)) / r;
        // Widest just past where it struck, narrowing to its end.
        let shape = smoothstep(-0.3, 0.1, along) * (1.0 - smoothstep(SCAR_GOUGE_LONG * 0.6, SCAR_GOUGE_LONG, along));
        let wide = SCAR_GOUGE_WIDE * mix(1.0, 0.35, clamp(along / SCAR_GOUGE_LONG, 0.0, 1.0)) * mix(0.8, 1.2, lo);
        let groove = shape * (1.0 - smoothstep(wide * 0.6, wide, abs(across)));
        // Fine lines torn along it.
        let lines = 0.6 + 0.4 * sin(across * 90.0 + hi * 3.0);
        look.bare = max(look.bare, groove * lines);
        look.soot = max(look.soot, shape * (1.0 - smoothstep(wide, wide * 2.4, abs(across))) * 0.35);
        look.shade *= 1.0 - 0.3 * groove;
      }
    }
    if ((style & SCAR_CRACK) != 0u && s < 1.0) {
      // Round the hole in the face's own plane: each crack of the star runs out at its own bearing, as
      // far as its own length, and thins as it goes. The scar's place seeds them, so no two look alike.
      let side = normalize(cross(n, select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), abs(n.y) < 0.9)));
      let seed = scarHash(a.xyz * 211.0);
      let turn = (atan2(dot(q, cross(n, side)), dot(q, side)) / 6.2831853 + 0.5) * SCAR_CRACK_RAYS + seed * SCAR_CRACK_RAYS;
      let cell = floor(turn);
      let wander = (scarHash(vec3f(cell, seed * 97.0, 3.0)) - 0.5) * 0.6;
      let off = abs(fract(turn) - 0.5 - wander * 0.5);
      let across = d * off * 6.2831853 / SCAR_CRACK_RAYS;
      let wide = r * SCAR_CRACK_WIDE * (1.15 - s);
      let runs = mix(0.5, 1.0, scarHash(vec3f(cell, seed * 53.0, 7.0)));
      let ray = (1.0 - smoothstep(wide * 0.5, wide, across)) * (1.0 - smoothstep(runs * 0.85, runs, s));
      // The ring breaks off in some sectors and wanders a little in the rest.
      let ringAt = SCAR_CRACK_RING + (scarHash(vec3f(cell, seed * 17.0, 11.0)) - 0.5) * 0.08;
      let ring = (1.0 - smoothstep(0.012, 0.025, abs(s - ringAt))) * step(0.35, scarHash(vec3f(cell, seed * 29.0, 13.0)));
      let crush = 1.0 - smoothstep(SCAR_CRACK_HOLE, SCAR_CRACK_CRUSH, s);
      look.crack = max(look.crack, max(max(ray, ring * 0.8), crush * 0.9));
      look.hole = max(look.hole, 1.0 - smoothstep(SCAR_CRACK_HOLE * 0.6, SCAR_CRACK_HOLE, s));
    }
    if ((style & SCAR_BLOOD) != 0u) {
      // In the face's own plane, against the way the spray went over it (any way, for blood that struck square).
      let qp = q - n * dot(q, n);
      let dp = length(qp) / r;
      let side = normalize(cross(n, select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), abs(n.y) < 0.9)));
      let seed = scarHash(a.xyz * 173.0);
      let thrown = dir - n * dot(dir, n);
      let lean = clamp(length(thrown) * 1.5, 0.0, 1.0);
      let t = select(side, thrown / max(length(thrown), 1e-4), lean > 0.15);
      let b = cross(n, t);
      let along = dot(qp, t) / r;
      let athwart = dot(qp, b) / r;
      // The middle: a ragged blob, drawn out a little the way it was thrown.
      let blob = length(vec2f((along - 0.2 * lean) / (1.0 + 0.7 * lean), athwart));
      var blood = 1.0 - smoothstep(SCAR_BLOOD_CORE * 0.8, SCAR_BLOOD_CORE, blob + (lo - 0.5) * 0.3 + (hi - 0.5) * 0.12);
      // Spikes out of it at their own bearings, longest the way it was thrown, each with a drop at its end.
      let bearing = atan2(athwart, along);
      let turn = (bearing / 6.2831853 + 0.5) * SCAR_BLOOD_RAYS + seed * SCAR_BLOOD_RAYS;
      let cell = floor(turn);
      let pick = scarHash(vec3f(cell, seed * 41.0, 5.0));
      let runs = mix(0.55, 1.25, pick) * (1.0 + lean * max(0.0, cos(bearing)) * 1.1);
      let across = dp * abs(fract(turn) - 0.5) * 6.2831853 / SCAR_BLOOD_RAYS;
      let wide = SCAR_BLOOD_WIDE * max(0.15, 1.0 - dp / runs);
      let spike = (1.0 - smoothstep(wide * 0.5, wide, across)) * (1.0 - smoothstep(runs * 0.85, runs, dp)) * step(0.35, pick);
      let drop = (1.0 - smoothstep(0.045, 0.07, length(vec2f(dp - runs * 1.12, across)))) * step(0.5, pick);
      blood = max(blood, max(spike, drop));
      // Runs down the face from the splat's lower edge, where the face is not lying flat.
      let fall = length(rise);
      if (fall > 0.3) {
        let down = -rise / fall;
        let sideways = cross(n, down);
        let x = dot(qp, sideways) / r;
        let y = dot(qp, down) / r;
        let column = floor(x / SCAR_BLOOD_RUN_EVERY);
        let k = scarHash(vec3f(column, seed * 67.0, 17.0));
        let middle = (column + 0.5 + (scarHash(vec3f(column, seed * 31.0, 19.0)) - 0.5) * 0.5) * SCAR_BLOOD_RUN_EVERY;
        let long = mix(0.5, SCAR_BLOOD_RUN_LONG, k * k) * fall;
        let width = 0.022 * (1.0 + 0.8 * smoothstep(long * 0.8, long, y));
        let run = (1.0 - smoothstep(width * 0.6, width, abs(x - middle))) * step(0.0, y) * (1.0 - smoothstep(long * 0.97, long, y)) * step(0.45, k) * step(abs(middle), SCAR_BLOOD_CORE * 0.8);
        blood = max(blood, run);
      }
      look.blood = max(look.blood, blood);
    }
    if ((style & SCAR_HOT) != 0u) {
      let age = now - c.y;
      if (age >= 0.0 && age < SCAR_COOL) {
        let cool = age / SCAR_COOL;
        let glow = exp(-s * s * 9.0) * (1.0 - cool) * (1.0 - cool);
        // White-orange to deep red as it cools.
        let tint = mix(vec3f(4.0, 1.4, 0.35), vec3f(1.2, 0.12, 0.02), sqrt(cool));
        look.heat += tint * glow * mix(0.8, 1.2, hi);
      }
    }
  }
  // A battered vehicle's grime, all over its parts.
  let grime = header.y;
  if (grime <= 0.0) {
    return look;
  }
  if (!noisy) {
    lo = scarNoise(p * 1.7);
    hi = scarNoise(p * 4.0);
  }
  look.soot = max(look.soot, grime * smoothstep(0.35, 0.8, lo) * 0.6);
  look.bare = max(look.bare, grime * step(0.86, hi) * 0.6);
  return look;
}
`
