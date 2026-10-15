import { GeometryArena, VERTEX_LAYOUT, type GeometrySlot } from './gpu/arena'
import { MaterialTable } from './gpu/materials'
import { GpuTimer } from './gpu/timer'
import { SHADOW_WGSL } from './materials/shadow.wgsl'
import { STANDARD_WGSL } from './materials/standard.wgsl'
import { Frustum } from './math/frustum'
import * as mat4 from './math/mat4'
import { OUTPUT_WGSL } from './passes/output.wgsl'
import type { PerspectiveCamera } from './scene/camera'
import type { Material } from './scene/material'
import { InstancedMesh, Mesh } from './scene/mesh'
import type { Node } from './scene/node'
import type { Scene } from './scene/scene'

/** Floats in the frame uniform block. Matches `Frame` in common.wgsl (padded to 384 bytes). */
const FRAME_FLOATS = 96
/** Floats per object record: model mat4, tint vec4, misc vec4u. Matches `Object` in common.wgsl. */
const OBJECT_FLOATS = 24
const OBJ_RECEIVE_SHADOW = 1
const HDR_FORMAT: GPUTextureFormat = 'rgba16float'
const DEPTH_FORMAT: GPUTextureFormat = 'depth32float'

export interface RendererOptions {
  /** Multisampling for the main pass: 4 (default) or 1. */
  sampleCount?: 1 | 4
  /** Called if the GPU device is lost (driver reset, tab backgrounded too long on some platforms). */
  onDeviceLost?: (info: GPUDeviceLostInfo) => void
}

/** What the last frame cost and drew. */
export class RenderStats {
  /** Draw calls in all passes. */
  drawCalls = 0
  /** Triangles in all passes. */
  triangles = 0
  /** Object records written (an instanced mesh writes one per instance). */
  objects = 0
  shadowDraws = 0
  /** CPU time inside `render`, ms. */
  cpuMs = 0
  pipelines = 0
  materials = 0
  geometryBytes = 0
  /** Smoothed GPU ms per pass, when the device supports timestamp queries. */
  gpuMs: Record<string, number> = {}
}

/** One mesh (or one instanced mesh) to draw in a pass this frame. Pooled. */
interface DrawItem {
  mesh: Mesh
  slot: GeometrySlot
  material: number
  pipeline: number
  order: number
  depth: number
}

/** A run of consecutive object records drawn with one call. */
interface Batch {
  pipeline: number
  slot: GeometrySlot
  first: number
  count: number
}

/**
 * Draws a `Scene` with WebGPU. All object transforms of a frame go into one storage buffer, all
 * materials live in another, and all geometry in one vertex and one index buffer, so a draw call is a
 * pipeline switch at most and then offsets. Meshes sharing geometry and render state become a single
 * instanced draw whatever their materials.
 */
export class Renderer {
  readonly stats = new RenderStats()
  readonly format: GPUTextureFormat
  readonly sampleCount: number
  width = 1
  height = 1

  private readonly context: GPUCanvasContext
  private readonly arena: GeometryArena
  private readonly materials: MaterialTable
  private readonly timer: GpuTimer
  private readonly frameData = new Float32Array(FRAME_FLOATS)
  private readonly frameBuffer: GPUBuffer
  private objectData = new Float32Array(1024 * OBJECT_FLOATS)
  private objectWords = new Uint32Array(this.objectData.buffer)
  private objectBuffer: GPUBuffer
  private readonly sceneLayout: GPUBindGroupLayout
  private readonly lightingLayout: GPUBindGroupLayout
  private readonly mainLayout: GPUPipelineLayout
  private readonly shadowLayout: GPUPipelineLayout
  private readonly standardModule: GPUShaderModule
  private readonly shadowModule: GPUShaderModule
  private readonly pipelines: GPURenderPipeline[] = []
  private readonly pipelineKeys = new Map<string, number>()
  private readonly materialPipelines = new Map<number, { version: number; pipeline: number }>()
  private readonly shadowPipelines: GPURenderPipeline[]
  private readonly outputPipeline: GPURenderPipeline
  private readonly outputUniform: GPUBuffer
  private readonly comparisonSampler: GPUSampler
  private sceneGroup: GPUBindGroup | null = null
  private sceneGroupKey = ''
  private lightingGroup: GPUBindGroup | null = null
  private outputGroup: GPUBindGroup | null = null
  private colorTarget: GPUTexture | null = null
  private depthTarget: GPUTexture | null = null
  private hdrTarget: GPUTexture | null = null
  private shadowMap: GPUTexture
  private readonly frustum = new Frustum()
  private readonly shadowFrustum = new Frustum()
  private readonly shadowMatrix = mat4.create()
  private readonly lightView = mat4.create()
  private readonly lightProjection = mat4.create()
  private readonly stack: Node[] = []
  private readonly pool: DrawItem[] = []
  private poolUsed = 0
  private readonly opaque: DrawItem[] = []
  private readonly transparent: DrawItem[] = []
  private readonly casters: DrawItem[] = []
  private readonly mainBatches: Batch[] = []
  private readonly shadowBatches: Batch[] = []
  private recordCount = 0
  private readonly tmp = mat4.create()
  private camX = 0
  private camY = 0
  private camZ = 0
  private shadowsOn = false

  static async create(canvas: HTMLCanvasElement, options: RendererOptions = {}): Promise<Renderer> {
    if (!navigator.gpu) throw new Error('WebGPU is not available in this browser')
    const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' })
    if (!adapter) throw new Error('No WebGPU adapter')
    const requiredFeatures: GPUFeatureName[] = []
    if (adapter.features.has('timestamp-query')) requiredFeatures.push('timestamp-query')
    const device = await adapter.requestDevice({ requiredFeatures })
    return new Renderer(canvas, device, options)
  }

  constructor(readonly canvas: HTMLCanvasElement, readonly device: GPUDevice, options: RendererOptions = {}) {
    this.sampleCount = options.sampleCount ?? 4
    device.lost.then((info) => {
      if (options.onDeviceLost) options.onDeviceLost(info)
      else console.error(`WebGPU device lost: ${info.message}`)
    })
    device.addEventListener('uncapturederror', (event) => console.error('WebGPU:', (event as GPUUncapturedErrorEvent).error.message))

    const context = canvas.getContext('webgpu')
    if (!context) throw new Error('Could not get a WebGPU canvas context')
    this.context = context
    this.format = navigator.gpu.getPreferredCanvasFormat()
    context.configure({ device, format: this.format, alphaMode: 'opaque' })

    this.arena = new GeometryArena(device)
    this.materials = new MaterialTable(device)
    this.timer = new GpuTimer(device)
    this.frameBuffer = device.createBuffer({ label: 'frame', size: FRAME_FLOATS * 4, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.objectBuffer = this.makeObjectBuffer(1024)
    this.comparisonSampler = device.createSampler({ compare: 'less-equal', magFilter: 'linear', minFilter: 'linear' })
    this.shadowMap = this.makeShadowMap(1)

    const both = GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT
    this.sceneLayout = device.createBindGroupLayout({
      label: 'scene',
      entries: [
        { binding: 0, visibility: both, buffer: { type: 'uniform' } },
        { binding: 1, visibility: both, buffer: { type: 'read-only-storage' } },
        { binding: 2, visibility: both, buffer: { type: 'read-only-storage' } },
      ],
    })
    this.lightingLayout = device.createBindGroupLayout({
      label: 'lighting',
      entries: [
        { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'unfilterable-float' } },
        { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'depth' } },
        { binding: 2, visibility: GPUShaderStage.FRAGMENT, sampler: { type: 'comparison' } },
      ],
    })
    this.mainLayout = device.createPipelineLayout({ bindGroupLayouts: [this.sceneLayout, this.lightingLayout] })
    this.shadowLayout = device.createPipelineLayout({ bindGroupLayouts: [this.sceneLayout] })
    this.standardModule = device.createShaderModule({ label: 'standard', code: STANDARD_WGSL })
    this.shadowModule = device.createShaderModule({ label: 'shadow', code: SHADOW_WGSL })
    this.shadowPipelines = (['back', 'none'] as GPUCullMode[]).map((cullMode) =>
      device.createRenderPipeline({
        label: `shadow ${cullMode}`,
        layout: this.shadowLayout,
        vertex: { module: this.shadowModule, entryPoint: 'vs', buffers: [VERTEX_LAYOUT] },
        primitive: { topology: 'triangle-list', cullMode },
        depthStencil: { format: DEPTH_FORMAT, depthWriteEnabled: true, depthCompare: 'less', depthBias: 1, depthBiasSlopeScale: 1.5 },
      }),
    )

    const outputModule = device.createShaderModule({ label: 'output', code: OUTPUT_WGSL })
    this.outputPipeline = device.createRenderPipeline({
      label: 'output',
      layout: 'auto',
      vertex: { module: outputModule, entryPoint: 'vs' },
      fragment: { module: outputModule, entryPoint: 'fs', targets: [{ format: this.format }] },
      primitive: { topology: 'triangle-list' },
    })
    this.outputUniform = device.createBuffer({ size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.setSize(canvas.clientWidth || 1, canvas.clientHeight || 1, 1)
  }

  /** Sizes the canvas's drawing buffer to `width`×`height` CSS pixels at `pixelRatio`. */
  setSize(width: number, height: number, pixelRatio = 1) {
    const w = Math.max(1, Math.floor(width * pixelRatio))
    const h = Math.max(1, Math.floor(height * pixelRatio))
    if (w === this.width && h === this.height && this.hdrTarget) return
    this.width = w
    this.height = h
    this.canvas.width = w
    this.canvas.height = h
    this.colorTarget?.destroy()
    this.depthTarget?.destroy()
    this.hdrTarget?.destroy()
    const size = [w, h]
    this.colorTarget = this.sampleCount > 1
      ? this.device.createTexture({ label: 'msaa colour', size, format: HDR_FORMAT, sampleCount: this.sampleCount, usage: GPUTextureUsage.RENDER_ATTACHMENT })
      : null
    this.depthTarget = this.device.createTexture({
      label: 'depth',
      size,
      format: DEPTH_FORMAT,
      sampleCount: this.sampleCount,
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    })
    this.hdrTarget = this.device.createTexture({
      label: 'hdr',
      size,
      format: HDR_FORMAT,
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    })
    this.outputGroup = this.device.createBindGroup({
      layout: this.outputPipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: this.outputUniform } },
        { binding: 1, resource: this.hdrTarget.createView() },
      ],
    })
  }

  render(scene: Scene, camera: PerspectiveCamera) {
    const start = performance.now()
    const stats = this.stats
    scene.updateWorld()
    if (!camera.parent) camera.updateWorld()
    camera.updateMatrices()
    this.frustum.setFromMatrix(camera.viewProjection)
    this.camX = camera.matrixWorld[12]!
    this.camY = camera.matrixWorld[13]!
    this.camZ = camera.matrixWorld[14]!
    this.setupShadow(scene)

    this.poolUsed = 0
    this.opaque.length = 0
    this.transparent.length = 0
    this.casters.length = 0
    this.collect(scene)
    this.opaque.sort(byStateThenGeometry)
    this.transparent.sort(byOrderThenFarToNear)
    this.casters.sort(byStateThenGeometry)

    this.recordCount = 0
    this.mainBatches.length = 0
    this.shadowBatches.length = 0
    this.writeBatches(this.opaque, this.mainBatches, true)
    this.writeBatches(this.transparent, this.mainBatches, false)
    this.writeBatches(this.casters, this.shadowBatches, true)

    this.writeFrame(scene, camera)
    this.materials.flush()
    this.uploadObjects()
    this.bindScene()

    const encoder = this.device.createCommandEncoder()
    this.timer.begin()
    stats.drawCalls = 0
    stats.triangles = 0
    stats.shadowDraws = 0
    if (this.shadowsOn) this.shadowPass(encoder)
    this.mainPass(encoder, scene)
    this.outputPass(encoder, scene)
    this.timer.resolve(encoder)
    this.device.queue.submit([encoder.finish()])
    this.timer.read()
    this.materials.collect()

    stats.objects = this.recordCount
    stats.pipelines = this.pipelines.length + this.shadowPipelines.length + 1
    stats.materials = this.materials.count
    stats.geometryBytes = this.arena.usedBytes
    stats.gpuMs = this.timer.ms
    stats.cpuMs = performance.now() - start
  }

  // ---- culling and sorting ----

  private collect(scene: Scene) {
    const stack = this.stack
    stack.length = 0
    stack.push(scene)
    while (stack.length) {
      const node = stack.pop()!
      if (!node.visible) continue
      if (node instanceof Mesh) this.consider(node)
      const children = node.children
      for (let i = 0; i < children.length; i++) stack.push(children[i]!)
    }
  }

  private consider(mesh: Mesh) {
    if (mesh instanceof InstancedMesh && mesh.count <= 0) return
    const slot = this.arena.sync(mesh.geometry)
    if (!slot || !slot.indexCount) return
    const m = mesh.matrixWorld
    let bx: number, by: number, bz: number, br: number
    if (mesh instanceof InstancedMesh) {
      const b = instanceBounds(mesh)
      bx = b.x; by = b.y; bz = b.z; br = b.radius
    } else {
      const b = mesh.geometry.bounds
      bx = b.x; by = b.y; bz = b.z; br = b.radius
    }
    const cx = m[0]! * bx + m[4]! * by + m[8]! * bz + m[12]!
    const cy = m[1]! * bx + m[5]! * by + m[9]! * bz + m[13]!
    const cz = m[2]! * bx + m[6]! * by + m[10]! * bz + m[14]!
    const radius = br * mat4.maxScale(m)
    const material = mesh.material
    const materialIndex = this.materials.sync(material)

    if (!mesh.frustumCulled || this.frustum.intersectsSphere(cx, cy, cz, radius)) {
      const item = this.item(mesh, slot, materialIndex, this.pipelineFor(material))
      const dx = cx - this.camX, dy = cy - this.camY, dz = cz - this.camZ
      item.depth = dx * dx + dy * dy + dz * dz
      ;(material.transparent ? this.transparent : this.opaque).push(item)
    }
    if (this.shadowsOn && mesh.castShadow && !material.transparent && (!mesh.frustumCulled || this.shadowFrustum.intersectsSphere(cx, cy, cz, radius))) {
      this.casters.push(this.item(mesh, slot, materialIndex, material.side === 'front' ? 0 : 1))
    }
  }

  private item(mesh: Mesh, slot: GeometrySlot, material: number, pipeline: number): DrawItem {
    let item = this.pool[this.poolUsed]
    if (!item) {
      item = { mesh, slot, material, pipeline, order: 0, depth: 0 }
      this.pool.push(item)
    }
    this.poolUsed++
    item.mesh = mesh
    item.slot = slot
    item.material = material
    item.pipeline = pipeline
    item.order = mesh.renderOrder
    item.depth = 0
    return item
  }

  /** Writes the records for `items` in order, merging runs that can share one draw when `merge` is set. */
  private writeBatches(items: DrawItem[], batches: Batch[], merge: boolean) {
    for (const item of items) {
      const first = this.recordCount
      const mesh = item.mesh
      const flags = mesh.receiveShadow ? OBJ_RECEIVE_SHADOW : 0
      if (mesh instanceof InstancedMesh) {
        this.reserve(mesh.count)
        for (let i = 0; i < mesh.count; i++) this.writeInstance(mesh, i, item.material, flags)
      } else {
        this.reserve(1)
        this.writeRecord(mesh.matrixWorld, 0, mesh.tint, 0, item.material, flags)
      }
      const count = this.recordCount - first
      const last = batches[batches.length - 1]
      if (merge && last && last.pipeline === item.pipeline && last.slot === item.slot && last.first + last.count === first) {
        last.count += count
      } else {
        batches.push({ pipeline: item.pipeline, slot: item.slot, first, count })
      }
    }
  }

  private reserve(n: number) {
    const needed = (this.recordCount + n) * OBJECT_FLOATS
    if (needed <= this.objectData.length) return
    let size = this.objectData.length
    while (size < needed) size *= 2
    const data = new Float32Array(size)
    data.set(this.objectData)
    this.objectData = data
    this.objectWords = new Uint32Array(data.buffer)
  }

  private writeRecord(model: Float32Array, modelOffset: number, tint: Float32Array, tintOffset: number, material: number, flags: number) {
    const o = this.recordCount * OBJECT_FLOATS
    const d = this.objectData
    for (let k = 0; k < 16; k++) d[o + k] = model[modelOffset + k]!
    d[o + 16] = tint[tintOffset]!
    d[o + 17] = tint[tintOffset + 1]!
    d[o + 18] = tint[tintOffset + 2]!
    d[o + 19] = tint[tintOffset + 3]!
    this.objectWords[o + 20] = material
    this.objectWords[o + 21] = flags
    this.recordCount++
  }

  private writeInstance(mesh: InstancedMesh, i: number, material: number, flags: number) {
    const o = this.recordCount * OBJECT_FLOATS
    const d = this.objectData
    const a = mesh.matrixWorld
    const b = mesh.matrices
    const bo = i * 16
    for (let c = 0; c < 4; c++) {
      const b0 = b[bo + c * 4]!, b1 = b[bo + c * 4 + 1]!, b2 = b[bo + c * 4 + 2]!, b3 = b[bo + c * 4 + 3]!
      d[o + c * 4] = a[0]! * b0 + a[4]! * b1 + a[8]! * b2 + a[12]! * b3
      d[o + c * 4 + 1] = a[1]! * b0 + a[5]! * b1 + a[9]! * b2 + a[13]! * b3
      d[o + c * 4 + 2] = a[2]! * b0 + a[6]! * b1 + a[10]! * b2 + a[14]! * b3
      d[o + c * 4 + 3] = a[3]! * b0 + a[7]! * b1 + a[11]! * b2 + a[15]! * b3
    }
    const t = mesh.tint
    const col = mesh.colors
    d[o + 16] = t[0]! * col[i * 4]!
    d[o + 17] = t[1]! * col[i * 4 + 1]!
    d[o + 18] = t[2]! * col[i * 4 + 2]!
    d[o + 19] = t[3]! * col[i * 4 + 3]!
    this.objectWords[o + 20] = material
    this.objectWords[o + 21] = flags
    this.recordCount++
  }

  // ---- pipelines ----

  private pipelineFor(material: Material): number {
    const cached = this.materialPipelines.get(material.id)
    if (cached && cached.version === material.version) return cached.pipeline
    const key = `${material.blend}|${material.side}|${material.depthTest}|${material.depthWrite}|${material.depthBias}|${material.depthBiasSlope}`
    let pipeline = this.pipelineKeys.get(key)
    if (pipeline === undefined) {
      pipeline = this.pipelines.length
      this.pipelines.push(this.makePipeline(material, key))
      this.pipelineKeys.set(key, pipeline)
    }
    this.materialPipelines.set(material.id, { version: material.version, pipeline })
    return pipeline
  }

  private makePipeline(m: Material, label: string): GPURenderPipeline {
    let blend: GPUBlendState | undefined
    if (m.blend === 'alpha') {
      blend = {
        color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha' },
        alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' },
      }
    } else if (m.blend === 'additive') {
      blend = {
        color: { srcFactor: 'src-alpha', dstFactor: 'one' },
        alpha: { srcFactor: 'src-alpha', dstFactor: 'one' },
      }
    }
    return this.device.createRenderPipeline({
      label,
      layout: this.mainLayout,
      vertex: { module: this.standardModule, entryPoint: 'vs', buffers: [VERTEX_LAYOUT] },
      fragment: { module: this.standardModule, entryPoint: 'fs', targets: [{ format: HDR_FORMAT, blend }] },
      primitive: { topology: 'triangle-list', cullMode: m.side === 'front' ? 'back' : m.side === 'back' ? 'front' : 'none' },
      depthStencil: {
        format: DEPTH_FORMAT,
        depthWriteEnabled: m.depthWrite,
        // Reversed depth: nearer is larger.
        depthCompare: m.depthTest ? 'greater-equal' : 'always',
        // Positive bias pulls toward the camera, which with reversed depth means a larger value.
        depthBias: Math.round(m.depthBias),
        depthBiasSlopeScale: m.depthBiasSlope,
      },
      multisample: { count: this.sampleCount },
    })
  }

  // ---- uniforms and bind groups ----

  private setupShadow(scene: Scene) {
    const sun = scene.sun
    const shadow = sun.shadow
    this.shadowsOn = shadow.enabled
    if (!shadow.enabled) return
    if (this.shadowMap.width !== shadow.mapSize) {
      this.shadowMap.destroy()
      this.shadowMap = this.makeShadowMap(shadow.mapSize)
      this.lightingGroup = null
    }
    let dx = sun.position.x - sun.target.x, dy = sun.position.y - sun.target.y, dz = sun.position.z - sun.target.z
    const len = Math.hypot(dx, dy, dz) || 1
    dx /= len; dy /= len; dz /= len
    // Orient the light's view by direction alone and snap the target to whole texels in that view, so the
    // shadow's edges do not crawl as the target moves.
    mat4.lookAt(this.lightView, dx, dy, dz, 0, 0, 0, Math.abs(dy) > 0.99 ? 1 : 0, Math.abs(dy) > 0.99 ? 0 : 1, 0)
    const v = this.lightView
    const t = sun.target
    const texel = shadow.span / shadow.mapSize
    const cx = Math.round((v[0]! * t.x + v[4]! * t.y + v[8]! * t.z + v[12]!) / texel) * texel
    const cy = Math.round((v[1]! * t.x + v[5]! * t.y + v[9]! * t.z + v[13]!) / texel) * texel
    const cz = v[2]! * t.x + v[6]! * t.y + v[10]! * t.z + v[14]!
    const half = shadow.span / 2
    mat4.orthographic(this.lightProjection, cx - half, cx + half, cy - half, cy + half, -cz - shadow.depth, -cz + shadow.depth)
    mat4.multiply(this.shadowMatrix, this.lightProjection, this.lightView)
    this.shadowFrustum.setFromMatrix(this.shadowMatrix)
  }

  private writeFrame(scene: Scene, camera: PerspectiveCamera) {
    const f = this.frameData
    f.set(camera.viewProjection, 0)
    f.set(camera.view, 16)
    f[32] = this.camX
    f[33] = this.camY
    f[34] = this.camZ
    const sun = scene.sun
    let dx = sun.position.x - sun.target.x, dy = sun.position.y - sun.target.y, dz = sun.position.z - sun.target.z
    const len = Math.hypot(dx, dy, dz) || 1
    f[36] = dx / len
    f[37] = dy / len
    f[38] = dz / len
    f[40] = sun.color.r * sun.intensity
    f[41] = sun.color.g * sun.intensity
    f[42] = sun.color.b * sun.intensity
    const hemi = scene.hemisphere
    f[44] = hemi.sky.r * hemi.intensity
    f[45] = hemi.sky.g * hemi.intensity
    f[46] = hemi.sky.b * hemi.intensity
    f[48] = hemi.ground.r * hemi.intensity
    f[49] = hemi.ground.g * hemi.intensity
    f[50] = hemi.ground.b * hemi.intensity
    const fog = scene.fog
    if (fog) {
      f[52] = fog.color.r
      f[53] = fog.color.g
      f[54] = fog.color.b
      f[56] = fog.near
      f[57] = fog.far
    }
    f[58] = fog ? 1 : 0
    f[59] = performance.now() / 1000
    f[60] = this.width
    f[61] = this.height
    f[62] = 1 / this.width
    f[63] = 1 / this.height
    f.set(this.shadowMatrix, 64)
    const shadow = sun.shadow
    f[80] = shadow.span / shadow.mapSize
    f[81] = shadow.radius
    f[82] = this.shadowsOn ? 1 : 0
    this.device.queue.writeBuffer(this.frameBuffer, 0, f)
  }

  private uploadObjects() {
    const bytes = this.recordCount * OBJECT_FLOATS * 4
    if (bytes > this.objectBuffer.size) {
      this.objectBuffer.destroy()
      this.objectBuffer = this.makeObjectBuffer(this.objectData.length / OBJECT_FLOATS)
    }
    if (bytes) this.device.queue.writeBuffer(this.objectBuffer, 0, this.objectData, 0, this.recordCount * OBJECT_FLOATS)
  }

  private bindScene() {
    const key = `${this.objectBuffer.label}:${this.objectBuffer.size}:${this.materials.generation}`
    if (!this.sceneGroup || key !== this.sceneGroupKey) {
      this.sceneGroupKey = key
      this.sceneGroup = this.device.createBindGroup({
        label: 'scene',
        layout: this.sceneLayout,
        entries: [
          { binding: 0, resource: { buffer: this.frameBuffer } },
          { binding: 1, resource: { buffer: this.objectBuffer } },
          { binding: 2, resource: { buffer: this.materials.buffer } },
        ],
      })
    }
    if (!this.lightingGroup) {
      this.lightingGroup = this.device.createBindGroup({
        label: 'lighting',
        layout: this.lightingLayout,
        entries: [
          { binding: 0, resource: this.materials.ramps.createView() },
          { binding: 1, resource: this.shadowMap.createView() },
          { binding: 2, resource: this.comparisonSampler },
        ],
      })
    }
  }

  // ---- passes ----

  private shadowPass(encoder: GPUCommandEncoder) {
    const pass = encoder.beginRenderPass({
      label: 'shadow',
      colorAttachments: [],
      depthStencilAttachment: { view: this.shadowMap.createView(), depthClearValue: 1, depthLoadOp: 'clear', depthStoreOp: 'store' },
      timestampWrites: this.timer.pass('shadow'),
    })
    pass.setBindGroup(0, this.sceneGroup!)
    pass.setVertexBuffer(0, this.arena.vertexBuffer)
    pass.setIndexBuffer(this.arena.indexBuffer, 'uint32')
    let current = -1
    for (const batch of this.shadowBatches) {
      if (batch.pipeline !== current) {
        pass.setPipeline(this.shadowPipelines[batch.pipeline]!)
        current = batch.pipeline
      }
      pass.drawIndexed(batch.slot.indexCount, batch.count, batch.slot.firstIndex, batch.slot.baseVertex, batch.first)
      this.stats.drawCalls++
      this.stats.shadowDraws++
      this.stats.triangles += (batch.slot.indexCount / 3) * batch.count
    }
    pass.end()
  }

  private mainPass(encoder: GPUCommandEncoder, scene: Scene) {
    const bg = scene.background
    const msaa = this.colorTarget !== null
    const pass = encoder.beginRenderPass({
      label: 'main',
      colorAttachments: [{
        view: msaa ? this.colorTarget!.createView() : this.hdrTarget!.createView(),
        resolveTarget: msaa ? this.hdrTarget!.createView() : undefined,
        clearValue: { r: bg.r, g: bg.g, b: bg.b, a: 1 },
        loadOp: 'clear',
        storeOp: msaa ? 'discard' : 'store',
      }],
      depthStencilAttachment: { view: this.depthTarget!.createView(), depthClearValue: 0, depthLoadOp: 'clear', depthStoreOp: 'store' },
      timestampWrites: this.timer.pass('main'),
    })
    pass.setBindGroup(0, this.sceneGroup!)
    pass.setBindGroup(1, this.lightingGroup!)
    pass.setVertexBuffer(0, this.arena.vertexBuffer)
    pass.setIndexBuffer(this.arena.indexBuffer, 'uint32')
    let current = -1
    for (const batch of this.mainBatches) {
      if (batch.pipeline !== current) {
        pass.setPipeline(this.pipelines[batch.pipeline]!)
        current = batch.pipeline
      }
      pass.drawIndexed(batch.slot.indexCount, batch.count, batch.slot.firstIndex, batch.slot.baseVertex, batch.first)
      this.stats.drawCalls++
      this.stats.triangles += (batch.slot.indexCount / 3) * batch.count
    }
    pass.end()
  }

  private outputPass(encoder: GPUCommandEncoder, scene: Scene) {
    this.device.queue.writeBuffer(this.outputUniform, 0, new Float32Array([scene.toneMapping === 'neutral' ? 1 : 0, scene.exposure, 0, 0]))
    const pass = encoder.beginRenderPass({
      label: 'output',
      colorAttachments: [{ view: this.context.getCurrentTexture().createView(), loadOp: 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 1 } }],
      timestampWrites: this.timer.pass('output'),
    })
    pass.setPipeline(this.outputPipeline)
    pass.setBindGroup(0, this.outputGroup!)
    pass.draw(3)
    pass.end()
  }

  private makeObjectBuffer(records: number) {
    return this.device.createBuffer({
      label: `objects ${records}`,
      size: records * OBJECT_FLOATS * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    })
  }

  private makeShadowMap(size: number) {
    return this.device.createTexture({
      label: 'shadow map',
      size: [size, size],
      format: DEPTH_FORMAT,
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    })
  }

  destroy() {
    this.device.destroy()
  }
}

function byStateThenGeometry(a: DrawItem, b: DrawItem) {
  return a.order - b.order || a.pipeline - b.pipeline || a.slot.firstIndex - b.slot.firstIndex
}

function byOrderThenFarToNear(a: DrawItem, b: DrawItem) {
  return a.order - b.order || b.depth - a.depth
}

/** Bounds of all of an instanced mesh's instances in its own space, recomputed when it changes. */
function instanceBounds(mesh: InstancedMesh) {
  if (mesh.boundsVersion === mesh.version) return mesh.bounds
  const g = mesh.geometry.bounds
  const m = mesh.matrices
  let minX = Infinity, minY = Infinity, minZ = Infinity, maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity
  let reach = 0
  for (let i = 0; i < mesh.count; i++) {
    const o = i * 16
    const x = m[o]! * g.x + m[o + 4]! * g.y + m[o + 8]! * g.z + m[o + 12]!
    const y = m[o + 1]! * g.x + m[o + 5]! * g.y + m[o + 9]! * g.z + m[o + 13]!
    const z = m[o + 2]! * g.x + m[o + 6]! * g.y + m[o + 10]! * g.z + m[o + 14]!
    minX = Math.min(minX, x); minY = Math.min(minY, y); minZ = Math.min(minZ, z)
    maxX = Math.max(maxX, x); maxY = Math.max(maxY, y); maxZ = Math.max(maxZ, z)
    reach = Math.max(reach, g.radius * mat4.maxScale(m.subarray(o, o + 16)))
  }
  const b = mesh.bounds
  b.x = (minX + maxX) / 2
  b.y = (minY + maxY) / 2
  b.z = (minZ + maxZ) / 2
  b.radius = Math.hypot(maxX - b.x, maxY - b.y, maxZ - b.z) + reach
  mesh.boundsVersion = mesh.version
  return b
}
