/**
 * The last pass, from the resolved HDR image to the canvas: ink outlines (optional), then tone mapping
 * and the linear-to-sRGB encode. `multisampled` says whether the scene depth it reads for the ink is.
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
}

@group(0) @binding(0) var<uniform> output: Output;
@group(0) @binding(1) var hdr: texture_2d<f32>;
@group(0) @binding(2) var depth: ${multisampled ? 'texture_depth_multisampled_2d' : 'texture_depth_2d'};

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

fn toSrgb(c: vec3f) -> vec3f {
  let lo = c * 12.92;
  let hi = 1.055 * pow(c, vec3f(1.0 / 2.4)) - 0.055;
  return select(hi, lo, c <= vec3f(0.0031308));
}

@fragment
fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = vec2i(pos.xy);
  var c = textureLoad(hdr, p, 0).rgb;
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
  c *= output.params.y;
  if (output.params.x == 1.0) {
    c = neutral(c);
  }
  return vec4f(toSrgb(clamp(c, vec3f(0.0), vec3f(1.0))), 1.0);
}
`
}
