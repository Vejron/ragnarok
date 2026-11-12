/**
 * Light shafts (god rays), after Mitchell, "Volumetric Light Scattering as a Post-Process" (GPU Gems 3):
 * at a quarter of the screen's size, a mask of the sky near the light (whatever stands in front of it
 * is black), then a blur of that mask along lines toward the light. Gaps in trees, buildings and cloud
 * edges become shafts. The output pass adds the result.
 */

const FORMAT: GPUTextureFormat = 'rgba16float'
/** Samples along each line toward the light. */
const STEPS = 32

function shaftsWgsl(multisampled: boolean) {
  return /* wgsl */ `
struct Shafts {
  // The light's place on screen (uv), the screen's aspect (width / height), the shafts' strength (0 when off).
  light: vec4f,
  // How far toward the light a pixel gathers (share of the way), what each step keeps of the one before.
  reach: vec4f,
}

@group(0) @binding(0) var<uniform> shafts: Shafts;
@group(0) @binding(1) var hdr: texture_2d<f32>;
@group(0) @binding(2) var depth: ${multisampled ? 'texture_depth_multisampled_2d' : 'texture_depth_2d'};
@group(0) @binding(3) var mask: texture_2d<f32>;
@group(0) @binding(4) var smp: sampler;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let uv = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
  return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
}

// The sky's light round the light source, four taps of the 4×4 pixels under each texel: anything in
// front of the sky (depth above 0, reversed) is black. Very bright pixels (the sun's disc) are held
// down so the shafts take the sky's colour rather than a blinding streak.
@fragment
fn maskSky(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let full = vec2i(textureDimensions(hdr));
  let base = vec2i(pos.xy) * 4;
  var light = vec3f(0.0);
  for (var i = 0; i < 4; i++) {
    let p = min(base + vec2i(1 + (i & 1) * 2, 1 + (i >> 1) * 2), full - 1);
    if (textureLoad(depth, p, 0) == 0.0) {
      light += min(textureLoad(hdr, p, 0).rgb, vec3f(3.0)) * 0.25;
    }
  }
  let uv = (vec2f(base) + 2.0) / vec2f(full);
  let off = (uv - shafts.light.xy) * vec2f(shafts.light.z, 1.0);
  return vec4f(light * exp(-dot(off, off) * 5.0), 1.0);
}

// Gathers the mask along the line from this pixel toward the light, each step weaker than the one
// before, starting at a jittered point so the steps do not band.
@fragment
fn blur(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let size = vec2f(textureDimensions(mask));
  let uv = pos.xy / size;
  let delta = (uv - shafts.light.xy) * shafts.reach.x / ${STEPS}.0;
  let jitter = fract(52.9829189 * fract(dot(pos.xy, vec2f(0.06711056, 0.00583715))));
  var p = uv - delta * jitter;
  var weight = 1.0;
  var sum = vec3f(0.0);
  for (var i = 0; i < ${STEPS}; i++) {
    sum += textureSampleLevel(mask, smp, p, 0.0).rgb * weight;
    weight *= shafts.reach.y;
    p -= delta;
  }
  return vec4f(sum * (shafts.light.w / ${STEPS}.0), 1.0);
}
`
}

/** Where the light is and how strong its shafts are this frame. */
export interface ShaftLight {
  /** The light's place on screen, uv (0 to 1, y down); may be off the screen. */
  u: number
  v: number
  /** Overall strength: the scene's, times how much the light counts (sun or moon), times how near the screen it is. */
  strength: number
  length: number
  decay: number
}

export class LightShafts {
  private maskTexture: GPUTexture | null = null
  private raysTexture: GPUTexture | null = null
  private maskGroup: GPUBindGroup | null = null
  private blurGroup: GPUBindGroup | null = null
  private readonly maskPipeline: GPURenderPipeline
  private readonly blurPipeline: GPURenderPipeline
  private readonly uniform: GPUBuffer
  private readonly data = new Float32Array(8)
  private readonly sampler: GPUSampler
  /** The shafts' light, a quarter of the screen's size, for the output pass. Valid after `resize`. */
  view: GPUTextureView | null = null

  constructor(private readonly device: GPUDevice, multisampled: boolean) {
    const module = device.createShaderModule({ label: 'shafts', code: shaftsWgsl(multisampled) })
    const make = (entryPoint: string) => device.createRenderPipeline({
      label: `shafts ${entryPoint}`,
      layout: 'auto',
      vertex: { module, entryPoint: 'vs' },
      fragment: { module, entryPoint, targets: [{ format: FORMAT }] },
      primitive: { topology: 'triangle-list' },
    })
    this.maskPipeline = make('maskSky')
    this.blurPipeline = make('blur')
    this.uniform = device.createBuffer({ label: 'shafts', size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.sampler = device.createSampler({ magFilter: 'linear', minFilter: 'linear', addressModeU: 'clamp-to-edge', addressModeV: 'clamp-to-edge' })
  }

  /** (Re)makes the quarter-size targets for a screen of `width`×`height`, reading `hdr` and `depth`. */
  resize(width: number, height: number, hdr: GPUTexture, depth: GPUTexture) {
    this.maskTexture?.destroy()
    this.raysTexture?.destroy()
    const size = [Math.max(1, Math.ceil(width / 4)), Math.max(1, Math.ceil(height / 4))]
    const usage = GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING
    this.maskTexture = this.device.createTexture({ label: 'shafts mask', size, format: FORMAT, usage })
    this.raysTexture = this.device.createTexture({ label: 'shafts', size, format: FORMAT, usage })
    this.view = this.raysTexture.createView()
    this.maskGroup = this.device.createBindGroup({
      layout: this.maskPipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: this.uniform } },
        { binding: 1, resource: hdr.createView() },
        { binding: 2, resource: depth.createView() },
      ],
    })
    this.blurGroup = this.device.createBindGroup({
      layout: this.blurPipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: this.uniform } },
        { binding: 3, resource: this.maskTexture.createView() },
        { binding: 4, resource: this.sampler },
      ],
    })
  }

  /** Draws the shafts for `light`, or clears them when `light` is null. `aspect` is the screen's width over its height. */
  encode(encoder: GPUCommandEncoder, light: ShaftLight | null, aspect: number, timed: (name: string) => GPURenderPassTimestampWrites | undefined) {
    if (!this.maskTexture || !this.raysTexture) return
    if (!light) {
      encoder.beginRenderPass({
        label: 'shafts off',
        colorAttachments: [{ view: this.view!, loadOp: 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 1 } }],
      }).end()
      return
    }
    const d = this.data
    d[0] = light.u
    d[1] = light.v
    d[2] = aspect
    d[3] = light.strength
    d[4] = light.length
    d[5] = light.decay
    this.device.queue.writeBuffer(this.uniform, 0, d)
    const mask = encoder.beginRenderPass({
      label: 'shafts mask',
      colorAttachments: [{ view: this.maskTexture.createView(), loadOp: 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 1 } }],
      timestampWrites: timed('shaftsMask'),
    })
    mask.setPipeline(this.maskPipeline)
    mask.setBindGroup(0, this.maskGroup!)
    mask.draw(3)
    mask.end()
    const blur = encoder.beginRenderPass({
      label: 'shafts blur',
      colorAttachments: [{ view: this.view!, loadOp: 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 1 } }],
      timestampWrites: timed('shaftsBlur'),
    })
    blur.setPipeline(this.blurPipeline)
    blur.setBindGroup(0, this.blurGroup!)
    blur.draw(3)
    blur.end()
  }

  destroy() {
    this.maskTexture?.destroy()
    this.raysTexture?.destroy()
    this.uniform.destroy()
  }
}

/** The shafts' WGSL, for checking it outside a browser. */
export const SHAFTS_WGSL = shaftsWgsl
