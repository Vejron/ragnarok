import { Color } from '../math/vec'
import { Effect, type EffectContext } from './effect'
import { PARTICLE_COMPUTE_WGSL, particleRenderWgsl } from './particles.wgsl'

type Vec3Like = readonly [number, number, number] | { x: number; y: number; z: number }

/** How one particle moves and looks over its life (the CPU pool's spec, so emitters port unchanged). */
export interface ParticleSpec {
  position: Vec3Like
  velocity: Vec3Like
  /** Seconds. */
  life: number
  /** Diameter at birth and at death, metres. */
  size: readonly [number, number]
  /** sRGB hex or a linear Color. */
  color: number | Color
  /** Opacity at birth; it fades in over the first tenth of its life and out over the rest. */
  alpha: number
  /** Metres per second² pulling it down; negative floats it up. */
  gravity?: number
  /** Share of its speed lost per second. */
  drag?: number
  /** Bounces off the ground height field instead of passing through. */
  bounce?: boolean
  /** Alpha-blended (smoke, dust) or additive (fire, sparks). */
  blend?: 'alpha' | 'additive'
  /** Fogged with distance like everything else; off for things that should glow through haze. */
  fog?: boolean
  /** The colour it has reached at the end of its life (mixed linearly from `color`): fire cooling to smoke. */
  colorEnd?: number | Color
  /** Radians per second it turns, or a [min, max] range (each particle also starts at a random angle). */
  spin?: number | readonly [number, number]
  /** Metres of extra length per m/s of speed: the billboard stretches along its motion, for sparks and streaks. */
  stretch?: number
  /** Multiplies the colour (HDR), so values over 1 bloom. */
  glow?: number
  /** `puff` gives alpha-blended smoke a billowy, noise-eaten edge instead of a smooth disc. */
  shape?: 'round' | 'puff'
}

/** A burst: `count` particles from one spec, each with its own random values, rolled on the GPU. */
export interface EmitSpec extends Omit<ParticleSpec, 'life'> {
  count: number
  /** Seconds, or a [min, max] range. */
  life: number | readonly [number, number]
  /** Births scattered in a ball of this radius around `position`. */
  spread?: number
  /** Added velocity, uniform in ±this per axis. */
  jitter?: readonly [number, number, number]
  /** Added velocity in a random direction, at a speed up to this. */
  burst?: number
  /** Both sizes multiplied by a factor in this range. */
  scale?: readonly [number, number]
  /** Each particle's colour is a random mix between `color` and this. */
  color2?: number | Color
}

/** Floats per emit command; matches `Command` in particles.wgsl. */
const COMMAND_FLOATS = 40
/** Bytes in a particle; matches `Particle`. */
const PARTICLE_BYTES = 112
const PF_BOUNCE = 1
const PF_ADDITIVE = 2
const PF_NO_FOG = 4
const PF_PUFF = 8
/** Frames between reads of the live count. */
const COUNT_EVERY = 10

/**
 * Particles simulated entirely on the GPU. The CPU only records emit commands (40 floats per call, however
 * many particles it makes); a compute pass expands them into a fixed pool and another integrates every
 * live particle, then each blend class draws with one indirect call whose instance count the GPU wrote.
 * When the pool is full, new particles are dropped.
 *
 * Call `update(dt)` once per frame with the frame's time step; without it the particles hold still.
 */
export class ParticleSystem extends Effect {
  /** Live particles, read back from the GPU every few frames (so a little behind). */
  alive = 0
  private dt = 0
  private commands = new Float32Array(256 * COMMAND_FLOATS)
  private commandWords = new Uint32Array(this.commands.buffer)
  private commandCount = 0
  private spawnCount = 0
  private seed = 1
  private readonly color = new Color()
  private readonly color2 = new Color()
  private readonly colorEnd = new Color()
  private ground: { data: Float32Array<ArrayBuffer>; width: number; height: number; x0: number; z0: number; cell: number; version: number } | null = null
  private groundVersion = 0
  private gpu: ParticleGpu | null = null

  constructor(readonly capacity = 65536) {
    super()
    // The alive list is split per blend class; each part must start on a 256-byte boundary to bind it.
    if (capacity % 64) throw new Error('ParticleSystem capacity must be a multiple of 64')
  }

  /** Advances the simulation by `dt` seconds at the next render. */
  update(dt: number) {
    this.dt = dt
  }

  /** One particle, exactly as specified. */
  spawn(spec: ParticleSpec) {
    this.record(spec, 1, spec.life, spec.life, 0, null, 0, 1, 1, spec.color)
  }

  /** `count` particles from one spec, randomised on the GPU. */
  emit(spec: EmitSpec) {
    const [lifeMin, lifeMax] = typeof spec.life === 'number' ? [spec.life, spec.life] : spec.life
    const [scaleMin, scaleMax] = spec.scale ?? [1, 1]
    this.record(spec, spec.count, lifeMin, lifeMax, spec.spread ?? 0, spec.jitter ?? null, spec.burst ?? 0, scaleMin, scaleMax, spec.color2 ?? spec.color)
  }

  /**
   * The ground that bouncing particles land on: heights in metres on a grid of `cell`-metre squares,
   * row by row along +X then +Z from (x0, z0). Outside it particles fall through.
   */
  setGround(heights: Float32Array, width: number, height: number, x0: number, z0: number, cell: number) {
    this.ground = { data: heights as Float32Array<ArrayBuffer>, width, height, x0, z0, cell, version: ++this.groundVersion }
  }

  private record(
    spec: Omit<ParticleSpec, 'life'>, count: number, lifeMin: number, lifeMax: number, spread: number,
    jitter: readonly [number, number, number] | null, burst: number, scaleMin: number, scaleMax: number, color2: number | Color,
  ) {
    if (count <= 0) return
    if ((this.commandCount + 1) * COMMAND_FLOATS > this.commands.length) {
      const grown = new Float32Array(this.commands.length * 2)
      grown.set(this.commands)
      this.commands = grown
      this.commandWords = new Uint32Array(grown.buffer)
    }
    const c = this.commands
    const w = this.commandWords
    const o = this.commandCount * COMMAND_FLOATS
    const p = spec.position
    const v = spec.velocity
    if ('x' in p) { c[o] = p.x; c[o + 1] = p.y; c[o + 2] = p.z } else { c[o] = p[0]; c[o + 1] = p[1]; c[o + 2] = p[2] }
    c[o + 3] = spread
    if ('x' in v) { c[o + 4] = v.x; c[o + 5] = v.y; c[o + 6] = v.z } else { c[o + 4] = v[0]; c[o + 5] = v[1]; c[o + 6] = v[2] }
    c[o + 7] = burst
    c[o + 8] = jitter?.[0] ?? 0
    c[o + 9] = jitter?.[1] ?? 0
    c[o + 10] = jitter?.[2] ?? 0
    c[o + 11] = spec.drag ?? 0
    c[o + 12] = lifeMin
    c[o + 13] = lifeMax
    c[o + 14] = spec.size[0]
    c[o + 15] = spec.size[1]
    c[o + 16] = scaleMin
    c[o + 17] = scaleMax
    c[o + 18] = spec.alpha
    c[o + 19] = spec.gravity ?? 0
    w[o + 20] = count
    w[o + 21] = this.spawnCount
    w[o + 22] = this.seed = (this.seed * 1664525 + 1013904223) >>> 0
    w[o + 23] = (spec.bounce ? PF_BOUNCE : 0) | (spec.blend === 'additive' ? PF_ADDITIVE : 0) | (spec.fog === false ? PF_NO_FOG : 0) | (spec.shape === 'puff' ? PF_PUFF : 0)
    const a = toLinear(spec.color, this.color)
    const b = toLinear(color2, this.color2)
    c[o + 24] = a.r; c[o + 25] = a.g; c[o + 26] = a.b
    c[o + 28] = b.r; c[o + 29] = b.g; c[o + 30] = b.b
    const e = toLinear(spec.colorEnd ?? spec.color, this.colorEnd)
    c[o + 32] = e.r; c[o + 33] = e.g; c[o + 34] = e.b
    c[o + 35] = spec.glow ?? 1
    const spin = spec.spin ?? 0
    c[o + 36] = typeof spin === 'number' ? spin : spin[0]
    c[o + 37] = typeof spin === 'number' ? spin : spin[1]
    c[o + 38] = spec.stretch ?? 0
    this.commandCount++
    this.spawnCount += count
  }

  prepare(ctx: EffectContext, encoder: GPUCommandEncoder) {
    const gpu = (this.gpu ??= new ParticleGpu(ctx.device, this.capacity))
    const device = ctx.device
    if (this.ground && this.ground.version !== gpu.groundVersion) gpu.uploadGround(this.ground)
    const spawn = Math.min(this.spawnCount, this.capacity)
    if (this.commandCount) gpu.uploadCommands(this.commands, this.commandCount)
    const sim = gpu.simData
    sim[0] = this.dt
    const words = new Uint32Array(sim.buffer)
    words[1] = this.commandCount
    words[2] = spawn
    words[3] = this.capacity
    if (this.ground) {
      sim[4] = this.ground.x0
      sim[5] = this.ground.z0
      sim[6] = this.ground.cell
      sim[7] = 1
    }
    device.queue.writeBuffer(gpu.simBuffer, 0, sim)
    // Instance counts start at zero; the simulation counts the live particles back in.
    device.queue.writeBuffer(gpu.argsBuffer, 0, gpu.argsReset)
    const pass = encoder.beginComputePass({ label: 'particles' })
    pass.setBindGroup(0, gpu.computeGroup)
    if (spawn) {
      pass.setPipeline(gpu.emitPipeline)
      pass.dispatchWorkgroups(Math.ceil(spawn / 64))
    }
    pass.setPipeline(gpu.simulatePipeline)
    pass.dispatchWorkgroups(this.capacity / 64)
    pass.end()
    gpu.readCount(encoder, (dead) => { this.alive = this.capacity - dead })
    this.commandCount = 0
    this.spawnCount = 0
    this.dt = 0
  }

  draw(ctx: EffectContext, pass: GPURenderPassEncoder): number {
    const gpu = this.gpu
    if (!gpu) return 0
    gpu.ensureRender(ctx)
    for (let blend = 0; blend < 2; blend++) {
      pass.setPipeline(gpu.renderPipelines[blend]!)
      pass.setBindGroup(0, ctx.sceneGroup)
      pass.setBindGroup(1, gpu.renderGroups[blend]!)
      pass.drawIndirect(gpu.argsBuffer, blend * 16)
    }
    return 2
  }

  override afterSubmit() {
    this.gpu?.afterSubmit()
  }
}

function toLinear(c: number | Color, out: Color): Color {
  return typeof c === 'number' ? out.setHex(c) : out.copy(c)
}

/** The GPU half of a ParticleSystem, made on first use. */
class ParticleGpu {
  readonly simData = new Float32Array(8)
  readonly simBuffer: GPUBuffer
  readonly argsBuffer: GPUBuffer
  readonly argsReset: Uint32Array<ArrayBuffer>
  readonly emitPipeline: GPUComputePipeline
  readonly simulatePipeline: GPUComputePipeline
  computeGroup: GPUBindGroup
  renderPipelines: GPURenderPipeline[] = []
  renderGroups: GPUBindGroup[] = []
  groundVersion = 0
  private readonly particles: GPUBuffer
  private readonly deadList: GPUBuffer
  private readonly counters: GPUBuffer
  private readonly alive: GPUBuffer
  private commandBuffer: GPUBuffer
  private groundTexture: GPUTexture
  private readonly computeLayout: GPUBindGroupLayout
  private renderKey = ''
  private readonly countBuffers: GPUBuffer[] = []
  private pendingCount: { buffer: GPUBuffer; done: (dead: number) => void } | null = null
  private frame = 0

  constructor(private readonly device: GPUDevice, private readonly capacity: number) {
    const storage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST
    this.particles = device.createBuffer({ label: 'particles', size: capacity * PARTICLE_BYTES, usage: storage })
    this.deadList = device.createBuffer({ label: 'particle dead list', size: capacity * 4, usage: storage })
    this.counters = device.createBuffer({ label: 'particle counters', size: 16, usage: storage | GPUBufferUsage.COPY_SRC })
    this.alive = device.createBuffer({ label: 'particle alive list', size: capacity * 2 * 4, usage: storage })
    this.argsBuffer = device.createBuffer({ label: 'particle draw args', size: 32, usage: storage | GPUBufferUsage.INDIRECT })
    this.simBuffer = device.createBuffer({ label: 'particle sim', size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.commandBuffer = this.makeCommandBuffer(256)
    this.groundTexture = this.makeGroundTexture(1, 1)
    // Every slot starts dead: the dead stack holds all of them.
    const dead = new Uint32Array(capacity)
    for (let i = 0; i < capacity; i++) dead[i] = capacity - 1 - i
    device.queue.writeBuffer(this.deadList, 0, dead)
    device.queue.writeBuffer(this.counters, 0, new Int32Array([capacity, 0, 0, 0]))
    this.argsReset = new Uint32Array([6, 0, 0, 0, 6, 0, 0, 0])

    const module = device.createShaderModule({ label: 'particles compute', code: PARTICLE_COMPUTE_WGSL })
    const compute = GPUShaderStage.COMPUTE
    this.computeLayout = device.createBindGroupLayout({
      label: 'particles compute',
      entries: [
        { binding: 0, visibility: compute, buffer: { type: 'uniform' } },
        { binding: 1, visibility: compute, buffer: { type: 'storage' } },
        { binding: 2, visibility: compute, buffer: { type: 'storage' } },
        { binding: 3, visibility: compute, buffer: { type: 'storage' } },
        { binding: 4, visibility: compute, buffer: { type: 'storage' } },
        { binding: 5, visibility: compute, buffer: { type: 'storage' } },
        { binding: 6, visibility: compute, buffer: { type: 'read-only-storage' } },
        { binding: 7, visibility: compute, texture: { sampleType: 'unfilterable-float' } },
      ],
    })
    const layout = device.createPipelineLayout({ bindGroupLayouts: [this.computeLayout] })
    this.emitPipeline = device.createComputePipeline({ label: 'particles emit', layout, compute: { module, entryPoint: 'emit' } })
    this.simulatePipeline = device.createComputePipeline({ label: 'particles simulate', layout, compute: { module, entryPoint: 'simulate' } })
    this.computeGroup = this.makeComputeGroup()
  }

  uploadCommands(data: Float32Array, count: number) {
    const bytes = count * COMMAND_FLOATS * 4
    if (bytes > this.commandBuffer.size) {
      this.commandBuffer.destroy()
      this.commandBuffer = this.makeCommandBuffer(Math.max(count, (this.commandBuffer.size / (COMMAND_FLOATS * 4)) * 2))
      this.computeGroup = this.makeComputeGroup()
    }
    this.device.queue.writeBuffer(this.commandBuffer, 0, data as Float32Array<ArrayBuffer>, 0, count * COMMAND_FLOATS)
  }

  uploadGround(ground: { data: Float32Array<ArrayBuffer>; width: number; height: number; version: number }) {
    if (this.groundTexture.width !== ground.width || this.groundTexture.height !== ground.height) {
      this.groundTexture.destroy()
      this.groundTexture = this.makeGroundTexture(ground.width, ground.height)
      this.computeGroup = this.makeComputeGroup()
    }
    this.device.queue.writeTexture({ texture: this.groundTexture }, ground.data, { bytesPerRow: ground.width * 4 }, [ground.width, ground.height])
    this.groundVersion = ground.version
  }

  ensureRender(ctx: EffectContext) {
    const key = `${ctx.targetsVersion}|${ctx.sampleCount}`
    if (key === this.renderKey) return
    this.renderKey = key
    const device = this.device
    const multisampled = ctx.sampleCount > 1
    if (this.renderMultisampled !== multisampled) {
      const layout = device.createBindGroupLayout({
        label: 'particles render',
        entries: [
          { binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: 'read-only-storage' } },
          { binding: 1, visibility: GPUShaderStage.VERTEX, buffer: { type: 'read-only-storage' } },
          { binding: 2, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'depth', multisampled } },
        ],
      })
      const module = device.createShaderModule({ label: 'particles render', code: particleRenderWgsl(multisampled) })
      const pipelineLayout = device.createPipelineLayout({ bindGroupLayouts: [ctx.sceneLayout, layout] })
      const blends: GPUBlendState[] = [
        { color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha' }, alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' } },
        { color: { srcFactor: 'src-alpha', dstFactor: 'one' }, alpha: { srcFactor: 'src-alpha', dstFactor: 'one' } },
      ]
      this.renderPipelines = blends.map((blend, i) => device.createRenderPipeline({
        label: i ? 'particles additive' : 'particles alpha',
        layout: pipelineLayout,
        vertex: { module, entryPoint: 'vs' },
        fragment: { module, entryPoint: 'fs', targets: [{ format: ctx.colorFormat, blend }] },
        primitive: { topology: 'triangle-list' },
        // Tested against the scene's depth (reversed: nearer is larger), never written.
        depthStencil: { format: ctx.depthFormat, depthWriteEnabled: false, depthCompare: 'greater-equal' },
        multisample: { count: ctx.sampleCount },
      }))
      this.renderMultisampled = multisampled
      this.renderLayout = layout
    }
    this.renderGroups = [0, 1].map((blend) => device.createBindGroup({
      label: 'particles render',
      layout: this.renderLayout!,
      entries: [
        { binding: 0, resource: { buffer: this.particles } },
        { binding: 1, resource: { buffer: this.alive, offset: blend * this.capacity * 4, size: this.capacity * 4 } },
        { binding: 2, resource: ctx.depthView },
      ],
    }))
  }

  private renderLayout: GPUBindGroupLayout | null = null
  private renderMultisampled: boolean | null = null

  /** Every few frames, copies the dead count out for `alive`; the read completes after the submit. */
  readCount(encoder: GPUCommandEncoder, done: (dead: number) => void) {
    if (++this.frame % COUNT_EVERY || this.pendingCount) return
    const buffer = this.countBuffers.pop() ?? this.device.createBuffer({ size: 4, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST })
    encoder.copyBufferToBuffer(this.counters, 0, buffer, 0, 4)
    this.pendingCount = { buffer, done }
  }

  afterSubmit() {
    const pending = this.pendingCount
    if (!pending) return
    pending.buffer.mapAsync(GPUMapMode.READ).then(() => {
      pending.done(new Int32Array(pending.buffer.getMappedRange())[0]!)
      pending.buffer.unmap()
      this.countBuffers.push(pending.buffer)
      this.pendingCount = null
    }, () => { this.pendingCount = null })
  }

  private makeComputeGroup() {
    return this.device.createBindGroup({
      label: 'particles compute',
      layout: this.computeLayout,
      entries: [
        { binding: 0, resource: { buffer: this.simBuffer } },
        { binding: 1, resource: { buffer: this.particles } },
        { binding: 2, resource: { buffer: this.deadList } },
        { binding: 3, resource: { buffer: this.counters } },
        { binding: 4, resource: { buffer: this.argsBuffer } },
        { binding: 5, resource: { buffer: this.alive } },
        { binding: 6, resource: { buffer: this.commandBuffer } },
        { binding: 7, resource: this.groundTexture.createView() },
      ],
    })
  }

  private makeCommandBuffer(commands: number) {
    return this.device.createBuffer({ label: 'particle commands', size: commands * COMMAND_FLOATS * 4, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST })
  }

  private makeGroundTexture(width: number, height: number) {
    return this.device.createTexture({ label: 'particle ground', size: [width, height], format: 'r32float', usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST })
  }
}
