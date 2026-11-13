/**
 * The last pass, from the resolved HDR image to the canvas: ink outlines (optional), bloom and light
 * shafts added, the exposure (fixed, or the eye's adaptation), colour grading, tone mapping, a
 * vignette and the linear-to-sRGB encode. `multisampled` says whether the scene depth it reads for the
 * ink is.
 */
export function outputWgsl(multisampled: boolean) {
  return /* wgsl */ `
struct Output {
  // tone mapping (0 none, 1 neutral), exposure, ink on (0 or 1), ink width in pixels.
  params: vec4f,
  // ink darken, ink strength, ink threshold, unused.
  ink: vec4f,
  // camera near, camera far, unused, unused.
  clip: vec4f,
  // bloom strength, light shafts' strength, vignette (0 to 1), Purkinje shift (0 to 1).
  bloom: vec4f,
  // Grading: what the shadows are multiplied by (rgb), saturation.
  shadows: vec4f,
  // What the highlights are multiplied by (rgb), whether the exposure is the eye's adaptation (0 or 1).
  highlights: vec4f,
  // Whether lamps' beams were drawn this frame (0 or 1), how much to sharpen (0 to 1), unused, unused.
  beams: vec4f,
}

@group(0) @binding(0) var<uniform> output: Output;
@group(0) @binding(1) var hdr: texture_2d<f32>;
@group(0) @binding(2) var depth: ${multisampled ? 'texture_depth_multisampled_2d' : 'texture_depth_2d'};
@group(0) @binding(3) var bloom: texture_2d<f32>;
@group(0) @binding(4) var bloomSampler: sampler;
@group(0) @binding(5) var shafts: texture_2d<f32>;
// The eye's adaptation (passes/exposure.ts): its exposure is element 1.
@group(0) @binding(6) var<storage, read> adaptation: array<f32, 4>;
@group(0) @binding(7) var beams: texture_2d<f32>;

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
  if (output.bloom.w > 0.0) {
    let dim = 1.0 - smoothstep(0.02, 0.35, luminance);
    c = mix(c, vec3f(luminance) * vec3f(0.72, 0.86, 1.3), output.bloom.w * dim * 0.75);
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

fn toSrgb(c: vec3f) -> vec3f {
  let lo = c * 12.92;
  let hi = 1.055 * pow(c, vec3f(1.0 / 2.4)) - 0.055;
  return select(hi, lo, c <= vec3f(0.0031308));
}

@fragment
fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = vec2i(pos.xy);
  var c: vec3f;
  if (output.beams.y > 0.0) {
    c = sharpened(p, output.beams.y);
  } else {
    c = textureLoad(hdr, p, 0).rgb;
  }
  if (output.params.z > 0.0) {
    // Ink (client/src/render/ink.ts): a silhouette or occluding edge bends depth sharply across the
    // pixel; a smooth surface, even a long slope seen at a low angle, only changes its rate.
    let w = i32(max(1.0, round(output.params.w)));
    let mid = viewDepth(p);
    let l = viewDepth(p + vec2i(-w, 0));
    let r = viewDepth(p + vec2i(w, 0));
    let d = viewDepth(p + vec2i(0, w));
    let u = viewDepth(p + vec2i(0, -w));
    let jump = max(abs(l + r - 2.0 * mid), abs(d + u - 2.0 * mid)) / mid;
    let threshold = output.ink.z;
    let edge = smoothstep(threshold, threshold * 1.6, jump);
    // The line darkens what it is drawn over, so the hue carries through.
    c = mix(c, c * output.ink.x, edge * output.ink.y);
  }
  let uv = (vec2f(p) + 0.5) / vec2f(textureDimensions(hdr));
  if (output.bloom.x > 0.0) {
    c += textureSampleLevel(bloom, bloomSampler, uv, 0.0).rgb * output.bloom.x;
  }
  if (output.bloom.y > 0.0) {
    c += textureSampleLevel(shafts, bloomSampler, uv, 0.0).rgb * output.bloom.y;
  }
  if (output.beams.x > 0.0) {
    c += textureSampleLevel(beams, bloomSampler, uv, 0.0).rgb;
  }
  c *= output.params.y * select(1.0, adaptation[1], output.highlights.w > 0.5);
  c = grade(c);
  if (output.params.x == 1.0) {
    c = neutral(c);
  }
  if (output.bloom.z > 0.0) {
    let off = (uv - 0.5) * vec2f(f32(textureDimensions(hdr).x) / f32(textureDimensions(hdr).y), 1.0);
    c *= 1.0 - output.bloom.z * smoothstep(0.35, 1.05, length(off));
  }
  return vec4f(toSrgb(clamp(c, vec3f(0.0), vec3f(1.0))), 1.0);
}
`
}
