/**
 * The sun in a lens: a starburst of glare on it and a few ghosts strung along the line from it through
 * the middle of the screen. Drawn additively over the finished picture, in the output pass. How much
 * of the sun is clear of whatever stands in front of it is worked out in the vertex stage, from 25
 * depth taps round it, so the flare winks out behind a tree without any pass of its own.
 */

function flareWgsl(multisampled: boolean) {
  return /* wgsl */ `
struct Flare {
  // The sun on screen (normalised device coordinates), the screen's aspect (width / height), strength (0 for none).
  light: vec4f,
  // The sun's colour (rgb, brightest channel 1), the sun's radius on screen as a share of its height.
  color: vec4f,
}

@group(0) @binding(0) var<uniform> flare: Flare;
@group(0) @binding(1) var depth: ${multisampled ? 'texture_depth_multisampled_2d' : 'texture_depth_2d'};

// Per element: where along the line from the middle of the screen it sits (1 on the sun, negative past
// the middle), its size as a share of the screen's height, its tint and brightness, its shape (0 the
// glare, 1 a soft disc, 2 a ring).
const PLACES = array<f32, 7>(1.0, 0.55, 0.22, -0.28, -0.62, -1.0, -1.35);
const SIZES = array<f32, 7>(0.34, 0.05, 0.025, 0.085, 0.16, 0.06, 0.11);
const TINTS = array<vec4f, 7>(
  vec4f(1.0, 0.96, 0.88, 0.42),
  vec4f(1.0, 0.72, 0.42, 0.09),
  vec4f(0.6, 1.0, 0.65, 0.08),
  vec4f(0.5, 0.72, 1.0, 0.06),
  vec4f(0.85, 0.62, 1.0, 0.05),
  vec4f(1.0, 0.82, 0.5, 0.07),
  vec4f(0.55, 0.9, 1.0, 0.035),
);
const SHAPES = array<u32, 7>(0u, 1u, 1u, 1u, 2u, 1u, 2u);

struct FlareOut {
  @builtin(position) clip: vec4f,
  @location(0) local: vec2f,
  @location(1) tint: vec4f,
  @location(2) @interpolate(flat) shape: u32,
}

// The share of 5 × 5 depth taps round the sun that see the sky (reversed depth: 0 is nothing drawn).
fn clear() -> f32 {
  let size = vec2f(textureDimensions(depth));
  let centre = vec2f(flare.light.x * 0.5 + 0.5, 0.5 - flare.light.y * 0.5) * size;
  let step = flare.color.w * size.y * 0.5;
  var seen = 0.0;
  for (var y = -2; y <= 2; y++) {
    for (var x = -2; x <= 2; x++) {
      let p = vec2i(centre + vec2f(f32(x), f32(y)) * step);
      if (all(p >= vec2i(0)) && all(p < vec2i(size)) && textureLoad(depth, p, 0) == 0.0) {
        seen += 1.0;
      }
    }
  }
  return seen / 25.0;
}

@vertex
fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> FlareOut {
  var places = PLACES;
  var sizes = SIZES;
  var tints = TINTS;
  var shapes = SHAPES;
  let corners = array<vec2f, 6>(vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0), vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0));
  let corner = corners[v];
  var out: FlareOut;
  let strength = flare.light.w * clear();
  if (strength <= 0.001) {
    out.clip = vec4f(2.0, 2.0, 0.0, 1.0);
    return out;
  }
  let centre = flare.light.xy * places[i];
  let size = sizes[i] * 2.0;
  out.clip = vec4f(centre + corner * vec2f(size / flare.light.z, size), 0.0, 1.0);
  out.local = corner;
  let t = tints[i];
  out.tint = vec4f(t.rgb * flare.color.rgb, t.a * strength);
  out.shape = shapes[i];
  return out;
}

@fragment
fn fs(in: FlareOut) -> @location(0) vec4f {
  let r = length(in.local);
  var a = 0.0;
  if (in.shape == 0u) {
    // A hot core, and six long thin streaks and six shorter ones turned between them.
    let angle = atan2(in.local.y, in.local.x);
    let long = pow(abs(cos(angle * 3.0)), 90.0) * exp(-r * 3.0);
    let short = pow(abs(cos(angle * 3.0 + 0.52)), 160.0) * exp(-r * 5.5) * 0.6;
    a = exp(-r * r * 30.0) * 1.4 + exp(-r * 4.5) * 0.25 + (long + short) * 0.8;
  } else if (in.shape == 1u) {
    a = smoothstep(1.0, 0.7, r) * (0.55 + 0.45 * r);
  } else {
    a = exp(-pow((r - 0.82) / 0.08, 2.0));
  }
  a *= smoothstep(1.0, 0.9, r);
  return vec4f(in.tint.rgb * a * in.tint.a, 0.0);
}
`
}

/** Where the sun is on screen and how strongly it flares this frame. */
export interface FlareLight {
  /** Normalised device coordinates: -1 to 1, y up. */
  x: number
  y: number
  strength: number
  /** The sun's colour, its brightest channel 1. */
  r: number
  g: number
  b: number
  /** The sun's radius on screen, as a share of the screen's height: how far apart the visibility taps are. */
  radius: number
}

export class LensFlarePass {
  private readonly pipeline: GPURenderPipeline
  private readonly uniform: GPUBuffer
  private readonly data = new Float32Array(8)
  private group: GPUBindGroup | null = null

  constructor(private readonly device: GPUDevice, format: GPUTextureFormat, multisampled: boolean) {
    const module = device.createShaderModule({ label: 'flare', code: flareWgsl(multisampled) })
    this.pipeline = device.createRenderPipeline({
      label: 'flare',
      layout: 'auto',
      vertex: { module, entryPoint: 'vs' },
      fragment: {
        module,
        entryPoint: 'fs',
        targets: [{ format, blend: { color: { srcFactor: 'one', dstFactor: 'one' }, alpha: { srcFactor: 'zero', dstFactor: 'one' } } }],
      },
      primitive: { topology: 'triangle-list' },
    })
    this.uniform = device.createBuffer({ label: 'flare', size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
  }

  /** Binds the scene's depth, after the render targets are (re)made. */
  resize(depth: GPUTexture) {
    this.group = this.device.createBindGroup({
      layout: this.pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: this.uniform } },
        { binding: 1, resource: depth.createView() },
      ],
    })
  }

  /** Draws the flare for `light` into `pass` (the output pass, after the picture). Returns the draw calls. */
  draw(pass: GPURenderPassEncoder, light: FlareLight, aspect: number): number {
    if (!this.group) return 0
    const d = this.data
    d[0] = light.x
    d[1] = light.y
    d[2] = aspect
    d[3] = light.strength
    d[4] = light.r
    d[5] = light.g
    d[6] = light.b
    d[7] = light.radius
    this.device.queue.writeBuffer(this.uniform, 0, d)
    pass.setPipeline(this.pipeline)
    pass.setBindGroup(0, this.group)
    pass.draw(6, 7)
    return 1
  }

  destroy() {
    this.uniform.destroy()
  }
}

export const FLARE_WGSL = flareWgsl
