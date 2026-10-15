/**
 * A half-width, half-height target for effects whose cost is overdraw (smoke), and the pass that lays it
 * back over the image. The target holds premultiplied colour and coverage, cleared to nothing, so over
 * the scene it is `colour + scene * (1 - coverage)`.
 */

const WGSL = /* wgsl */ `
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let uv = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
  return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let size = vec2f(textureDimensions(src)) * 2.0;
  return textureSampleLevel(src, smp, pos.xy / size, 0.0);
}
`

export class HalfResolution {
  texture: GPUTexture | null = null
  view: GPUTextureView | null = null
  private group: GPUBindGroup | null = null
  private pipeline: GPURenderPipeline | null = null
  private pipelineKey = ''
  private readonly layout: GPUBindGroupLayout
  private readonly sampler: GPUSampler
  private readonly module: GPUShaderModule

  constructor(private readonly device: GPUDevice, private readonly format: GPUTextureFormat) {
    this.layout = device.createBindGroupLayout({
      label: 'half resolution',
      entries: [
        { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'float' } },
        { binding: 1, visibility: GPUShaderStage.FRAGMENT, sampler: { type: 'filtering' } },
      ],
    })
    this.sampler = device.createSampler({ magFilter: 'linear', minFilter: 'linear' })
    this.module = device.createShaderModule({ label: 'half resolution', code: WGSL })
  }

  /** The target for a screen of `width`×`height`; made on first use and when the size changes. */
  ensure(width: number, height: number) {
    const w = Math.max(1, Math.ceil(width / 2))
    const h = Math.max(1, Math.ceil(height / 2))
    if (this.texture && this.texture.width === w && this.texture.height === h) return
    this.texture?.destroy()
    this.texture = this.device.createTexture({
      label: 'half resolution',
      size: [w, h],
      format: this.format,
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    })
    this.view = this.texture.createView()
    this.group = this.device.createBindGroup({
      label: 'half resolution',
      layout: this.layout,
      entries: [{ binding: 0, resource: this.view }, { binding: 1, resource: this.sampler }],
    })
  }

  /** Releases the target, for a resize; `ensure` makes the next one. */
  release() {
    this.texture?.destroy()
    this.texture = null
    this.view = null
    this.group = null
  }

  /** Records the draw that blends the target over the pass's colour attachment. */
  composite(pass: GPURenderPassEncoder, sampleCount: number, depthFormat: GPUTextureFormat) {
    const key = `${sampleCount}|${depthFormat}`
    if (this.pipelineKey !== key) {
      this.pipelineKey = key
      this.pipeline = this.device.createRenderPipeline({
        label: 'half resolution composite',
        layout: this.device.createPipelineLayout({ bindGroupLayouts: [this.layout] }),
        vertex: { module: this.module, entryPoint: 'vs' },
        fragment: {
          module: this.module,
          entryPoint: 'fs',
          targets: [{
            format: this.format,
            blend: {
              color: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' },
              alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' },
            },
          }],
        },
        // The effects pass holds the scene's depth read-only, so the pipeline has to declare it.
        depthStencil: { format: depthFormat, depthWriteEnabled: false, depthCompare: 'always' },
        multisample: { count: sampleCount },
      })
    }
    pass.setPipeline(this.pipeline!)
    pass.setBindGroup(0, this.group!)
    pass.draw(3)
  }
}
