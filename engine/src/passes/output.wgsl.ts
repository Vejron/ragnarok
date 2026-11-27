/**
 * The last pass, from the resolved HDR image to the canvas: ink outlines (optional), light shafts and
 * lamps' beams added, the exposure (fixed, or the eye's adaptation), what is far past white burnt
 * white, colour grading, tone mapping, a vignette and the linear-to-sRGB encode. `multisampled` says whether the scene depth it reads for the
 * ink is.
 */
export function outputWgsl(multisampled: boolean) {
  return /* wgsl */ `
struct Output {
  // tone mapping (0 none, 1 neutral), exposure, ink on (0 or 1), ink width in pixels.
  params: vec4f,
  // ink darken, ink strength, ink threshold, the distance by which the lines have faded away (metres).
  ink: vec4f,
  // camera near, camera far, and the pixel in the target the picture starts at (x, y).
  clip: vec4f,
  // how far light past white burns white (0 to 1), light shafts' strength, vignette (0 to 1), Purkinje shift (0 to 1).
  finish: vec4f,
  // Grading: what the shadows are multiplied by (rgb), saturation.
  shadows: vec4f,
  // What the highlights are multiplied by (rgb), whether the exposure is the eye's adaptation (0 or 1).
  highlights: vec4f,
  // Whether lamps' beams were drawn this frame (0 or 1), how much to sharpen (0 to 1), whether smoke was (0 or 1), how much the ink's line wavers (0 to 1).
  beams: vec4f,
  // The way a pixel looks out into the world, for the ink's waver: the ray through the picture's top
  // left corner, and what is added to it across the picture's width and down its height.
  rayCorner: vec4f,
  rayAcross: vec4f,
  rayDown: vec4f,
}

@group(0) @binding(0) var<uniform> output: Output;
@group(0) @binding(1) var hdr: texture_2d<f32>;
@group(0) @binding(2) var depth: ${multisampled ? 'texture_depth_multisampled_2d' : 'texture_depth_2d'};
@group(0) @binding(3) var linearSampler: sampler;
@group(0) @binding(4) var shafts: texture_2d<f32>;
// The eye's adaptation (passes/exposure.ts): its exposure is element 1.
@group(0) @binding(5) var<storage, read> adaptation: array<f32, 4>;
@group(0) @binding(6) var beams: texture_2d<f32>;
// The half-resolution effects' target (passes/halfres.ts): its alpha is how much of a pixel smoke covers.
@group(0) @binding(7) var smoke: texture_2d<f32>;
// Random numbers for the ink's pen, and a sampler that repeats them.
@group(0) @binding(8) var pen: texture_2d<f32>;
@group(0) @binding(9) var penSampler: sampler;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  // One triangle covering the screen.
  let uv = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
  return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
}

// View distance of a reversed-depth value (0 is the far plane, 1 the near).
fn viewDepth(p: vec2i) -> f32 {
  let size = vec2i(textureDimensions(depth));
  let d = textureLoad(depth, clamp(p, vec2i(0), size - 1), 0);
  let near = output.clip.x;
  let far = output.clip.y;
  return near * far / (d * (far - near) + near);
}

// A colour's brightest channel at which it starts to burn white, and at which it is white through.
const HOT_START = 2.0;
const HOT_FULL = 10.0;

// Light far past white is no longer a colour: a flash, a tracer or the heart of a fire burns white, and
// keeps its colour only at its dimmer rim. Without it such a colour clips to a flat, saturated patch.
fn whiteHot(c: vec3f, amount: f32) -> vec3f {
  let peak = max(c.r, max(c.g, c.b));
  return mix(c, vec3f(peak), smoothstep(HOT_START, HOT_FULL, peak) * amount);
}

// How far the pen's line strays to either side of the edge it follows, against the ink's width, and the
// share of its way along which it swells to a heavier line.
const PEN_STRAY = 1.1;
const PEN_SWELLS = 0.4;
// How many of the pen's swells fit in a radian of the view, and of its slower strayings and its runs
// of ink and no ink.
const PEN_SWELL = 34.0;
const PEN_SLOW = 11.0;

// Four smooth noises over the directions of the view, each 0 to 1, with about \`grain\` turns of each to
// a radian: one read of a small texture of random numbers (the renderer's pen noise), blended between
// its texels. The direction is unfolded onto a square as an octahedron's faces are, the sky's half
// over the ground's, so the noise is fixed to the world as the camera turns and a line keeps its shape
// under the crosshair; walking past a thing shifts it slowly, as a line drawn again would. Doing this
// with arithmetic cost five times what the rest of the ink does where trees fill the view.
fn penNoise(ray: vec3f, grain: f32) -> vec4f {
  let square = ray.xz / (abs(ray.x) + abs(ray.y) + abs(ray.z));
  return textureSampleLevel(pen, penSampler, square * grain / f32(textureDimensions(pen).x), 0.0);
}

// The ink starts to fade at this share of its distance, is at full weight within INK_NEAR metres, and
// is INK_NEAR_WEIGHT times as strong there as further off.
const INK_FADE_FROM = 0.3;
const INK_NEAR = 12.0;
const INK_NEAR_WEIGHT = 1.3;

// Khronos PBR Neutral, as three's NeutralToneMapping.
fn neutral(c: vec3f) -> vec3f {
  let start = 0.8 - 0.04;
  let desaturation = 0.15;
  let x = min(c.r, min(c.g, c.b));
  let offset = select(0.04, x - 6.25 * x * x, x < 0.08);
  var color = c - offset;
  let peak = max(color.r, max(color.g, color.b));
  if (peak < start) {
    return color;
  }
  let d = 1.0 - start;
  let newPeak = 1.0 - d * d / (peak + d - start);
  color *= newPeak / peak;
  let g = 1.0 - 1.0 / (desaturation * (peak - newPeak) + 1.0);
  return mix(color, vec3f(newPeak), g);
}

// Split toning by luminance, saturation, and by night the shift of dim colours toward the blue-grey of
// rod vision (the Purkinje effect): bright lamps and fire keep their colour.
fn grade(rgb: vec3f) -> vec3f {
  var c = rgb;
  let luminance = dot(c, vec3f(0.2126, 0.7152, 0.0722));
  c *= mix(output.shadows.rgb, output.highlights.rgb, smoothstep(0.03, 0.7, luminance));
  c = max(mix(vec3f(luminance), c, output.shadows.w), vec3f(0.0));
  if (output.finish.w > 0.0) {
    let dim = 1.0 - smoothstep(0.02, 0.35, luminance);
    c = mix(c, vec3f(luminance) * vec3f(0.72, 0.86, 1.3), output.finish.w * dim * 0.75);
  }
  return c;
}

// Contrast-adaptive sharpening (AMD's CAS) of the HDR image around \`p\`, for a picture drawn at fewer
// pixels than the screen has and stretched to it: the four neighbours are taken away in a measure that
// shrinks where the contrast is already high, so edges sharpen without ringing. The measure is worked
// out on luminance squeezed into 0 to 1, as CAS expects; the sharpening itself is on the linear colour.
fn sharpened(p: vec2i, amount: f32) -> vec3f {
  let last = vec2i(textureDimensions(hdr)) - 1;
  let c = textureLoad(hdr, p, 0).rgb;
  let n = textureLoad(hdr, clamp(p + vec2i(0, -1), vec2i(0), last), 0).rgb;
  let s = textureLoad(hdr, clamp(p + vec2i(0, 1), vec2i(0), last), 0).rgb;
  let e = textureLoad(hdr, clamp(p + vec2i(1, 0), vec2i(0), last), 0).rgb;
  let w = textureLoad(hdr, clamp(p + vec2i(-1, 0), vec2i(0), last), 0).rgb;
  let weights = vec3f(0.2126, 0.7152, 0.0722);
  let l5 = vec4f(dot(n, weights), dot(s, weights), dot(e, weights), dot(w, weights));
  let lc = dot(c, weights);
  let q = l5 / (1.0 + l5);
  let qc = lc / (1.0 + lc);
  let lo = min(qc, min(min(q.x, q.y), min(q.z, q.w)));
  let hi = max(qc, max(max(q.x, q.y), max(q.z, q.w)));
  let amp = sqrt(clamp(min(lo, 1.0 - hi) / max(hi, 1e-4), 0.0, 1.0));
  let lobe = -amp * mix(0.125, 0.2, amount);
  return max((c + (n + s + e + w) * lobe) / (1.0 + 4.0 * lobe), vec3f(0.0));
}

// Whether there is an edge at p, 0 to 1, by the depths w pixels to each side of it, and the view
// distance of the nearest thing among them: a silhouette or occluding edge bends depth sharply across
// the pixel; a smooth surface, even a long slope seen at a low angle, only changes its rate.
fn inkEdge(p: vec2i, w: i32, mid: f32) -> vec2f {
  let l = viewDepth(p + vec2i(-w, 0));
  let r = viewDepth(p + vec2i(w, 0));
  let d = viewDepth(p + vec2i(0, w));
  let u = viewDepth(p + vec2i(0, -w));
  let jump = max(abs(l + r - 2.0 * mid), abs(d + u - 2.0 * mid)) / mid;
  let threshold = output.ink.z;
  return vec2f(smoothstep(threshold, threshold * 1.6, jump), min(mid, min(min(l, r), min(d, u))));
}

fn toSrgb(c: vec3f) -> vec3f {
  let lo = c * 12.92;
  let hi = 1.055 * pow(c, vec3f(1.0 / 2.4)) - 0.055;
  return select(hi, lo, c <= vec3f(0.0031308));
}

@fragment
fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = vec2i(pos.xy) - vec2i(output.clip.zw);
  var c: vec3f;
  if (output.beams.y > 0.0) {
    c = sharpened(p, output.beams.y);
  } else {
    c = textureLoad(hdr, p, 0).rgb;
  }
  let uv = (vec2f(p) + 0.5) / vec2f(textureDimensions(hdr));
  if (output.params.z > 0.0) {
    let waver = output.beams.w;
    let base = i32(max(1.0, round(output.params.w)));
    // The taps are first taken as far out as the pen's line ever reaches: a pixel with no edge there
    // has none however the pen goes, and that is nearly all of them. Every distance the taps are taken
    // at is the ink's own plus a multiple of two pixels: a tree fading out near the camera is dithered
    // away pixel by pixel, and taps an odd step from the ink's would see an edge at every one of them.
    let stray = PEN_STRAY * output.params.w * waver;
    let reach = base + select(0, 2 + 2 * i32(ceil(stray * 0.5)), waver > 0.0);
    let mid = viewDepth(p);
    var found = inkEdge(p, reach, mid);
    var pressure = 1.0;
    if (found.x > 0.0 && waver > 0.0) {
      if (found.y > output.ink.w) {
        // Too far off for a line at all.
        found.x = 0.0;
      } else if (found.y > output.ink.w * INK_FADE_FROM) {
        // Far enough that the line is fading: an even one, as nothing of the pen's hand would show.
        found = inkEdge(p, base, mid);
      } else {
        // A pen's line: it strays a little from the edge it follows, swells and thins along its way,
        // and now and then runs dry.
        let ray = normalize(output.rayCorner.xyz + output.rayAcross.xyz * uv.x + output.rayDown.xyz * uv.y);
        let slow = penNoise(ray, PEN_SLOW);
        let swell = penNoise(ray, PEN_SWELL).a;
        pressure = mix(1.0, smoothstep(0.2, 0.5, slow.b), waver);
        let q = p + vec2i(round((slow.rg - 0.5) * 2.0 * stray));
        found = inkEdge(q, base + select(0, 2, swell > 1.0 - PEN_SWELLS * waver), viewDepth(q));
      }
    }
    let edge = found.x;
    let nearest = found.y;
    // Smoke has no depth to find an edge in: the lines of what is behind it would show through it, so
    // they fade by how much of the pixel the smoke covers.
    var clear = 1.0;
    if (edge > 0.0 && output.beams.z > 0.0) {
      clear = 1.0 - textureSampleLevel(smoke, linearSampler, uv, 0.0).a;
    }
    // The line is the nearer thing's: it is at its heaviest close by and has faded away by the ink's
    // distance, so far woods and ridges are not a tangle of lines.
    let weight = (1.0 - smoothstep(output.ink.w * INK_FADE_FROM, output.ink.w, nearest)) * mix(1.0, INK_NEAR_WEIGHT, 1.0 - smoothstep(0.0, INK_NEAR, nearest));
    // The line darkens what it is drawn over, so the hue carries through.
    c = mix(c, c * output.ink.x, min(edge * output.ink.y * clear * pressure * weight, 1.0));
  }
  if (output.finish.y > 0.0) {
    c += textureSampleLevel(shafts, linearSampler, uv, 0.0).rgb * output.finish.y;
  }
  if (output.beams.x > 0.0) {
    c += textureSampleLevel(beams, linearSampler, uv, 0.0).rgb;
  }
  c *= output.params.y * select(1.0, adaptation[1], output.highlights.w > 0.5);
  if (output.finish.x > 0.0) {
    c = whiteHot(c, output.finish.x);
  }
  c = grade(c);
  if (output.params.x == 1.0) {
    c = neutral(c);
  }
  if (output.finish.z > 0.0) {
    let off = (uv - 0.5) * vec2f(f32(textureDimensions(hdr).x) / f32(textureDimensions(hdr).y), 1.0);
    c *= 1.0 - output.finish.z * smoothstep(0.35, 1.05, length(off));
  }
  return vec4f(toSrgb(clamp(c, vec3f(0.0), vec3f(1.0))), 1.0);
}
`
}
