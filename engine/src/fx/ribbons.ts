import { Color } from '../math/vec'
import { COMMON_WGSL } from '../materials/common.wgsl'
import { Effect, type EffectContext } from './effect'

/** How a trail looks: it is the same along its whole length, the age of each part doing the changing. */
export interface RibbonStyle {
  /** sRGB hex or a linear Color, at the head. */
  color: number | Color
  /** The colour it has aged to at the tail; defaults to `color`. */
  colorEnd?: number | Color
  /** Multiplies the colour (HDR), so values over 1 bloom. */
  glow?: number
  /** Opacity at the head; it fades to nothing at the tail. */
  alpha?: number
  /** Seconds a point of the trail lasts. */
  life: number
  /** Width in metres at the head. */
  width: number
  /** Width at the tail as a share of `width`, 0 to 1 (default 0: tapering to a point). */
  taper?: number
  blend?: 'alpha' | 'additive'
  /** Metres the head must move before a new point is laid down (default 0.5). */
  step?: number
  /** Fogged with distance; off for things that should glow through haze. */
  fog?: boolean
}

/** Floats per segment; matches `Segment` in the shader. */
const SEGMENT_FLOATS = 20
const F_NO_FOG = 2

interface Trail {
  style: RibbonStyle
  /** Committed points, flattened x y z t. The head is the extra point after them. */
  points: number[]
  head: [number, number, number, number] | null
  lastSeen: number
  ended: boolean
}

const WGSL = /* wgsl */ `
${COMMON_WGSL}

struct Segment {
  // p0 xyz, age at p0.
  a: vec4f,
  // p1 xyz, age at p1.
  b: vec4f,
  // head colour rgb, glow.
  c: vec4f,
  // tail colour rgb, opacity at the head.
  d: vec4f,
  // life, width, taper, flags.
  e: vec4f,
}

@group(1) @binding(0) var<storage, read> segments: array<Segment>;

struct Out {
  @builtin(position) clip: vec4f,
  @location(0) color: vec4f,
  @location(1) across: f32,
  @location(2) viewDepth: f32,
  @location(3) @interpolate(flat) flags: u32,
}

@vertex
fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> Out {
  let s = segments[instance];
  var ends = array<f32, 6>(0.0, 1.0, 1.0, 0.0, 1.0, 0.0);
  var sides = array<f32, 6>(-1.0, -1.0, 1.0, -1.0, 1.0, 1.0);
  let end = ends[v];
  let side = sides[v];
  let pos = mix(s.a.xyz, s.b.xyz, end);
  let age = mix(s.a.w, s.b.w, end);
  let t = clamp(age / max(s.e.x, 1e-3), 0.0, 1.0);
  let width = s.e.y * mix(1.0, s.e.z, t) * select(1.0, 0.0, age >= s.e.x);
  var dir = s.b.xyz - s.a.xyz;
  let toCamera = frame.cameraPos.xyz - pos;
  var across = cross(dir, toCamera);
  if (dot(across, across) < 1e-10) {
    across = vec3f(frame.view[0].x, frame.view[1].x, frame.view[2].x);
  }
  let world = pos + normalize(across) * side * width * 0.5;
  var out: Out;
  out.clip = frame.viewProj * vec4f(world, 1.0);
  let rgb = mix(s.c.rgb, s.d.rgb, t) * s.c.w;
  let fade = (1.0 - t) * (1.0 - t);
  out.color = vec4f(rgb, s.d.w * fade);
  out.across = side;
  out.viewDepth = -(frame.view * vec4f(pos, 1.0)).z;
  out.flags = u32(s.e.w);
  return out;
}

@fragment
fn fs(in: Out) -> @location(0) vec4f {
  var alpha = in.color.a * smoothstep(1.0, 0.25, abs(in.across));
  if (alpha < 0.004) {
    discard;
  }
  var rgb = in.color.rgb;
  if ((in.flags & ${F_NO_FOG}u) == 0u && frame.fog.z > 0.0) {
    let f = smoothstep(frame.fog.x, frame.fog.y, in.viewDepth);
    rgb = mix(rgb, frame.fogColor.rgb, f);
  }
  return vec4f(rgb, alpha);
}
`

/**
 * Smooth camera-facing ribbons behind moving things: rocket exhaust, tracers, shell trails. Call
 * `extend(id, ...)` each frame with the thing's position; the ribbon lays points down as it moves, fades them
 * with age and keeps the tail after `end(id)` (or after it stops being extended) until it has faded.
 * Call `update(dt)` once per frame.
 */
export class Ribbons extends Effect {
  /** Segments drawn last frame. */
  count = 0
  private now = 0
  private readonly trails = new Map<number | string, Trail>()
  private data: Float32Array<ArrayBuffer>
  private readonly color = new Color()
  private readonly colorEnd = new Color()
  private gpu: { device: GPUDevice; buffer: GPUBuffer; pipelines: GPURenderPipeline[]; group: GPUBindGroup; format: string } | null = null
  private counts = [0, 0]

  constructor(readonly capacity = 8192) {
    super()
    this.renderOrder = 1
    this.data = new Float32Array(capacity * SEGMENT_FLOATS)
  }

  /** Moves the clock on by `dt` seconds and drops the parts of trails that have faded. */
  update(dt: number) {
    this.now += dt
    for (const [id, trail] of this.trails) {
      if (!trail.ended && this.now - trail.lastSeen > 0.5) trail.ended = true
      const p = trail.points
      let drop = 0
      while (drop + 4 <= p.length && this.now - p[drop + 3]! > trail.style.life) drop += 4
      if (drop) p.splice(0, drop)
      if (trail.ended && p.length === 0) this.trails.delete(id)
    }
  }

  /** Extends the trail `id` to `position` (creating it on the first call). */
  extend(id: number | string, x: number, y: number, z: number, style: RibbonStyle) {
    let trail = this.trails.get(id)
    if (!trail) {
      trail = { style, points: [], head: null, lastSeen: this.now, ended: false }
      this.trails.set(id, trail)
    }
    trail.style = style
    trail.lastSeen = this.now
    trail.ended = false
    const p = trail.points
    const n = p.length
    const step = style.step ?? 0.5
    if (n === 0 || (x - p[n - 4]!) ** 2 + (y - p[n - 3]!) ** 2 + (z - p[n - 2]!) ** 2 >= step * step) {
      p.push(x, y, z, this.now)
    }
    trail.head = [x, y, z, this.now]
  }

  /** The thing is gone: the trail stays where it is and fades out. */
  end(id: number | string) {
    const trail = this.trails.get(id)
    if (!trail) return
    trail.ended = true
    const h = trail.head
    const p = trail.points
    if (h && (p.length === 0 || p[p.length - 4] !== h[0] || p[p.length - 3] !== h[1] || p[p.length - 2] !== h[2])) {
      p.push(h[0], h[1], h[2], h[3])
    }
    trail.head = null
  }

  prepare(ctx: EffectContext, _encoder: GPUCommandEncoder) {
    const gpu = this.ensureGpu(ctx)
    const data = this.data
    let n = 0
    this.counts = [0, 0]
    for (let blend = 0; blend < 2; blend++) {
      const start = n
      for (const trail of this.trails.values()) {
        const style = trail.style
        if ((style.blend === 'additive' ? 1 : 0) !== blend) continue
        const p = trail.points
        const h = trail.head
        const points = p.length / 4 + (h ? 1 : 0)
        if (points < 2) continue
        const c = toLinear(style.color, this.color)
        const e = toLinear(style.colorEnd ?? style.color, this.colorEnd)
        const glow = style.glow ?? 1
        const flags = style.fog === false ? F_NO_FOG : 0
        for (let i = 0; i + 1 < points && n < this.capacity; i++) {
          const o = n * SEGMENT_FLOATS
          const a = i * 4
          const b = a + 4
          data[o] = p[a]!; data[o + 1] = p[a + 1]!; data[o + 2] = p[a + 2]!; data[o + 3] = this.now - p[a + 3]!
          if (b < p.length) {
            data[o + 4] = p[b]!; data[o + 5] = p[b + 1]!; data[o + 6] = p[b + 2]!; data[o + 7] = this.now - p[b + 3]!
          } else {
            data[o + 4] = h![0]; data[o + 5] = h![1]; data[o + 6] = h![2]; data[o + 7] = this.now - h![3]
          }
          data[o + 8] = c.r; data[o + 9] = c.g; data[o + 10] = c.b; data[o + 11] = glow
          data[o + 12] = e.r; data[o + 13] = e.g; data[o + 14] = e.b; data[o + 15] = style.alpha ?? 1
          data[o + 16] = style.life; data[o + 17] = style.width; data[o + 18] = style.taper ?? 0; data[o + 19] = flags
          n++
        }
      }
      this.counts[blend] = n - start
    }
    this.count = n
    if (n) ctx.device.queue.writeBuffer(gpu.buffer, 0, data, 0, n * SEGMENT_FLOATS)
  }

  draw(ctx: EffectContext, pass: GPURenderPassEncoder): number {
    const gpu = this.gpu
    if (!gpu || !this.count) return 0
    pass.setBindGroup(0, ctx.sceneGroup)
    pass.setBindGroup(1, gpu.group)
    let calls = 0
    let first = 0
    for (let blend = 0; blend < 2; blend++) {
      const n = this.counts[blend]!
      if (n) {
        pass.setPipeline(gpu.pipelines[blend]!)
        pass.draw(6, n, 0, first)
        calls++
      }
      first += n
    }
    return calls
  }

  private ensureGpu(ctx: EffectContext) {
    const key = `${ctx.colorFormat}|${ctx.sampleCount}`
    // A new device (the old one was lost) needs everything made again.
    if (this.gpu && this.gpu.device !== ctx.device) this.gpu = null
    if (this.gpu && this.gpu.format === key) return this.gpu
    const device = ctx.device
    const buffer = this.gpu?.buffer ?? device.createBuffer({
      label: 'ribbons',
      size: this.capacity * SEGMENT_FLOATS * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    })
    const layout = device.createBindGroupLayout({
      label: 'ribbons',
      entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: 'read-only-storage' } }],
    })
    const module = device.createShaderModule({ label: 'ribbons', code: WGSL })
    const pipelineLayout = device.createPipelineLayout({ bindGroupLayouts: [ctx.sceneLayout, layout] })
    const blends: GPUBlendState[] = [
      { color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha' }, alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' } },
      { color: { srcFactor: 'src-alpha', dstFactor: 'one' }, alpha: { srcFactor: 'src-alpha', dstFactor: 'one' } },
    ]
    const pipelines = blends.map((blend, i) => device.createRenderPipeline({
      label: i ? 'ribbons additive' : 'ribbons alpha',
      layout: pipelineLayout,
      vertex: { module, entryPoint: 'vs' },
      fragment: { module, entryPoint: 'fs', targets: [{ format: ctx.colorFormat, blend }] },
      primitive: { topology: 'triangle-list' },
      depthStencil: { format: ctx.depthFormat, depthWriteEnabled: false, depthCompare: 'greater-equal' },
      multisample: { count: ctx.sampleCount },
    }))
    const group = device.createBindGroup({ label: 'ribbons', layout, entries: [{ binding: 0, resource: { buffer } }] })
    this.gpu = { device, buffer, pipelines, group, format: key }
    return this.gpu
  }
}

function toLinear(c: number | Color, out: Color): Color {
  return typeof c === 'number' ? out.setHex(c) : out.copy(c)
}
