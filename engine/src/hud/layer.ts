import type { TextureStore } from '../gpu/textures'
import type { Texture } from '../scene/texture'

/** Floats per sprite: centre xy, half size xy, rotation, pad, uv rect (4), colour rgba (4), pad (2). */
const STRIDE = 16

const HUD_WGSL = /* wgsl */ `
struct View { size: vec2f, scale: f32, pad: f32 }
@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var imageSampler: sampler;

struct Out {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) color: vec4f,
}

@vertex
fn vs(
  @builtin(vertex_index) i: u32,
  @location(0) centre: vec4f,
  @location(1) spin: vec4f,
  @location(2) uvRect: vec4f,
  @location(3) color: vec4f,
) -> Out {
  let corner = vec2f(f32(i & 1u), f32((i >> 1u) & 1u));
  let local = (corner * 2.0 - 1.0) * centre.zw;
  let c = cos(spin.x);
  let s = sin(spin.x);
  let p = centre.xy + vec2f(local.x * c - local.y * s, local.x * s + local.y * c);
  let ndc = vec2f(p.x / view.size.x * 2.0 - 1.0, 1.0 - p.y / view.size.y * 2.0);
  return Out(vec4f(ndc, 0.0, 1.0), mix(uvRect.xy, uvRect.zw, corner), color);
}

@fragment
fn fs(in: Out) -> @location(0) vec4f {
  let texel = textureSample(image, imageSampler, in.uv) * in.color;
  return vec4f(texel.rgb * texel.a, texel.a);
}
`

interface Run {
  texture: Texture | null
  first: number
  count: number
}

/**
 * Flat 2D sprites drawn over the finished frame, after tone mapping, straight onto the canvas: the game's
 * HUD. Positions are CSS pixels from the top-left. Fill it each frame between `begin` and the next
 * `Renderer.render`, which draws and empties it; sprites are drawn in the order given.
 *
 * Colours and textures are taken as they are, in sRGB, and blended like a web page: make HUD textures
 * with `srgb: false` so the GPU does not decode them to linear first.
 */
export class HudLayer {
  /** CSS pixel size of the view, from the last `begin`. */
  width = 1
  height = 1
  private data = new Float32Array(256 * STRIDE)
  private count = 0
  private readonly runs: Run[] = []

  /** Starts a frame's HUD on a view `width` × `height` CSS pixels. */
  begin(width: number, height: number) {
    this.width = width
    this.height = height
    this.count = 0
    this.runs.length = 0
  }

  get size() {
    return this.count
  }

  /**
   * A rectangle at `x`, `y` (top-left), `w` × `h`, in `color` (0xRRGGBB) at `alpha`, optionally textured
   * with `uv` (u0, v0, u1, v1) of `texture` and turned `rotation` radians (clockwise) about its centre.
   */
  quad(x: number, y: number, w: number, h: number, color = 0xffffff, alpha = 1, texture: Texture | null = null, uv?: readonly number[], rotation = 0) {
    this.centred(x + w / 2, y + h / 2, w, h, color, alpha, texture, uv, rotation)
  }

  /** Like `quad`, but placed by its centre. */
  centred(cx: number, cy: number, w: number, h: number, color = 0xffffff, alpha = 1, texture: Texture | null = null, uv?: readonly number[], rotation = 0) {
    if (alpha <= 0 || w === 0 || h === 0) return
    if ((this.count + 1) * STRIDE > this.data.length) {
      const grown = new Float32Array(this.data.length * 2)
      grown.set(this.data)
      this.data = grown
    }
    const o = this.count * STRIDE
    const d = this.data
    d[o] = cx
    d[o + 1] = cy
    d[o + 2] = w / 2
    d[o + 3] = h / 2
    d[o + 4] = rotation
    d[o + 8] = uv ? uv[0]! : 0
    d[o + 9] = uv ? uv[1]! : 0
    d[o + 10] = uv ? uv[2]! : 1
    d[o + 11] = uv ? uv[3]! : 1
    d[o + 12] = ((color >> 16) & 255) / 255
    d[o + 13] = ((color >> 8) & 255) / 255
    d[o + 14] = (color & 255) / 255
    d[o + 15] = alpha
    const last = this.runs[this.runs.length - 1]
    if (last && last.texture === texture) last.count++
    else this.runs.push({ texture, first: this.count, count: 1 })
    this.count++
  }

  /** A line `width` pixels thick from (`x0`, `y0`) to (`x1`, `y1`). */
  line(x0: number, y0: number, x1: number, y1: number, width: number, color = 0xffffff, alpha = 1) {
    const length = Math.hypot(x1 - x0, y1 - y0)
    if (length === 0) return
    this.centred((x0 + x1) / 2, (y0 + y1) / 2, length, width, color, alpha, null, undefined, Math.atan2(y1 - y0, x1 - x0))
  }

  /** @internal */
  get instances() {
    return this.data.subarray(0, this.count * STRIDE)
  }

  /** @internal */
  get batches(): readonly Run[] {
    return this.runs
  }
}

/** Draws a `HudLayer` onto the canvas at the end of the output pass. */
export class HudPass {
  private readonly pipeline: GPURenderPipeline
  private readonly viewBuffer: GPUBuffer
  private readonly viewGroup: GPUBindGroup
  private readonly imageLayout: GPUBindGroupLayout
  private instanceBuffer: GPUBuffer
  private readonly groups = new WeakMap<GPUTextureView, Map<GPUSampler, GPUBindGroup>>()
  private readonly viewData = new Float32Array(4)

  constructor(private readonly device: GPUDevice, format: GPUTextureFormat, private readonly textures: TextureStore) {
    const module = device.createShaderModule({ label: 'hud', code: HUD_WGSL })
    const viewLayout = device.createBindGroupLayout({
      label: 'hud view',
      entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: 'uniform' } }],
    })
    this.imageLayout = device.createBindGroupLayout({
      label: 'hud image',
      entries: [
        { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: {} },
        { binding: 1, visibility: GPUShaderStage.FRAGMENT, sampler: {} },
      ],
    })
    const attribute = (location: number, offset: number): GPUVertexAttribute => ({ shaderLocation: location, offset, format: 'float32x4' })
    this.pipeline = device.createRenderPipeline({
      label: 'hud',
      layout: device.createPipelineLayout({ bindGroupLayouts: [viewLayout, this.imageLayout] }),
      vertex: {
        module,
        entryPoint: 'vs',
        buffers: [{ arrayStride: STRIDE * 4, stepMode: 'instance', attributes: [attribute(0, 0), attribute(1, 16), attribute(2, 32), attribute(3, 48)] }],
      },
      fragment: {
        module,
        entryPoint: 'fs',
        targets: [{
          format,
          blend: {
            color: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
            alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
          },
        }],
      },
      primitive: { topology: 'triangle-strip' },
    })
    this.viewBuffer = device.createBuffer({ label: 'hud view', size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.viewGroup = device.createBindGroup({ layout: viewLayout, entries: [{ binding: 0, resource: { buffer: this.viewBuffer } }] })
    this.instanceBuffer = this.makeInstances(256)
  }

  private makeInstances(sprites: number) {
    return this.device.createBuffer({ label: 'hud sprites', size: sprites * STRIDE * 4, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST })
  }

  private group(texture: Texture | null): GPUBindGroup {
    const view = texture ? this.textures.view(texture) : this.textures.white
    const sampler = this.textures.sampler(texture ?? null)
    let bySampler = this.groups.get(view)
    if (!bySampler) this.groups.set(view, (bySampler = new Map()))
    let group = bySampler.get(sampler)
    if (!group) {
      group = this.device.createBindGroup({
        layout: this.imageLayout,
        entries: [{ binding: 0, resource: view }, { binding: 1, resource: sampler }],
      })
      bySampler.set(sampler, group)
    }
    return group
  }

  /**
   * Draws the layer's sprites into `pass`, an open render pass on the canvas (the output pass), and
   * empties the layer; returns the draw calls made. Drawing inside the output pass rather than in a pass
   * of its own saves loading and storing the whole finished frame again, which a tiled GPU pays in full.
   */
  draw(pass: GPURenderPassEncoder, layer: HudLayer): number {
    const instances = layer.instances
    if (instances.byteLength > this.instanceBuffer.size) {
      this.instanceBuffer.destroy()
      this.instanceBuffer = this.makeInstances(Math.ceil(instances.length / STRIDE) * 2)
    }
    this.device.queue.writeBuffer(this.instanceBuffer, 0, instances)
    this.viewData[0] = layer.width
    this.viewData[1] = layer.height
    this.device.queue.writeBuffer(this.viewBuffer, 0, this.viewData)
    pass.setPipeline(this.pipeline)
    pass.setBindGroup(0, this.viewGroup)
    pass.setVertexBuffer(0, this.instanceBuffer)
    let draws = 0
    for (const run of layer.batches) {
      pass.setBindGroup(1, this.group(run.texture))
      pass.draw(4, run.count, 0, run.first)
      draws++
    }
    layer.begin(layer.width, layer.height)
    return draws
  }
}
