import type { PixelData, Texture } from '../scene/texture'

interface TextureSlot {
  texture: GPUTexture
  view: GPUTextureView
  version: number
}

const BLIT_WGSL = /* wgsl */ `
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var linearSampler: sampler;

struct Out {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
  let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
  return Out(vec4f(p * 2.0 - 1.0, 0.0, 1.0), vec2f(p.x, 1.0 - p.y));
}

@fragment
fn fs(in: Out) -> @location(0) vec4f {
  return textureSampleLevel(source, linearSampler, in.uv, 0.0);
}
`

/**
 * GPU copies of textures, uploaded on first use and re-uploaded when their version changes, with mip
 * chains made by repeated half-size blits. Also hands out samplers, one per distinct setting.
 */
export class TextureStore {
  /** A 1×1 white texture: what an unset colour map samples. */
  readonly white: GPUTextureView
  /** A 1×1 texture holding the +Y normal, for an unset normal map. */
  readonly flat: GPUTextureView
  private readonly samplers = new Map<string, GPUSampler>()
  private readonly blitModule: GPUShaderModule
  private readonly blitPipelines = new Map<GPUTextureFormat, GPURenderPipeline>()
  private readonly blitSampler: GPUSampler

  constructor(private readonly device: GPUDevice) {
    this.white = this.solid([255, 255, 255, 255], 'rgba8unorm')
    this.flat = this.solid([128, 255, 128, 255], 'rgba8unorm')
    this.blitModule = device.createShaderModule({ label: 'mip blit', code: BLIT_WGSL })
    this.blitSampler = device.createSampler({ minFilter: 'linear', magFilter: 'linear' })
  }

  /** The texture's view, uploading it first if it is new or changed. */
  view(texture: Texture): GPUTextureView {
    let slot = texture.gpu as TextureSlot | undefined
    if (slot && slot.version === texture.version) return slot.view
    if (texture.disposed) {
      slot?.texture.destroy()
      texture.gpu = undefined
      return this.white
    }
    const format: GPUTextureFormat = texture.srgb ? 'rgba8unorm-srgb' : 'rgba8unorm'
    const width = texture.width
    const height = texture.height
    const mipLevelCount = texture.mipmaps ? Math.floor(Math.log2(Math.max(width, height))) + 1 : 1
    if (!slot || slot.texture.width !== width || slot.texture.height !== height || slot.texture.format !== format || slot.texture.mipLevelCount !== mipLevelCount) {
      slot?.texture.destroy()
      const gpu = this.device.createTexture({
        label: `texture ${texture.id}`,
        size: [width, height],
        format,
        mipLevelCount,
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
      })
      slot = { texture: gpu, view: gpu.createView(), version: -1 }
      texture.gpu = slot
    }
    slot.version = texture.version
    const source = texture.source
    if ('data' in source && source.data instanceof Uint8Array) {
      const pixels = source as PixelData
      this.device.queue.writeTexture({ texture: slot.texture }, pixels.data as Uint8Array<ArrayBuffer>, { bytesPerRow: width * 4 }, [width, height])
    } else {
      this.device.queue.copyExternalImageToTexture({ source: source as GPUCopyExternalImageSource }, { texture: slot.texture }, [width, height])
    }
    if (mipLevelCount > 1) this.generateMips(slot.texture)
    return slot.view
  }

  sampler(texture: Texture | null): GPUSampler {
    const filter = texture?.filter ?? 'linear'
    const wrap = texture?.wrap ?? 'repeat'
    const mip = texture?.mipmaps ?? false
    // Anisotropy needs linear filtering everywhere.
    const anisotropy = filter === 'linear' && mip ? Math.max(1, Math.min(16, texture?.anisotropy ?? 1)) : 1
    const key = `${filter}|${wrap}|${mip}|${anisotropy}`
    let sampler = this.samplers.get(key)
    if (!sampler) {
      const mode: GPUAddressMode = wrap === 'repeat' ? 'repeat' : 'clamp-to-edge'
      sampler = this.device.createSampler({
        magFilter: filter,
        minFilter: filter,
        mipmapFilter: mip ? 'linear' : 'nearest',
        addressModeU: mode,
        addressModeV: mode,
        maxAnisotropy: anisotropy,
      })
      this.samplers.set(key, sampler)
    }
    return sampler
  }

  private generateMips(texture: GPUTexture) {
    let pipeline = this.blitPipelines.get(texture.format)
    if (!pipeline) {
      pipeline = this.device.createRenderPipeline({
        label: `mip blit ${texture.format}`,
        layout: 'auto',
        vertex: { module: this.blitModule, entryPoint: 'vs' },
        fragment: { module: this.blitModule, entryPoint: 'fs', targets: [{ format: texture.format }] },
        primitive: { topology: 'triangle-list' },
      })
      this.blitPipelines.set(texture.format, pipeline)
    }
    const encoder = this.device.createCommandEncoder({ label: 'mips' })
    for (let level = 1; level < texture.mipLevelCount; level++) {
      const group = this.device.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: texture.createView({ baseMipLevel: level - 1, mipLevelCount: 1 }) },
          { binding: 1, resource: this.blitSampler },
        ],
      })
      const pass = encoder.beginRenderPass({
        colorAttachments: [{ view: texture.createView({ baseMipLevel: level, mipLevelCount: 1 }), loadOp: 'clear', storeOp: 'store' }],
      })
      pass.setPipeline(pipeline)
      pass.setBindGroup(0, group)
      pass.draw(3)
      pass.end()
    }
    this.device.queue.submit([encoder.finish()])
  }

  private solid(rgba: number[], format: GPUTextureFormat): GPUTextureView {
    const texture = this.device.createTexture({ size: [1, 1], format, usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST })
    this.device.queue.writeTexture({ texture }, new Uint8Array(rgba), { bytesPerRow: 4 }, [1, 1])
    return texture.createView()
  }
}
