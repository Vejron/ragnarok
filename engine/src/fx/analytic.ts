import { COMMON_WGSL } from '../materials/common.wgsl'
import { Color } from '../math/vec'
import { Effect, type EffectContext } from './effect'

type Vec3Like = readonly [number, number, number] | { x: number; y: number; z: number }

/**
 * One short-lived streak or ground ring whose whole life is a formula of its progress p (0..1):
 *
 *   position = at + direction × along × p
 *   radius   = size + grow × p
 *   opacity  = opacity × (1 − p), or × (1 − p³) with `fade: 'late'`
 *
 * Anything round in the air (flashes, sparks, smoke) is a particle (`ParticleSystem`), not one of these.
 */
export interface Flash {
  at: Vec3Like
  direction?: Vec3Like
  /** Seconds. */
  life: number
  along?: number
  /** Half the width of a streak, or a ring's radius at birth, metres. */
  size: number
  /** Radius gained by the end (negative shrinks). */
  grow?: number
  /** A streak's length along `direction`, or the thickness of a ring's band. */
  length: number
  /** sRGB hex or a linear Color (a linear one may run past 1, to bloom). */
  color: number | Color
  opacity?: number
  fade?: 'linear' | 'late'
  /**
   * A ring lying flat on the ground (a shockwave) of radius `size + grow × p`, whose band is `length`
   * metres thick at its outer edge, brightest there and fading inward.
   */
  ring?: boolean
  /** Ignores fog, so it keeps its colour at range and against a pale sky. */
  bright?: boolean
  /** A streak that falls away from its middle to nothing at its edges, rather than a flat bar. */
  soft?: boolean
}

/** Floats per record; matches `Flash` in the shader. */
const RECORD_FLOATS = 20
const FLAG_LATE = 1
const FLAG_BRIGHT = 2
const FLAG_RING = 8
const FLAG_SOFT = 16

const WGSL = /* wgsl */ `
${COMMON_WGSL}

struct Flash {
  // xyz start, w birth time.
  at: vec4f,
  // xyz unit direction, w life.
  dir: vec4f,
  // size, grow, length, along.
  shape: vec4f,
  // linear rgb, opacity.
  color: vec4f,
  flags: f32,
}

struct Clock {
  now: f32,
}

@group(1) @binding(0) var<storage, read> flashes: array<Flash>;
@group(1) @binding(1) var<uniform> clock: Clock;

struct Out {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) color: vec4f,
  @location(2) viewDepth: f32,
  @location(3) @interpolate(flat) flags: u32,
  @location(4) thick: f32,
}

@vertex
fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> Out {
  let f = flashes[i];
  var out: Out;
  let p = (clock.now - f.at.w) / f.dir.w;
  if (p < 0.0 || p >= 1.0) {
    // Not born yet or already gone: collapse to nothing.
    out.clip = vec4f(0.0, 0.0, -1.0, 1.0);
    return out;
  }
  var corners = array<vec2f, 6>(vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0), vec2f(-1.0, -1.0), vec2f(1.0, 1.0), vec2f(-1.0, 1.0));
  let corner = corners[v];
  let flags = u32(f.flags);
  let centre = f.at.xyz + f.dir.xyz * f.shape.w * p;
  let radius = max(f.shape.x + f.shape.y * p, 0.0);
  var world: vec3f;
  out.thick = 1.0;
  if ((flags & 8u) != 0u) {
    // A ring on the ground: the quad lies flat, the fragment shader cuts the band out of it.
    world = centre + vec3f(corner.x, 0.0, corner.y) * radius;
    out.uv = corner;
    out.thick = clamp(f.shape.z / max(radius, 1e-3), 0.02, 1.0);
  } else {
    // A streak: long along its direction, as wide as its radius across the view.
    let toCamera = normalize(frame.cameraPos.xyz - centre);
    var side = cross(f.dir.xyz, toCamera);
    side = select(normalize(side), vec3f(1.0, 0.0, 0.0), length(side) < 1e-4);
    world = centre + f.dir.xyz * corner.y * f.shape.z * 0.5 + side * corner.x * radius;
    out.uv = vec2f(corner.x, 0.0);
  }
  let fade = select(1.0 - p, 1.0 - p * p * p, (flags & 1u) != 0u);
  out.clip = frame.viewProj * vec4f(world, 1.0);
  out.color = vec4f(f.color.rgb, f.color.a * fade);
  out.viewDepth = -(frame.view * vec4f(centre, 1.0)).z;
  out.flags = flags;
  return out;
}

@fragment
fn fs(in: Out) -> @location(0) vec4f {
  var edge = 1.0;
  let r = length(in.uv);
  let w = fwidth(r);
  if ((in.flags & 8u) != 0u) {
    let inner = 1.0 - in.thick;
    let band = smoothstep(inner, 1.0 - in.thick * 0.1, r);
    edge = band * band * (1.0 - smoothstep(1.0 - max(w * 2.0, 0.03), 1.0, r));
  } else if ((in.flags & 16u) != 0u) {
    edge = (1.0 - r) * (1.0 - r);
  }
  var rgb = in.color.rgb;
  if ((in.flags & 2u) == 0u && frame.fog.z > 0.0) {
    rgb = mix(rgb, frame.fogColor.rgb, smoothstep(frame.fog.x, frame.fog.y, in.viewDepth));
  }
  return vec4f(rgb, in.color.a * edge);
}
`

/**
 * Short-lived tracer streaks and ground rings as GPU records in a ring. Each is
 * written once when spawned and then lives entirely in the shader, so a burst of dozens costs a few
 * hundred bytes of upload and no per-frame CPU work. All of them draw in one or two
 * calls. Call `update(dt)` once per frame to advance their clock.
 */
export class FlashEffects extends Effect {
  private readonly data: Float32Array<ArrayBuffer>
  /** Expiry time per slot, so the drawn range can skip what has gone. */
  private readonly expiry: Float32Array
  private head = 0
  private tail = 0
  private live = 0
  private time = 0
  private dirtyFrom = -1
  private dirtyCount = 0
  private readonly color = new Color()
  private gpu: { device: GPUDevice; buffer: GPUBuffer; clock: GPUBuffer; pipeline: GPURenderPipeline; group: GPUBindGroup; sampleCount: number } | null = null

  constructor(readonly capacity = 8192) {
    super()
    this.data = new Float32Array(capacity * RECORD_FLOATS)
    this.expiry = new Float32Array(capacity)
  }

  update(dt: number) {
    this.time += dt
    while (this.live && this.expiry[this.tail]! <= this.time) {
      this.tail = (this.tail + 1) % this.capacity
      this.live--
    }
    // Rebase the clock while nothing is alive, so it never grows large enough to lose precision.
    if (!this.live && !this.dirtyCount) this.time = 0
  }

  spawn(flash: Flash) {
    if (this.live === this.capacity) {
      // Full: the oldest goes.
      this.tail = (this.tail + 1) % this.capacity
      this.live--
    }
    const i = this.head
    const o = i * RECORD_FLOATS
    const d = this.data
    const at = flash.at
    if ('x' in at) { d[o] = at.x; d[o + 1] = at.y; d[o + 2] = at.z } else { d[o] = at[0]; d[o + 1] = at[1]; d[o + 2] = at[2] }
    d[o + 3] = this.time
    let dx = 0, dy = 1, dz = 0
    const dir = flash.direction
    if (dir) {
      if ('x' in dir) { dx = dir.x; dy = dir.y; dz = dir.z } else { dx = dir[0]; dy = dir[1]; dz = dir[2] }
      const len = Math.hypot(dx, dy, dz) || 1
      dx /= len; dy /= len; dz /= len
    }
    d[o + 4] = dx; d[o + 5] = dy; d[o + 6] = dz
    d[o + 7] = Math.max(flash.life, 1e-3)
    d[o + 8] = flash.size
    d[o + 9] = flash.grow ?? 0
    d[o + 10] = flash.length
    d[o + 11] = flash.along ?? 0
    const c = typeof flash.color === 'number' ? this.color.setHex(flash.color) : flash.color
    d[o + 12] = c.r; d[o + 13] = c.g; d[o + 14] = c.b
    d[o + 15] = flash.opacity ?? 1
    d[o + 16] = (flash.fade === 'late' ? FLAG_LATE : 0) | (flash.bright ? FLAG_BRIGHT : 0) | (flash.ring ? FLAG_RING : 0) | (flash.soft ? FLAG_SOFT : 0)
    // Expiry only ever counts as "gone" from the tail; a long-lived record in the middle just stays drawn.
    this.expiry[i] = this.time + flash.life
    this.head = (i + 1) % this.capacity
    this.live++
    if (this.dirtyFrom < 0) this.dirtyFrom = i
    this.dirtyCount = Math.min(this.capacity, this.dirtyCount + 1)
  }

  /** How many records are in the drawn window. */
  get count() {
    return this.live
  }

  prepare(ctx: EffectContext) {
    const gpu = this.ensure(ctx)
    const queue = ctx.device.queue
    if (this.dirtyCount) {
      // The new records run from dirtyFrom, possibly wrapping past the end of the ring.
      const first = Math.min(this.dirtyCount, this.capacity - this.dirtyFrom)
      queue.writeBuffer(gpu.buffer, this.dirtyFrom * RECORD_FLOATS * 4, this.data, this.dirtyFrom * RECORD_FLOATS, first * RECORD_FLOATS)
      if (first < this.dirtyCount) queue.writeBuffer(gpu.buffer, 0, this.data, 0, (this.dirtyCount - first) * RECORD_FLOATS)
      this.dirtyFrom = -1
      this.dirtyCount = 0
    }
    queue.writeBuffer(gpu.clock, 0, new Float32Array([this.time, 0, 0, 0]))
  }

  draw(ctx: EffectContext, pass: GPURenderPassEncoder): number {
    const gpu = this.gpu
    if (!gpu || !this.live) return 0
    pass.setPipeline(gpu.pipeline)
    pass.setBindGroup(0, ctx.sceneGroup)
    pass.setBindGroup(1, gpu.group)
    const end = this.tail + this.live
    if (end <= this.capacity) {
      pass.draw(6, this.live, 0, this.tail)
      return 1
    }
    pass.draw(6, this.capacity - this.tail, 0, this.tail)
    pass.draw(6, end - this.capacity, 0, 0)
    return 2
  }

  private ensure(ctx: EffectContext) {
    // A new device (the old one was lost) needs everything made again, records included.
    if (this.gpu && this.gpu.device !== ctx.device) this.gpu = null
    if (this.gpu && this.gpu.sampleCount === ctx.sampleCount) return this.gpu
    const device = ctx.device
    const buffer = this.gpu?.buffer ?? device.createBuffer({ label: 'flashes', size: this.data.byteLength, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST })
    const clock = this.gpu?.clock ?? device.createBuffer({ label: 'flash clock', size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    const layout = device.createBindGroupLayout({
      label: 'flashes',
      entries: [
        { binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: 'read-only-storage' } },
        { binding: 1, visibility: GPUShaderStage.VERTEX, buffer: { type: 'uniform' } },
      ],
    })
    const module = device.createShaderModule({ label: 'flashes', code: WGSL })
    const pipeline = device.createRenderPipeline({
      label: 'flashes',
      layout: device.createPipelineLayout({ bindGroupLayouts: [ctx.sceneLayout, layout] }),
      vertex: { module, entryPoint: 'vs' },
      fragment: {
        module,
        entryPoint: 'fs',
        targets: [{
          format: ctx.colorFormat,
          blend: { color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha' }, alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' } },
        }],
      },
      primitive: { topology: 'triangle-list' },
      depthStencil: { format: ctx.depthFormat, depthWriteEnabled: false, depthCompare: 'greater-equal' },
      multisample: { count: ctx.sampleCount },
    })
    const group = device.createBindGroup({
      layout,
      entries: [
        { binding: 0, resource: { buffer } },
        { binding: 1, resource: { buffer: clock } },
      ],
    })
    // A fresh buffer has none of the records yet.
    if (!this.gpu && this.live) {
      this.dirtyFrom = this.tail
      this.dirtyCount = this.live
    }
    this.gpu = { device, buffer, clock, pipeline, group, sampleCount: ctx.sampleCount }
    return this.gpu
  }
}
