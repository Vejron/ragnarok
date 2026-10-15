/** The last pass: tone mapping and the linear-to-sRGB encode, from the resolved HDR image to the canvas. */
export const OUTPUT_WGSL = /* wgsl */ `
struct Output {
  // tone mapping (0 none, 1 neutral), exposure, unused, unused.
  params: vec4f,
}

@group(0) @binding(0) var<uniform> output: Output;
@group(0) @binding(1) var hdr: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  // One triangle covering the screen.
  let uv = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
  return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
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
  var c = textureLoad(hdr, vec2u(pos.xy), 0).rgb * output.params.y;
  if (output.params.x == 1.0) {
    c = neutral(c);
  }
  return vec4f(toSrgb(clamp(c, vec3f(0.0), vec3f(1.0))), 1.0);
}
`
