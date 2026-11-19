/**
 * Asphalt (`MAT_ASPHALT`): a paved road's ribbon, with the tarmac's broad mottling, mended patches laid
 * along the road, sealed joints and cracks, grit close up, edges that crumble a little, and its white
 * paint (Swedish practice: dashed edge lines, a dashed or solid centre line, lane lines).
 *
 * The ribbon's vertices carry where they are along the road and across it (`client/src/render/roads.ts`),
 * so the paint and the patches follow the road's own line and run on unbroken across tiles; the
 * mottling, grit and cracks are laid out in world space. Needs MARSH_WGSL before it, for its noise.
 */
export const ASPHALT_WGSL = /* wgsl */ `
// A mended patch's fresher tar, and the sealed seam round it, as shares of the colour.
const ASPHALT_PATCH = 0.78;
const ASPHALT_SEAM = 0.5;
// A crack, and the tar a sealed crack or joint is filled with, as shares of the colour.
const ASPHALT_CRACK = 0.55;
const ASPHALT_TAR = 0.42;
// The tar along a sealed joint down the middle: fainter than a crack's, being older and worn by the traffic.
const ASPHALT_JOINT = 0.72;
// On a tile's ground (\`MAT_GROUND\`), how much of the ground's colour the dust on the road gives it.
const ASPHALT_DUST = 0.08;
// How long a stretch of road may hold one mended patch, metres, and the share of stretches that do.
const ASPHALT_PATCH_STRETCH = 11.0;
const ASPHALT_PATCH_SHARE = 0.35;
// How wide a crack is, metres, and a sealed crack's tar.
const ASPHALT_CRACK_WIDTH = 0.012;
const ASPHALT_TAR_WIDTH = 0.05;
// What \`custom.w\` packs the road's paint in: its half width plus this times the profile
// (\`PROFILE_SCALE\` in roads.ts).
const ASPHALT_PROFILE_SCALE = 32.0;
// The road's paint (\`MARK_*\` in crates/shared/src/world.rs).
const PAINT_EDGE = 1u;
const PAINT_EDGE_SOLID = 2u;
const PAINT_LANES = 4u;
const PAINT_CENTRE = 8u;
const PAINT_CENTRE_SOLID = 16u;
const PAINT_NO_LEFT_EDGE = 32u;
const PAINT_LEFT_SOLID = 64u;
// How far in from the asphalt's edge an edge line's middle is, and how wide the lines are, metres.
const PAINT_EDGE_IN = 0.35;
const PAINT_EDGE_WIDTH = 0.1;
const PAINT_LINE_WIDTH = 0.12;
// Road paint: white, a little dulled by the road's dirt.
const PAINT_WHITE = vec3f(0.78, 0.78, 0.75);

struct AsphaltLook {
  rgb: vec3f,
  // Whether the road covers the ground here, or its crumbled edge has left the verge showing.
  keep: bool,
}

// A line of \`width\` metres centred \`d\` metres away, its edges softened over \`aa\` metres: 1 on it, 0 off.
fn asphaltStripe(d: f32, width: f32, aa: f32) -> f32 {
  return 1.0 - smoothstep(width * 0.5 - aa, width * 0.5 + aa, abs(d));
}

// A dash pattern along the road: \`on\` metres of paint every \`period\` metres, 1 on a dash.
fn asphaltDash(along: f32, period: f32, on: f32, aa: f32) -> f32 {
  let f = fract(along / period) * period;
  return smoothstep(0.0, aa, f) * (1.0 - smoothstep(on - aa, on, f));
}

// \`marshValue\` at \`p\` with its slope, from the same four corners: (value, d/dx, d/dz).
fn asphaltNoiseSlope(p: vec2f) -> vec3f {
  let cell = floor(p);
  let f = p - cell;
  let u = f * f * (3.0 - 2.0 * f);
  let du = 6.0 * f * (1.0 - f);
  let ix = i32(cell.x);
  let iz = i32(cell.y);
  let a = marshHash(ix, iz);
  let b = marshHash(ix + 1, iz);
  let c = marshHash(ix, iz + 1);
  let d = marshHash(ix + 1, iz + 1);
  let k = a - b - c + d;
  return vec3f(a + (b - a) * u.x + (c - a) * u.y + k * u.x * u.y, du.x * (b - a + k * u.y), du.y * (c - a + k * u.x));
}

// How far \`p\` is from the line a crack follows, metres: where a broad wander with a finer one on it
// (so the line zigzags as a crack does) crosses a half, its value there over how steeply it changes,
// so a crack is as thin where the noise is flat as where it is steep.
fn asphaltCrackDistance(p: vec2f, scale: f32, offset: vec2f) -> f32 {
  let broad = asphaltNoiseSlope(p * scale + offset);
  let fine = asphaltNoiseSlope(p * scale * 4.3 + offset.yx);
  let v = 0.7 * broad.x + 0.3 * fine.x;
  let slope = (0.7 * broad.yz + 0.3 * 4.3 * fine.yz) * scale;
  return abs(v - 0.5) / max(length(slope), 0.05);
}

// The white paint at this point: how much of it covers the road, 0 to 1. \`lateral\` is how far right
// of the middle the point is, metres, \`along\` how far along the road, \`half\` the road's half width.
fn asphaltPaint(profile: u32, along: f32, lateral: f32, half: f32, aa: f32) -> f32 {
  let marks = profile & 127u;
  if (marks == 0u) {
    return 0.0;
  }
  let lanes = (profile >> 7u) & 7u;
  let forward = (profile >> 10u) & 7u;
  var paint = 0.0;
  // Edge lines: 1 m of paint and 2 m gap on a country road, solid on a motorway.
  let edge = half - PAINT_EDGE_IN;
  let solidEdge = (marks & PAINT_EDGE_SOLID) != 0u;
  if ((marks & (PAINT_EDGE | PAINT_EDGE_SOLID)) != 0u) {
    let dashed = select(asphaltDash(along, 3.0, 1.0, aa), 1.0, solidEdge);
    paint = max(paint, asphaltStripe(lateral - edge, PAINT_EDGE_WIDTH, aa) * dashed);
    if ((marks & PAINT_NO_LEFT_EDGE) == 0u) {
      let left = select(dashed, 1.0, (marks & PAINT_LEFT_SOLID) != 0u);
      paint = max(paint, asphaltStripe(lateral + edge, PAINT_EDGE_WIDTH, aa) * left);
    }
  }
  // The lanes lie between the edge lines, the ones running against the points on the left.
  if (lanes >= 2u) {
    let width = 2.0 * edge / f32(lanes);
    let backward = lanes - min(forward, lanes);
    for (var k = 1u; k < lanes; k++) {
      let at = -edge + width * f32(k);
      let stripe = asphaltStripe(lateral - at, PAINT_LINE_WIDTH, aa);
      if (k == backward) {
        // Between the two directions: 3 m of paint and 9 m gap, or solid where overtaking is barred.
        if ((marks & PAINT_CENTRE_SOLID) != 0u) {
          paint = max(paint, stripe);
        } else if ((marks & PAINT_CENTRE) != 0u) {
          paint = max(paint, stripe * asphaltDash(along, 12.0, 3.0, aa));
        }
      } else if ((marks & PAINT_LANES) != 0u) {
        paint = max(paint, stripe * asphaltDash(along, 12.0, 3.0, aa));
      }
    }
  }
  return paint;
}

// \`inside\` runs 0 in the middle to 1 at the edge, as far into the road as the whole paved surface
// says (deeper at a junction); \`along\` and \`lateral\` place the point on the road's own line, metres;
// \`packed\` is the half width and the paint (\`ASPHALT_PROFILE_SCALE\`). \`pixel\` is the metres a pixel
// spans: grit and cracks fade out where they would be under a pixel, rather than shimmer. \`ground\` is
// the colour of the ground under it, where \`onGround\` says it is known: dust blown on from it.
fn asphaltLook(rgb: vec3f, world: vec2f, inside: f32, along: f32, lateral: f32, packed: f32, pixel: f32, ground: vec3f, onGround: bool) -> AsphaltLook {
  let profile = u32(floor(packed / ASPHALT_PROFILE_SCALE + 0.001));
  let half = packed - f32(profile) * ASPHALT_PROFILE_SCALE;
  let grit = 1.0 - smoothstep(0.006, 0.03, pixel);
  let close = 1.0 - smoothstep(0.03, 0.12, pixel);
  let aa = max(pixel * 0.75, 0.004);
  // Broad mottling: older, greyer stretches and darker, newer ones.
  let broad = 0.6 * marshValue(world * 0.11 + vec2f(17.0, 5.0)) + 0.4 * marshValue(world * 0.37 + vec2f(3.0, 91.0));
  var c = rgb * mix(0.86, 1.12, broad);

  // A mended patch: a rectangle along the road, in some stretches of it. Most are as wide as a lane or
  // a wheel track and a few metres long; some are a small square where a pothole was filled.
  let stretch = floor(along / ASPHALT_PATCH_STRETCH);
  let ix = i32(stretch);
  let has = step(1.0 - ASPHALT_PATCH_SHARE, marshHash(ix + 313, 71));
  let shape = marshHash(ix + 5, 17);
  let span = mix(1.2, 6.0, marshHash(ix + 29, 3));
  let start = stretch * ASPHALT_PATCH_STRETCH + (ASPHALT_PATCH_STRETCH - span) * marshHash(ix + 11, 43);
  // Across: a wheel track (narrow, at a quarter of the road), a lane, or a small square anywhere.
  let side = select(-1.0, 1.0, marshHash(ix + 61, 13) > 0.5);
  var across = vec2f(side * half * 0.5, 0.45);
  var long = vec2f(start + span * 0.5, span * 0.5);
  if (shape > 0.7) {
    across = vec2f(side * half * 0.45, half * 0.45);
  } else if (shape < 0.25) {
    let size = mix(0.4, 0.9, marshHash(ix + 7, 91));
    across = vec2f((marshHash(ix + 47, 23) * 2.0 - 1.0) * (half - 1.0), size);
    long = vec2f(start + size, size);
  }
  let d = abs(vec2f(along - long.x, lateral - across.x)) - vec2f(long.y, across.y);
  // Distance outside the patch, metres (negative inside), its corners a little rounded.
  let box = length(max(d + 0.08, vec2f(0.0))) + min(max(d.x, d.y) + 0.08, 0.0) - 0.08;
  let mended = (1.0 - smoothstep(-aa, aa, box)) * has;
  c = mix(c, c * ASPHALT_PATCH, mended);
  let seam = asphaltStripe(box, 0.04, aa) * has * close;
  c = mix(c, c * ASPHALT_SEAM, seam * 0.6);

  // Each of the finer details below is worked out only where it can show: most of a road's pixels
  // are far off, uncracked, unpainted asphalt, and the noise is what a pixel costs.
  // Grit: the stones in the mix, pale and dark, only seen close up.
  if (grit > 0.0) {
    let pale = smoothstep(0.7, 0.78, marshValue(world * 29.0 + vec2f(7.0, 31.0)));
    let dark = smoothstep(0.68, 0.76, marshValue(world * 53.0 + vec2f(61.0, 2.0)));
    c *= 1.0 + grit * (0.18 * pale - 0.14 * dark);
  }

  // Cracks: thin wandering lines in about a fifth of the road, some of them sealed with a strip of tar.
  // A sealed crack shows farther off than a bare one.
  let seen = 1.0 - smoothstep(0.03, 0.25, pixel);
  if (seen > 0.0 && mended < 1.0) {
    let cracked = smoothstep(0.62, 0.7, marshValue(world * 0.07 + vec2f(41.0, 113.0)));
    if (cracked > 0.0) {
      let sealed = step(0.55, marshValue(world * 0.05 + vec2f(7.0, 3.0)));
      let fade = mix(close, seen, sealed);
      if (fade > 0.0) {
        let crackAt = asphaltCrackDistance(world, 0.45, vec2f(101.0, 37.0));
        let crackWidth = mix(ASPHALT_CRACK_WIDTH, ASPHALT_TAR_WIDTH, sealed);
        let crack = asphaltStripe(crackAt, max(crackWidth, pixel * 0.6), aa) * cracked * (1.0 - mended) * fade;
        c = mix(c, c * mix(ASPHALT_CRACK, ASPHALT_TAR, sealed), crack);
      }
    }
  }
  // The joint down the middle of a painted road, sealed with tar along some stretches of it.
  let jointSeen = 1.0 - smoothstep(0.03, 0.2, pixel);
  if (profile != 0u && jointSeen > 0.0 && abs(lateral) < 0.2) {
    let joint = lateral + 0.06 * (marshValue(vec2f(along * 0.35, 5.0)) - 0.5);
    let jointed = smoothstep(0.62, 0.7, marshValue(vec2f(along * 0.015, 61.0)));
    c = mix(c, c * ASPHALT_JOINT, asphaltStripe(joint, ASPHALT_TAR_WIDTH, aa) * jointed * (1.0 - mended) * jointSeen);
  }

  // The paint, where this road is the surface: inside another road at a junction, its lines stop.
  if ((profile & 127u) != 0u) {
    let covered = 1.0 - smoothstep(0.15, 0.4, abs(lateral) - inside * half);
    let paint = asphaltPaint(profile, along, lateral, half, aa) * covered;
    if (paint > 0.0) {
      let worn = mix(0.75, 1.0, marshValue(world * 1.7 + vec2f(29.0, 11.0)));
      c = mix(c, PAINT_WHITE * worn, paint);
    }
  }

  // The edge: dusty and crumbling over its last half metre, and broken unevenly into the shoulder.
  let inward = (1.0 - inside) * half;
  let crumble = marshValue(world * 2.6 + vec2f(23.0, 53.0)) - 0.5;
  let keep = inward > crumble * 0.25;
  let dusty = 1.0 - smoothstep(0.0, 0.5, inward);
  c = mix(c, c * vec3f(1.12, 1.08, 1.0), dusty * 0.6);
  // Dust off the ground beside it, a little all over and more at the edges.
  if (onGround) {
    c = mix(c, ground * 0.6, ASPHALT_DUST + 0.25 * dusty);
  }
  return AsphaltLook(c, keep);
}
`
