import { Effect, type EffectContext } from './fx/effect'
import { GeometryArena, VERTEX_LAYOUT, type GeometrySlot } from './gpu/arena'
import { BoneTable } from './gpu/bones'
import { MaterialTable } from './gpu/materials'
import { ObjectTable } from './gpu/objects'
import { MaterialResources } from './gpu/resources'
import { TextureStore } from './gpu/textures'
import { GpuTimer } from './gpu/timer'
import { SHADOW_WGSL } from './materials/shadow.wgsl'
import { STANDARD_WGSL } from './materials/standard.wgsl'
import { Frustum } from './math/frustum'
import * as mat4 from './math/mat4'
import { outputWgsl } from './passes/output.wgsl'
import type { PerspectiveCamera } from './scene/camera'
import { ShaderMaterial, type Material } from './scene/material'
import { InstancedMesh, Mesh } from './scene/mesh'
import type { Node } from './scene/node'
import type { Scene } from './scene/scene'
import { SkinnedMesh } from './scene/skin'

/** Floats in the frame uniform block. Matches `Frame` in common.wgsl (padded to 384 bytes). */
const FRAME_FLOATS = 96
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
  /** Instances drawn in all passes (an instanced mesh counts each instance). */
  instances = 0
  /** Object records alive, and how many were rewritten this frame (moved, retinted, new). */
  records = 0
  recordsWritten = 0
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
  /** First object record and how many (instances). */
  record: number
  count: number
  pipeline: number
  /** The material's resource bind group (textures, or a ShaderMaterial's uniforms). */
  group: number
  order: number
  depth: number
}

/** A run of consecutive draw-list entries drawn with one call. */
interface Batch {
  pipeline: number
  group: number
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
  /** Drawing-buffer pixels per CSS pixel, from the last `setSize`. */
  pixelRatio = 1
  private readonly outputData = new Float32Array(12)

  private readonly context: GPUCanvasContext
  private readonly arena: GeometryArena
  private readonly materials: MaterialTable
  private readonly timer: GpuTimer
  private readonly frameData = new Float32Array(FRAME_FLOATS)
  private readonly frameBuffer: GPUBuffer
  private readonly objects: ObjectTable
  private readonly bones: BoneTable
  private drawList = new Uint32Array(4096)
  private drawListBuffer: GPUBuffer
  private drawCount = 0
  private frameNumber = 0
  private readonly sceneLayout: GPUBindGroupLayout
  private readonly lightingLayout: GPUBindGroupLayout
  private readonly mainLayout: GPUPipelineLayout
  private readonly shadowLayout: GPUPipelineLayout
  private readonly standardModule: GPUShaderModule
  private readonly shadowModule: GPUShaderModule
  private readonly textures: TextureStore
  private readonly resources: MaterialResources
  private readonly pipelines: GPURenderPipeline[] = []
  /** Per pipeline: true for ShaderMaterial pipelines, whose group 1 is the material's own. */
  private readonly customPipeline: boolean[] = []
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
  private readonly effects: Effect[] = []
  /** Bumped when the render targets are recreated, for effects holding views of them. */
  private targetsVersion = 0
  private readonly mainBatches: Batch[] = []
  private readonly shadowBatches: Batch[] = []
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
    this.textures = new TextureStore(device)
    this.resources = new MaterialResources(device, this.textures)
    this.timer = new GpuTimer(device)
    this.frameBuffer = device.createBuffer({ label: 'frame', size: FRAME_FLOATS * 4, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.objects = new ObjectTable(device)
    this.bones = new BoneTable(device)
    this.drawListBuffer = this.makeDrawListBuffer(this.drawList.length)
    this.comparisonSampler = device.createSampler({ compare: 'less-equal', magFilter: 'linear', minFilter: 'linear' })
    this.shadowMap = this.makeShadowMap(1)

    const both = GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT
    this.sceneLayout = device.createBindGroupLayout({
      label: 'scene',
      entries: [
        { binding: 0, visibility: both, buffer: { type: 'uniform' } },
        { binding: 1, visibility: both, buffer: { type: 'read-only-storage' } },
        { binding: 2, visibility: both, buffer: { type: 'read-only-storage' } },
        { binding: 3, visibility: GPUShaderStage.VERTEX, buffer: { type: 'read-only-storage' } },
        { binding: 4, visibility: GPUShaderStage.VERTEX, buffer: { type: 'read-only-storage' } },
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
    this.mainLayout = device.createPipelineLayout({ bindGroupLayouts: [this.sceneLayout, this.lightingLayout, this.resources.texturesLayout] })
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

    const outputModule = device.createShaderModule({ label: 'output', code: outputWgsl(this.sampleCount > 1) })
    this.outputPipeline = device.createRenderPipeline({
      label: 'output',
      layout: 'auto',
      vertex: { module: outputModule, entryPoint: 'vs' },
      fragment: { module: outputModule, entryPoint: 'fs', targets: [{ format: this.format }] },
      primitive: { topology: 'triangle-list' },
    })
    this.outputUniform = device.createBuffer({ size: 48, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.setSize(canvas.clientWidth || 1, canvas.clientHeight || 1, 1)
  }

  /** Sizes the canvas's drawing buffer to `width`×`height` CSS pixels at `pixelRatio`. */
  setSize(width: number, height: number, pixelRatio = 1) {
    const w = Math.max(1, Math.floor(width * pixelRatio))
    const h = Math.max(1, Math.floor(height * pixelRatio))
    if (w === this.width && h === this.height && this.hdrTarget) return
    this.width = w
    this.height = h
    this.pixelRatio = pixelRatio
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
    this.targetsVersion++
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
        { binding: 2, resource: this.depthTarget.createView() },
      ],
    })
  }

  render(scene: Scene, camera: PerspectiveCamera) {
    const start = performance.now()
    const stats = this.stats
    this.frameNumber++
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
    this.effects.length = 0
    this.collect(scene)
    this.effects.sort((a, b) => a.renderOrder - b.renderOrder)
    this.opaque.sort(byStateThenGeometry)
    this.transparent.sort(byOrderThenFarToNear)
    this.casters.sort(byStateThenGeometry)

    this.drawCount = 0
    this.mainBatches.length = 0
    this.shadowBatches.length = 0
    this.writeBatches(this.opaque, this.mainBatches, true)
    this.writeBatches(this.transparent, this.mainBatches, false)
    this.writeBatches(this.casters, this.shadowBatches, true)

    this.writeFrame(scene, camera)
    this.materials.flush()
    this.objects.flush()
    this.bones.flush()
    this.uploadDrawList()
    this.bindScene()

    const encoder = this.device.createCommandEncoder()
    this.timer.begin()
    stats.drawCalls = 0
    stats.triangles = 0
    stats.shadowDraws = 0
    const effects = this.effectContext()
    for (const effect of this.effects) effect.prepare(effects, encoder)
    if (this.shadowsOn) this.shadowPass(encoder)
    this.mainPass(encoder, scene)
    if (this.effects.length) this.effectsPass(encoder, effects)
    this.outputPass(encoder, scene, camera)
    this.timer.resolve(encoder)
    this.device.queue.submit([encoder.finish()])
    this.timer.read()
    for (const effect of this.effects) effect.afterSubmit()
    this.materials.collect()
    this.objects.sweep(this.frameNumber)
    this.bones.sweep(this.frameNumber)
    this.resources.sweep()

    stats.instances = this.drawCount
    stats.records = this.objects.count
    stats.recordsWritten = this.objects.written
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
      else if (node instanceof Effect) this.effects.push(node)
      const children = node.children
      for (let i = 0; i < children.length; i++) stack.push(children[i]!)
    }
  }

  private consider(mesh: Mesh) {
    if (mesh instanceof InstancedMesh && mesh.count <= 0) return
    const slot = this.arena.sync(mesh.geometry)
    if (!slot || !slot.indexCount) return
    let m = mesh.matrixWorld
    let bx: number, by: number, bz: number, br: number
    if (mesh instanceof SkinnedMesh) {
      // The bones place a skinned mesh: centre the bind-pose bounds on its root bone, with room to move.
      const root = mesh.skeleton.bones[0]
      if (root) m = root.matrixWorld
      const b = mesh.geometry.bounds
      bx = 0; by = 0; bz = 0; br = b.radius * 1.5 + Math.hypot(b.x, b.y, b.z)
    } else if (mesh instanceof InstancedMesh) {
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
    const inView = !mesh.frustumCulled || this.frustum.intersectsSphere(cx, cy, cz, radius)
    const inShadow = this.shadowsOn && mesh.castShadow && !material.transparent &&
      (!mesh.frustumCulled || this.shadowFrustum.intersectsSphere(cx, cy, cz, radius))
    if (!inView && !inShadow) return
    const bones = mesh instanceof SkinnedMesh ? this.bones.sync(mesh, this.frameNumber) + 1 : 0
    const record = this.objects.sync(mesh, this.materials.sync(material), this.frameNumber, bones)
    const count = mesh instanceof InstancedMesh ? mesh.count : 1
    if (inView) {
      const group = material instanceof ShaderMaterial ? this.resources.custom(material) : this.resources.standard(material)
      const item = this.item(mesh, slot, record, count, this.pipelineFor(material), group)
      const dx = cx - this.camX, dy = cy - this.camY, dz = cz - this.camZ
      item.depth = dx * dx + dy * dy + dz * dz
      ;(material.transparent ? this.transparent : this.opaque).push(item)
    }
    if (inShadow) this.casters.push(this.item(mesh, slot, record, count, material.side === 'front' ? 0 : 1, 0))
  }

  private item(mesh: Mesh, slot: GeometrySlot, record: number, count: number, pipeline: number, group: number): DrawItem {
    let item = this.pool[this.poolUsed]
    if (!item) {
      item = { mesh, slot, record, count, pipeline, group, order: 0, depth: 0 }
      this.pool.push(item)
    }
    this.poolUsed++
    item.mesh = mesh
    item.slot = slot
    item.record = record
    item.count = count
    item.pipeline = pipeline
    item.group = group
    item.order = mesh.renderOrder
    item.depth = 0
    return item
  }

  /** Appends `items` to the draw list in order, merging runs that can share one draw when `merge` is set. */
  private writeBatches(items: DrawItem[], batches: Batch[], merge: boolean) {
    for (const item of items) {
      const first = this.drawCount
      const count = item.count
      if (first + count > this.drawList.length) {
        let size = this.drawList.length
        while (size < first + count) size *= 2
        const list = new Uint32Array(size)
        list.set(this.drawList)
        this.drawList = list
      }
      const list = this.drawList
      for (let i = 0; i < count; i++) list[first + i] = item.record + i
      this.drawCount += count
      const last = batches[batches.length - 1]
      if (merge && last && last.pipeline === item.pipeline && last.group === item.group && last.slot === item.slot && last.first + last.count === first) {
        last.count += count
      } else {
        batches.push({ pipeline: item.pipeline, group: item.group, slot: item.slot, first, count })
      }
    }
  }

  // ---- pipelines ----

  private pipelineFor(material: Material): number {
    const cached = this.materialPipelines.get(material.id)
    if (cached && cached.version === material.version) return cached.pipeline
    const program = material instanceof ShaderMaterial ? this.resources.program(material) : null
    const state = `${material.blend}|${material.side}|${material.depthTest}|${material.depthWrite}|${material.depthBias}|${material.depthBiasSlope}`
    const key = program ? `${program.shader.key}|${state}` : state
    let pipeline = this.pipelineKeys.get(key)
    if (pipeline === undefined) {
      pipeline = this.pipelines.length
      const layout = program
        ? this.device.createPipelineLayout({ bindGroupLayouts: [this.sceneLayout, program.groupLayout] })
        : this.mainLayout
      this.pipelines.push(this.makePipeline(material, program ? 'shader material' : state, program?.module ?? this.standardModule, layout))
      this.customPipeline.push(program !== null)
      this.pipelineKeys.set(key, pipeline)
    }
    this.materialPipelines.set(material.id, { version: material.version, pipeline })
    return pipeline
  }

  private makePipeline(m: Material, label: string, module: GPUShaderModule, layout: GPUPipelineLayout): GPURenderPipeline {
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
      layout,
      vertex: { module, entryPoint: 'vs', buffers: [VERTEX_LAYOUT] },
      fragment: { module, entryPoint: 'fs', targets: [{ format: HDR_FORMAT, blend }] },
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
    f[84] = camera.near
    f[85] = camera.far
    this.device.queue.writeBuffer(this.frameBuffer, 0, f)
  }

  private uploadDrawList() {
    if (this.drawList.length * 4 > this.drawListBuffer.size) {
      this.drawListBuffer.destroy()
      this.drawListBuffer = this.makeDrawListBuffer(this.drawList.length)
    }
    if (this.drawCount) this.device.queue.writeBuffer(this.drawListBuffer, 0, this.drawList, 0, this.drawCount)
  }

  private bindScene() {
    const key = `${this.objects.generation}:${this.materials.generation}:${this.bones.generation}:${this.drawListBuffer.size}`
    if (!this.sceneGroup || key !== this.sceneGroupKey) {
      this.sceneGroupKey = key
      this.sceneGroup = this.device.createBindGroup({
        label: 'scene',
        layout: this.sceneLayout,
        entries: [
          { binding: 0, resource: { buffer: this.frameBuffer } },
          { binding: 1, resource: { buffer: this.objects.buffer } },
          { binding: 2, resource: { buffer: this.materials.buffer } },
          { binding: 3, resource: { buffer: this.drawListBuffer } },
          { binding: 4, resource: { buffer: this.bones.buffer } },
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

  private effectContext(): EffectContext {
    return {
      device: this.device,
      sceneLayout: this.sceneLayout,
      sceneGroup: this.sceneGroup!,
      depthView: this.depthTarget!.createView(),
      sampleCount: this.sampleCount,
      colorFormat: HDR_FORMAT,
      depthFormat: DEPTH_FORMAT,
      targetsVersion: this.targetsVersion,
    }
  }

  private mainPass(encoder: GPUCommandEncoder, scene: Scene) {
    const bg = scene.background
    const msaa = this.colorTarget !== null
    // With effects to come the multisampled image is kept for them and resolved at their pass's end.
    const resolveHere = msaa && !this.effects.length
    const pass = encoder.beginRenderPass({
      label: 'main',
      colorAttachments: [{
        view: msaa ? this.colorTarget!.createView() : this.hdrTarget!.createView(),
        resolveTarget: resolveHere ? this.hdrTarget!.createView() : undefined,
        clearValue: { r: bg.r, g: bg.g, b: bg.b, a: 1 },
        loadOp: 'clear',
        storeOp: resolveHere ? 'discard' : 'store',
      }],
      depthStencilAttachment: { view: this.depthTarget!.createView(), depthClearValue: 0, depthLoadOp: 'clear', depthStoreOp: 'store' },
      timestampWrites: this.timer.pass('main'),
    })
    pass.setBindGroup(0, this.sceneGroup!)
    pass.setVertexBuffer(0, this.arena.vertexBuffer)
    pass.setIndexBuffer(this.arena.indexBuffer, 'uint32')
    let current = -1
    // What is bound at groups 1 and 2: -1 for the lighting group at 1, else a resource group id.
    let group1 = -2
    let group2 = -2
    for (const batch of this.mainBatches) {
      if (batch.pipeline !== current) {
        pass.setPipeline(this.pipelines[batch.pipeline]!)
        current = batch.pipeline
      }
      if (this.customPipeline[batch.pipeline]) {
        if (group1 !== batch.group) {
          pass.setBindGroup(1, this.resources.group(batch.group))
          group1 = batch.group
        }
      } else {
        if (group1 !== -1) {
          pass.setBindGroup(1, this.lightingGroup!)
          group1 = -1
        }
        if (group2 !== batch.group) {
          pass.setBindGroup(2, this.resources.group(batch.group))
          group2 = batch.group
        }
      }
      pass.drawIndexed(batch.slot.indexCount, batch.count, batch.slot.firstIndex, batch.slot.baseVertex, batch.first)
      this.stats.drawCalls++
      this.stats.triangles += (batch.slot.indexCount / 3) * batch.count
    }
    pass.end()
  }

  /** Particles and other effects, over everything else, with the scene's depth bound read-only. */
  private effectsPass(encoder: GPUCommandEncoder, ctx: EffectContext) {
    const msaa = this.colorTarget !== null
    const pass = encoder.beginRenderPass({
      label: 'effects',
      colorAttachments: [{
        view: msaa ? this.colorTarget!.createView() : this.hdrTarget!.createView(),
        resolveTarget: msaa ? this.hdrTarget!.createView() : undefined,
        loadOp: 'load',
        storeOp: msaa ? 'discard' : 'store',
      }],
      depthStencilAttachment: { view: ctx.depthView, depthReadOnly: true },
      timestampWrites: this.timer.pass('effects'),
    })
    for (const effect of this.effects) this.stats.drawCalls += effect.draw(ctx, pass)
    pass.end()
  }

  private outputPass(encoder: GPUCommandEncoder, scene: Scene, camera: PerspectiveCamera) {
    const o = this.outputData
    const ink = scene.ink
    o[0] = scene.toneMapping === 'neutral' ? 1 : 0
    o[1] = scene.exposure
    o[2] = ink ? 1 : 0
    o[3] = ink ? ink.width * this.pixelRatio : 1
    o[4] = ink?.darken ?? 0
    o[5] = ink?.strength ?? 0
    o[6] = ink?.threshold ?? 0
    o[8] = camera.near
    o[9] = camera.far
    this.device.queue.writeBuffer(this.outputUniform, 0, o)
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

  private makeDrawListBuffer(entries: number) {
    return this.device.createBuffer({
      label: 'draw list',
      size: entries * 4,
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
  return a.order - b.order || a.pipeline - b.pipeline || a.group - b.group || a.slot.firstIndex - b.slot.firstIndex
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
