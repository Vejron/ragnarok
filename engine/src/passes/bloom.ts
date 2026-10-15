/** Bloom: the bright parts of the HDR image, blurred through a chain of halving mips and added back. */

const FORMAT: GPUTextureFormat = 'rgba16float'
const MAX_LEVELS = 6

const WGSL = /* wgsl */ `
// threshold, knee, upsample radius, unused.
@group(0) @binding(0) var<uniform> p: vec4f;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let uv = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
  return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn tap(uv: vec2f) -> vec3f {
  return textureSampleLevel(src, smp, uv, 0.0).rgb;
}

// The 13-tap downsample from "Next Generation Post Processing in Call of Duty": no shimmer on small bright things.
fn down(uv: vec2f, t: vec2f) -> vec3f {
  let a = tap(uv + t * vec2f(-2.0, -2.0));
  let b = tap(uv + t * vec2f(0.0, -2.0));
  let c = tap(uv + t * vec2f(2.0, -2.0));
  let d = tap(uv + t * vec2f(-2.0, 0.0));
  let e = tap(uv);
  let f = tap(uv + t * vec2f(2.0, 0.0));
  let g = tap(uv + t * vec2f(-2.0, 2.0));
  let h = tap(uv + t * vec2f(0.0, 2.0));
  let i = tap(uv + t * vec2f(2.0, 2.0));
  let j = tap(uv + t * vec2f(-1.0, -1.0));
  let k = tap(uv + t * vec2f(1.0, -1.0));
  let l = tap(uv + t * vec2f(-1.0, 1.0));
  let m = tap(uv + t * vec2f(1.0, 1.0));
  return e * 0.125 + (a + c + g + i) * 0.03125 + (b + d + f + h) * 0.0625 + (j + k + l + m) * 0.125;
}

@fragment
fn prefilter(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let size = vec2f(textureDimensions(src));
  let uv = pos.xy * 2.0 / size;
  // Clamp so one blinding pixel cannot smear across the screen.
  var c = min(down(uv, 1.0 / size), vec3f(64.0));
  let peak = max(c.r, max(c.g, c.b));
  let knee = max(p.y, 1e-4);
  let soft = clamp(peak - p.x + knee, 0.0, 2.0 * knee);
  let gain = max(soft * soft / (4.0 * knee), peak - p.x) / max(peak, 1e-4);
  return vec4f(c * max(gain, 0.0), 1.0);
}

@fragment
fn downsample(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let size = vec2f(textureDimensions(src));
  let uv = pos.xy * 2.0 / size;
  return vec4f(down(uv, 1.0 / size), 1.0);
}

// A 3x3 tent over the smaller mip, added onto the one it is drawn into.
@fragment
fn upsample(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let size = vec2f(textureDimensions(src));
  let uv = pos.xy * 0.5 / size;
  let t = p.z / size;
  var c = tap(uv) * 4.0;
  c += (tap(uv + vec2f(-t.x, 0.0)) + tap(uv + vec2f(t.x, 0.0)) + tap(uv + vec2f(0.0, -t.y)) + tap(uv + vec2f(0.0, t.y))) * 2.0;
  c += tap(uv + vec2f(-t.x, -t.y)) + tap(uv + vec2f(t.x, -t.y)) + tap(uv + vec2f(-t.x, t.y)) + tap(uv + vec2f(t.x, t.y));
  return vec4f(c / 16.0, 1.0);
}
`

export class Bloom {
  /** The blurred bright image, at half the size of the source. Read it with `view`. */
  private texture: GPUTexture | null = null
  private levels = 0
  private readonly layout: GPUBindGroupLayout
  private readonly uniform: GPUBuffer
  private readonly sampler: GPUSampler
  private readonly prefilter: GPURenderPipeline
  private readonly downsample: GPURenderPipeline
  private readonly upsample: GPURenderPipeline
  private readonly data = new Float32Array(4)
  private views: GPUTextureView[] = []
  private downGroups: GPUBindGroup[] = []
  private upGroups: GPUBindGroup[] = []
  private firstGroup: GPUBindGroup | null = null

  constructor(private readonly device: GPUDevice) {
    this.layout = device.createBindGroupLayout({
      label: 'bloom',
      entries: [
        { binding: 0, visibility: GPUShaderStage.FRAGMENT, buffer: { type: 'uniform' } },
        { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'float' } },
        { binding: 2, visibility: GPUShaderStage.FRAGMENT, sampler: { type: 'filtering' } },
      ],
    })
    const module = device.createShaderModule({ label: 'bloom', code: WGSL })
    const layout = device.createPipelineLayout({ bindGroupLayouts: [this.layout] })
    const make = (entryPoint: string, additive: boolean) =>
      device.createRenderPipeline({
        label: `bloom ${entryPoint}`,
        layout,
        vertex: { module, entryPoint: 'vs' },
        fragment: {
          module,
          entryPoint,
          targets: [{
            format: FORMAT,
            blend: additive
              ? { color: { srcFactor: 'one', dstFactor: 'one' }, alpha: { srcFactor: 'zero', dstFactor: 'one' } }
              : undefined,
          }],
        },
        primitive: { topology: 'triangle-list' },
      })
    this.prefilter = make('prefilter', false)
    this.downsample = make('downsample', false)
    this.upsample = make('upsample', true)
    this.uniform = device.createBuffer({ size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.sampler = device.createSampler({ magFilter: 'linear', minFilter: 'linear', addressModeU: 'clamp-to-edge', addressModeV: 'clamp-to-edge' })
  }

  /** The finished bloom, for the output pass to add. Valid after `resize`. */
  get view(): GPUTextureView {
    return this.views[0]!
  }

  /** (Re)builds the mip chain for a source of `width`×`height` and binds `source` as its input. */
  resize(width: number, height: number, source: GPUTexture) {
    this.texture?.destroy()
    const w = Math.max(1, width >> 1)
    const h = Math.max(1, height >> 1)
    this.levels = Math.max(1, Math.min(MAX_LEVELS, Math.floor(Math.log2(Math.min(w, h)))))
    this.texture = this.device.createTexture({
      label: 'bloom',
      size: [w, h],
      mipLevelCount: this.levels,
      format: FORMAT,
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    })
    this.views = []
    for (let i = 0; i < this.levels; i++) {
      this.views.push(this.texture.createView({ baseMipLevel: i, mipLevelCount: 1 }))
    }
    const group = (view: GPUTextureView) =>
      this.device.createBindGroup({
        layout: this.layout,
        entries: [
          { binding: 0, resource: { buffer: this.uniform } },
          { binding: 1, resource: view },
          { binding: 2, resource: this.sampler },
        ],
      })
    this.firstGroup = group(source.createView())
    // downGroups[i] reads mip i and writes mip i + 1; upGroups[i] reads mip i + 1 and writes mip i.
    this.downGroups = this.views.map(group)
    this.upGroups = this.views.slice(1).map(group)
  }

  /** `threshold` and `knee` are in linear HDR units, `radius` widens the upsample tent (1 is the default). */
  encode(encoder: GPUCommandEncoder, threshold: number, knee: number, radius: number) {
    if (!this.texture) return
    this.data[0] = threshold
    this.data[1] = knee
    this.data[2] = radius
    this.device.queue.writeBuffer(this.uniform, 0, this.data)
    const run = (pipeline: GPURenderPipeline, group: GPUBindGroup, target: GPUTextureView, load: boolean) => {
      const pass = encoder.beginRenderPass({
        label: 'bloom',
        colorAttachments: [{ view: target, loadOp: load ? 'load' : 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 1 } }],
      })
      pass.setPipeline(pipeline)
      pass.setBindGroup(0, group)
      pass.draw(3)
      pass.end()
    }
    run(this.prefilter, this.firstGroup!, this.views[0]!, false)
    for (let i = 1; i < this.levels; i++) run(this.downsample, this.downGroups[i - 1]!, this.views[i]!, false)
    for (let i = this.levels - 2; i >= 0; i--) run(this.upsample, this.upGroups[i]!, this.views[i]!, true)
  }

  destroy() {
    this.texture?.destroy()
    this.uniform.destroy()
  }
}
