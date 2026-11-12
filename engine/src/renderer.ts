import { Effect, type EffectContext } from './fx/effect'
import { GeometryArena, VERTEX_LAYOUT, type GeometrySlot } from './gpu/arena'
import { BoneTable } from './gpu/bones'
import { CraterBuffers } from './gpu/craters'
import { LightBuffers } from './gpu/lights'
import { MaterialTable } from './gpu/materials'
import { ObjectTable } from './gpu/objects'
import { MaterialResources } from './gpu/resources'
import { TextureStore, type TextureSlot } from './gpu/textures'
import { GpuTimer } from './gpu/timer'
import { SHADOW_WGSL } from './materials/shadow.wgsl'
import { STANDARD_WGSL } from './materials/standard.wgsl'
import { Frustum } from './math/frustum'
import * as mat4 from './math/mat4'
import { Bloom } from './passes/bloom'
import { SkyPass } from './passes/sky'
import { CLOUD_HEIGHT } from './materials/sky.wgsl'
import { transmittance } from './scene/atmosphere'
import { HudLayer, HudPass } from './hud/layer'
import { HalfResolution } from './passes/halfres'
import { LIGHT_CELL, LightGrid } from './scene/lightgrid'
import { outputWgsl } from './passes/output.wgsl'
import type { PerspectiveCamera } from './scene/camera'
import { ShaderMaterial, type Material } from './scene/material'
import { FADE_MARGIN, InstancedMesh, Mesh, levelAt } from './scene/mesh'
import type { Node } from './scene/node'
import { Craters } from './scene/craters'
import type { Scene } from './scene/scene'
import { Texture } from './scene/texture'
import { SkinnedMesh } from './scene/skin'

/** Where a mesh is drawn: with the world, held in front of the camera over it, or as an overlay over everything. */
type Layer = 'world' | 'held' | 'overlay'

/** Floats in the frame uniform block. Matches `Frame` in common.wgsl. */
const FRAME_FLOATS = 124
/** The sunlight on a white cloud against the atmosphere's tinted radiance for a sun of illuminance 1. */
const CLOUD_SUNLIGHT = 0.12
const HDR_FORMAT: GPUTextureFormat = 'rgba16float'
const DEPTH_FORMAT: GPUTextureFormat = 'depth32float'
/** The reflection holds nothing smaller than this share of the view's distance. */
const REFLECT_MIN_SIZE = 0.01

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
  /** Point and spot lights listed this frame (in view, on the light grid), and the cell entries they fill. */
  lights = 0
  lightEntries = 0
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
  /**
   * Which of the mesh's instances to draw, when not all of them: `count` indices from `listFirst` in
   * `list`, each added to `record`. Null draws the first `count`.
   */
  list: Uint32Array | null
  listFirst: number
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
  /** Tints lit surfaces by how many lights their light-grid cell lists, instead of lighting them. */
  showLightGrid = false
  /**
   * Multiplies the distances at which instanced meshes change to their coarser levels
   * (`InstancedMesh.levels`): under 1 they come nearer, for a slower GPU.
   */
  detail = 1
  /** Flat sprites drawn over the finished frame: fill it before each `render`, which draws and empties it. */
  hud = new HudLayer()
  private readonly hudPass: HudPass
  private readonly lightGrid = new LightGrid()
  private readonly lightVisible = (x: number, y: number, z: number, radius: number) =>
    this.frustum.intersectsSphere(x, y, z, radius) || (this.reflectionOn && this.reflectionFrustum.intersectsSphere(x, y, z, radius))
  private readonly outputData = new Float32Array(16)
  private readonly sky: SkyPass
  private readonly inverseViewProj = mat4.create()
  /** The sunlight through the atmosphere at the cloud layer, for `sunlightHeight`. */
  private readonly sunlight = [1, 1, 1]
  private sunlightHeight = NaN
  /** Where in `mainBatches` the translucent draws start: the sky goes in before them. */
  private translucentFrom = 0

  private readonly context: GPUCanvasContext
  private readonly arena: GeometryArena
  private readonly materials: MaterialTable
  private readonly timer: GpuTimer
  private readonly frameData = new Float32Array(FRAME_FLOATS)
  private readonly frameBuffer: GPUBuffer
  private readonly objects: ObjectTable
  private readonly bones: BoneTable
  private readonly craters: CraterBuffers
  private readonly lights: LightBuffers
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
  /** Per pipeline: 'standard', a ShaderMaterial's ('custom', whose group 1 is the material's own), or one that also takes the lighting group at 2 ('shadowed'). */
  private readonly pipelineKind: ('standard' | 'custom' | 'shadowed')[] = []
  private readonly pipelineKeys = new Map<string, number>()
  private readonly materialPipelines = new Map<number, { version: number; layer: Layer; pipeline: number }>()
  private readonly shadowPipelines: GPURenderPipeline[]
  private readonly outputPipeline: GPURenderPipeline
  private readonly outputUniform: GPUBuffer
  private readonly bloom: Bloom
  private readonly bloomSampler: GPUSampler
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
  /** This frame's lists of instances (`DrawItem.list`), handed out from the front. */
  private scratch = new Uint32Array(16384)
  private scratchUsed = 0
  private readonly levelSlots: GeometrySlot[] = []
  /** The same for the shadow pass, which draws a level's `shadow` where it has one. */
  private readonly levelShadowSlots: GeometrySlot[] = []
  /** How many instances each level's list holds so far: the view's lists, then the sun's. */
  private levelCounts = new Uint32Array(16)
  private levelFrom2 = new Float64Array(8)
  /** Per level, squared: where its fade band starts (Infinity for none) and where the one before it stops being drawn. */
  private levelFadeFrom2 = new Float64Array(8)
  private levelFadeTo2 = new Float64Array(8)
  private readonly opaque: DrawItem[] = []
  private readonly transparent: DrawItem[] = []
  private readonly overlay: DrawItem[] = []
  /** Held meshes: the end of the main pass, over the scene but writing depth (`Mesh.held`). */
  private readonly held: DrawItem[] = []
  /** Where in `mainBatches` the overlay's draws start. */
  private overlayFrom = 0

  /**
   * The scene mirrored in a horizontal plane, for water to sample: RGB is the scene, alpha is where it drew
   * anything (the rest is sky). Drawn only while `reflectionHeight` is a world Y; set it to null for no pass.
   * Draws opaque meshes only, within `reflectionRange` metres, at `reflectionScale` of the screen's size.
   */
  readonly reflection = new Texture({ data: new Uint8Array(4), width: 1, height: 1 }, { srgb: false, wrap: 'clamp', mipmaps: false })
  reflectionHeight: number | null = null
  reflectionScale = 0.5
  reflectionRange = 300
  private reflectionOn = false
  private reflectionTarget: { width: number; height: number; color: GPUTexture | null; depth: GPUTexture; resolve: GPUTexture } | null = null
  private readonly reflectionFrame = new Float32Array(FRAME_FLOATS)
  private readonly reflectionFrameBuffer: GPUBuffer
  private reflectionGroup: GPUBindGroup | null = null
  private readonly reflectionFrustum = new Frustum()
  private readonly reflectionView = mat4.create()
  private readonly reflectionProjection = mat4.create()
  private readonly reflectionViewProjection = mat4.create()
  private readonly mirror = mat4.create()
  private readonly reflected: DrawItem[] = []
  private readonly reflectionBatches: Batch[] = []
  private readonly casters: DrawItem[] = []
  private readonly effects: Effect[] = []
  /** Bumped when the render targets are recreated, for effects holding views of them. */
  private targetsVersion = 0
  private readonly halfResolution: HalfResolution
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
    // The geometry arena is one vertex buffer; take what the GPU allows rather than the 256 MB default.
    const requiredLimits = { maxBufferSize: adapter.limits.maxBufferSize }
    const device = await adapter.requestDevice({ requiredFeatures, requiredLimits })
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
    this.reflectionFrameBuffer = device.createBuffer({ label: 'reflection frame', size: FRAME_FLOATS * 4, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.objects = new ObjectTable(device)
    this.bones = new BoneTable(device)
    this.craters = new CraterBuffers(device, new Craters())
    this.lights = new LightBuffers(device, this.lightGrid)
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
        { binding: 5, visibility: both, buffer: { type: 'read-only-storage' } },
        { binding: 6, visibility: both, buffer: { type: 'read-only-storage' } },
        { binding: 7, visibility: GPUShaderStage.FRAGMENT, buffer: { type: 'read-only-storage' } },
        { binding: 8, visibility: GPUShaderStage.FRAGMENT, buffer: { type: 'read-only-storage' } },
        { binding: 9, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'float' } },
        { binding: 10, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'float' } },
        { binding: 11, visibility: GPUShaderStage.FRAGMENT, sampler: { type: 'filtering' } },
        { binding: 12, visibility: GPUShaderStage.FRAGMENT, sampler: { type: 'filtering' } },
      ],
    })
    this.sky = new SkyPass(device, this.sceneLayout, this.sampleCount, HDR_FORMAT, DEPTH_FORMAT)
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
    this.bloom = new Bloom(device)
    this.halfResolution = new HalfResolution(device, HDR_FORMAT)
    this.bloomSampler = device.createSampler({ magFilter: 'linear', minFilter: 'linear' })
    this.outputUniform = device.createBuffer({ size: 64, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.hudPass = new HudPass(device, this.format, this.textures)
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
    this.bloom.resize(w, h, this.hdrTarget)
    this.halfResolution.release()
    this.outputGroup = this.device.createBindGroup({
      layout: this.outputPipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: this.outputUniform } },
        { binding: 1, resource: this.hdrTarget.createView() },
        { binding: 2, resource: this.depthTarget.createView() },
        { binding: 3, resource: this.bloom.view },
        { binding: 4, resource: this.bloomSampler },
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
    this.setupReflection(camera)

    this.poolUsed = 0
    this.scratchUsed = 0
    this.reflected.length = 0
    this.opaque.length = 0
    this.transparent.length = 0
    this.overlay.length = 0
    this.held.length = 0
    this.casters.length = 0
    this.effects.length = 0
    this.collect(scene)
    this.effects.sort((a, b) => a.renderOrder - b.renderOrder)
    this.opaque.sort(byStateThenGeometry)
    this.transparent.sort(byOrderThenFarToNear)
    this.overlay.sort(byOrderThenFarToNear)
    this.held.sort(byOrderThenFarToNear)
    this.casters.sort(byStateThenGeometry)
    this.reflected.sort(byStateThenGeometry)

    this.drawCount = 0
    this.mainBatches.length = 0
    this.reflectionBatches.length = 0
    this.shadowBatches.length = 0
    this.writeBatches(this.opaque, this.mainBatches, true)
    this.translucentFrom = this.mainBatches.length
    this.writeBatches(this.transparent, this.mainBatches, false)
    this.writeBatches(this.held, this.mainBatches, false)
    this.overlayFrom = this.mainBatches.length
    this.writeBatches(this.overlay, this.mainBatches, false)
    this.writeBatches(this.casters, this.shadowBatches, true)
    this.writeBatches(this.reflected, this.reflectionBatches, true)

    this.writeFrame(scene, camera)
    this.materials.flush()
    this.objects.flush()
    this.bones.flush()
    this.uploadDrawList()
    this.craters.upload(scene.craters)
    this.lights.upload(this.lightGrid)
    this.bindScene()

    const encoder = this.device.createCommandEncoder()
    this.timer.begin()
    stats.drawCalls = 0
    stats.triangles = 0
    stats.shadowDraws = 0
    const effects = this.effectContext()
    for (const effect of this.effects) effect.prepare(effects, encoder)
    if (scene.sky) {
      this.sky.update(encoder, scene.sky)
      if (mat4.invert(this.inverseViewProj, camera.viewProjection)) this.sky.setView(this.inverseViewProj)
    }
    if (this.shadowsOn) this.shadowPass(encoder)
    if (this.reflectionOn) this.reflectionPass(encoder, scene)
    this.mainPass(encoder, scene)
    if (this.effects.length) {
      const half = this.effects.some((e) => e.halfResolution)
      if (half) this.halfResolutionPass(encoder, effects)
      this.effectsPass(encoder, effects, half)
    }
    const bloom = scene.bloom
    if (bloom && bloom.strength > 0) this.bloom.encode(encoder, bloom.threshold, bloom.knee, bloom.radius)
    const screen = this.context.getCurrentTexture().createView()
    this.outputPass(encoder, scene, camera, screen)
    this.timer.resolve(encoder)
    this.device.queue.submit([encoder.finish()])
    this.timer.read()
    for (const effect of this.effects) effect.afterSubmit()
    this.materials.collect()
    this.arena.collect()
    this.textures.collect()
    this.objects.sweep(this.frameNumber)
    this.bones.sweep(this.frameNumber)
    this.resources.sweep()

    stats.instances = this.drawCount
    stats.records = this.objects.count
    stats.recordsWritten = this.objects.written
    stats.pipelines = this.pipelines.length + this.shadowPipelines.length + 1
    stats.materials = this.materials.count
    stats.geometryBytes = this.arena.usedBytes
    stats.lights = this.lightGrid.count
    stats.lightEntries = this.lightGrid.indexCount
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
    const layer: Layer = mesh.overlay ? 'overlay' : mesh.held ? 'held' : 'world'
    const inShadow = this.shadowsOn && mesh.castShadow && !material.transparent && layer === 'world' &&
      (!mesh.frustumCulled || this.shadowFrustum.intersectsSphere(cx, cy, cz, radius))
    const dx = cx - this.camX, dy = cy - this.camY, dz = cz - this.camZ
    const distance2 = dx * dx + dy * dy + dz * dz
    // Only what rises above the mirror, is near and big enough to show, and falls in the mirrored view.
    const inReflection = this.reflectionOn && !material.transparent && layer === 'world' &&
      cy + radius > this.reflectionHeight! && distance2 < this.reflectionRange * this.reflectionRange &&
      radius * radius > distance2 * (REFLECT_MIN_SIZE * REFLECT_MIN_SIZE) &&
      (!mesh.frustumCulled || this.reflectionFrustum.intersectsSphere(cx, cy, cz, radius))
    if (!inView && !inShadow && !inReflection) return
    const bones = mesh instanceof SkinnedMesh ? this.bones.sync(mesh, this.frameNumber) + 1 : 0
    const record = this.objects.sync(mesh, this.materials.sync(material), this.frameNumber, bones)
    if (mesh instanceof InstancedMesh && mesh.levels) {
      return this.considerLevels(mesh, mesh.levels, slot, record, layer, inView, inShadow, inReflection, Math.max(0, Math.sqrt(distance2) - radius), distance2)
    }
    const count = mesh instanceof InstancedMesh ? mesh.count : 1
    if (inView || inReflection) {
      const group = material instanceof ShaderMaterial ? this.resources.custom(material) : this.resources.standard(material)
      const pipeline = this.pipelineFor(material, layer)
      if (inView) {
        const item = this.item(mesh, slot, record, count, pipeline, group)
        item.depth = distance2
        ;(layer === 'overlay' ? this.overlay : layer === 'held' ? this.held : material.transparent ? this.transparent : this.opaque).push(item)
      }
      if (inReflection) this.reflected.push(this.item(mesh, slot, record, count, pipeline, group))
    }
    if (inShadow) this.casters.push(this.item(mesh, slot, record, count, material.side === 'front' ? 0 : 1, 0))
  }

  /**
   * An instanced mesh with levels of detail, already found to be in the view, the sun's or the
   * reflection's: each instance is tested on its own and listed under the level its distance picks.
   * `nearest` is how close to the camera the mesh's bounds come, metres.
   */
  private considerLevels(
    mesh: InstancedMesh, levels: NonNullable<InstancedMesh['levels']>, slot: GeometrySlot, record: number, layer: Layer,
    inView: boolean, inShadow: boolean, inReflection: boolean, nearest: number, distance2: number,
  ) {
    const material = mesh.material
    const last = levels.length
    const slots = this.levelSlots
    const shadowSlots = this.levelShadowSlots
    slots.length = 0
    shadowSlots.length = 0
    slots.push(slot)
    shadowSlots.push(slot)
    // A level whose geometry is empty falls back on the one before it.
    for (let i = 0; i < last; i++) {
      const level = levels[i]!
      const s = this.arena.sync(level.geometry)
      slots.push(s && s.indexCount ? s : slots[i]!)
      const shadow = level.shadow ? this.arena.sync(level.shadow) : s
      shadowSlots.push(shadow && shadow.indexCount ? shadow : shadowSlots[i]!)
    }
    const shadowPipeline = material.side === 'front' ? 0 : 1
    const shadowLevel = Math.min(mesh.shadowLevel, last)
    let pipeline = 0, group = 0
    if (inView || inReflection) {
      group = material instanceof ShaderMaterial ? this.resources.custom(material) : this.resources.standard(material)
      pipeline = this.pipelineFor(material, layer)
      // The mirror image is small and blurred by the water: all of the mesh at its coarsest.
      if (inReflection) this.reflected.push(this.item(mesh, slots[last]!, record, mesh.count, pipeline, group))
    }
    if (!inView && !inShadow) return
    const into = layer === 'overlay' ? this.overlay : layer === 'held' ? this.held : material.transparent ? this.transparent : this.opaque
    // All of it past the last change of level (and the fade into it): one draw of the whole mesh, with no instance looked at.
    const lastFade = last > 0 && levels[last - 1]!.fade ? FADE_MARGIN : 0
    if (last > 0 && levelAt(nearest * nearest, levels, this.detail) === last && nearest >= levels[last - 1]!.from * this.detail + lastFade) {
      if (inView) {
        const item = this.item(mesh, slots[last]!, record, mesh.count, pipeline, group)
        item.depth = distance2
        into.push(item)
      }
      if (inShadow) this.casters.push(this.item(mesh, shadowSlots[last]!, record, mesh.count, shadowPipeline, 0))
      return
    }
    const count = mesh.count
    const spheres = instanceSpheres(mesh)
    // Room for every instance in every level's list, for the view and then for the sun.
    const lists = last + 1
    const first = this.reserve(lists * 2 * count)
    const scratch = this.scratch
    if (this.levelCounts.length < lists * 2) this.levelCounts = new Uint32Array(lists * 2)
    const counts = this.levelCounts
    counts.fill(0)
    // Each level's distance, squared, so an instance's level is a compare or two.
    if (this.levelFrom2.length < last) this.levelFrom2 = new Float64Array(last)
    const from2 = this.levelFrom2
    if (this.levelFadeFrom2.length < last) {
      this.levelFadeFrom2 = new Float64Array(last)
      this.levelFadeTo2 = new Float64Array(last)
    }
    const fadeFrom2 = this.levelFadeFrom2, fadeTo2 = this.levelFadeTo2
    let fades = false
    for (let i = 0; i < last; i++) {
      const level = levels[i]!
      from2[i] = (level.from * this.detail) ** 2
      // Inside a fade band an instance is listed at both levels; the material dithers between them.
      fadeFrom2[i] = level.fade ? Math.max(0, (level.from - level.fade) * this.detail - FADE_MARGIN) ** 2 : Infinity
      fadeTo2[i] = level.fade ? (level.from * this.detail + FADE_MARGIN) ** 2 : -1
      if (level.fade) fades = true
    }
    const frustum = this.frustum, shadowFrustum = this.shadowFrustum
    const culled = mesh.frustumCulled
    for (let i = 0; i < count; i++) {
      const o = i * 4
      const r = spheres[o + 3]!
      // An instance scaled to nothing (a felled tree) is not there.
      if (r <= 0) continue
      const x = spheres[o]!, y = spheres[o + 1]!, z = spheres[o + 2]!
      const dx = x - this.camX, dy = y - this.camY, dz = z - this.camZ
      const d2 = dx * dx + dy * dy + dz * dz
      let level = 0
      while (level < last && d2 >= from2[level]!) level++
      if (inView && (!culled || frustum.intersectsSphere(x, y, z, r))) {
        scratch[first + level * count + counts[level]!++] = i
        if (fades) {
          const also = level < last && d2 >= fadeFrom2[level]! ? level + 1 : level > 0 && d2 < fadeTo2[level - 1]! ? level - 1 : -1
          if (also >= 0) scratch[first + also * count + counts[also]!++] = i
        }
      }
      if (inShadow && (!culled || shadowFrustum.intersectsSphere(x, y, z, r))) {
        const cast = lists + Math.max(level, shadowLevel)
        scratch[first + cast * count + counts[cast]!++] = i
      }
    }
    for (let level = 0; level < lists; level++) {
      const seen = counts[level]!
      if (seen) {
        const item = this.item(mesh, slots[level]!, record, seen, pipeline, group)
        item.depth = distance2
        item.list = scratch
        item.listFirst = first + level * count
        into.push(item)
      }
      const cast = counts[lists + level]!
      if (cast) {
        const item = this.item(mesh, shadowSlots[level]!, record, cast, shadowPipeline, 0)
        item.list = scratch
        item.listFirst = first + (lists + level) * count
        this.casters.push(item)
      }
    }
  }

  /** `entries` of `scratch` for this frame, as the index of the first. Lists handed out before a growth keep the old array. */
  private reserve(entries: number): number {
    if (this.scratchUsed + entries > this.scratch.length) {
      this.scratch = new Uint32Array(Math.max(this.scratch.length * 2, entries * 2))
      this.scratchUsed = 0
    }
    const first = this.scratchUsed
    this.scratchUsed += entries
    return first
  }

  private item(mesh: Mesh, slot: GeometrySlot, record: number, count: number, pipeline: number, group: number): DrawItem {
    let item = this.pool[this.poolUsed]
    if (!item) {
      item = { mesh, slot, record, count, pipeline, group, order: 0, depth: 0, list: null, listFirst: 0 }
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
    item.list = null
    item.listFirst = 0
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
      const some = item.list
      if (some) {
        const from = item.listFirst
        for (let i = 0; i < count; i++) list[first + i] = item.record + some[from + i]!
      } else {
        for (let i = 0; i < count; i++) list[first + i] = item.record + i
      }
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

  private pipelineFor(material: Material, layer: Layer): number {
    const cached = this.materialPipelines.get(material.id)
    if (cached && cached.version === material.version && cached.layer === layer) return cached.pipeline
    const program = material instanceof ShaderMaterial ? this.resources.program(material) : null
    // An overlay ignores depth: it neither tests nor writes, which also lets it draw against a read-only depth.
    // A held mesh does not test either, but writes: what comes after is behind it, and the ink outlines it.
    const state = layer !== 'world'
      ? `${material.blend}|${material.side}|${layer}`
      : `${material.blend}|${material.side}|${material.depthTest}|${material.depthWrite}|${material.depthBias}|${material.depthBiasSlope}`
    const key = program ? `${program.shader.key}|${state}` : state
    let pipeline = this.pipelineKeys.get(key)
    if (pipeline === undefined) {
      pipeline = this.pipelines.length
      const layout = program
        ? this.device.createPipelineLayout({ bindGroupLayouts: (material as ShaderMaterial).shadows ? [this.sceneLayout, program.groupLayout, this.lightingLayout] : [this.sceneLayout, program.groupLayout] })
        : this.mainLayout
      this.pipelines.push(this.makePipeline(material, layer, program ? 'shader material' : state, program?.module ?? this.standardModule, layout))
      this.pipelineKind.push(program === null ? 'standard' : (material as ShaderMaterial).shadows ? 'shadowed' : 'custom')
      this.pipelineKeys.set(key, pipeline)
    }
    this.materialPipelines.set(material.id, { version: material.version, layer, pipeline })
    return pipeline
  }

  private makePipeline(m: Material, layer: Layer, label: string, module: GPUShaderModule, layout: GPUPipelineLayout): GPURenderPipeline {
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
        depthWriteEnabled: layer === 'held' || (m.depthWrite && layer === 'world'),
        // Reversed depth: nearer is larger.
        depthCompare: m.depthTest && layer === 'world' ? 'greater-equal' : 'always',
        // Positive bias pulls toward the camera, which with reversed depth means a larger value.
        depthBias: layer !== 'world' ? 0 : Math.round(m.depthBias),
        depthBiasSlopeScale: layer !== 'world' ? 0 : m.depthBiasSlope,
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
    f[83] = shadow.taps === 1 ? 1 : 0
    f[84] = camera.near
    f[85] = camera.far
    this.writeLights(scene, f)
    this.writeSky(scene, f)
    // The point-light count's neighbours are the clip: below `clip height` is cut away while mirroring.
    f[89] = 0
    f[90] = 0
    this.device.queue.writeBuffer(this.frameBuffer, 0, f)
    if (this.reflectionOn) {
      const r = this.reflectionFrame
      const h = this.reflectionHeight!
      r.set(f)
      r.set(this.reflectionViewProjection, 0)
      r.set(this.reflectionView, 16)
      r[33] = 2 * h - this.camY
      r[89] = h - 0.05
      r[90] = 1
      this.device.queue.writeBuffer(this.reflectionFrameBuffer, 0, r)
    }
  }

  /** The sky's part of the frame block (see `Frame` in common.wgsl): all zero when there is none. */
  private writeSky(scene: Scene, f: Float32Array) {
    const sky = scene.sky
    if (!sky) {
      f.fill(0, 96, 124)
      return
    }
    const s = sky.sunDir
    const length = Math.hypot(s.x, s.y, s.z) || 1
    f[96] = s.x / length
    f[97] = s.y / length
    f[98] = s.z / length
    f[99] = sky.sunDisc
    const height = s.y / length
    if (!(Math.abs(height - this.sunlightHeight) < 1e-4)) {
      this.sunlightHeight = height
      transmittance(height, this.sunlight, CLOUD_HEIGHT / 1000)
    }
    const tint = sky.tint
    // The tint's brightness only: its per-channel balance is the sky's, and a cloud in sunlight is white.
    const bright = (0.2126 * tint.r + 0.7152 * tint.g + 0.0722 * tint.b) * CLOUD_SUNLIGHT
    f[100] = this.sunlight[0]! * bright
    f[101] = this.sunlight[1]! * bright
    f[102] = this.sunlight[2]! * bright
    f[103] = sky.moon
    const m = sky.moonDir
    const moonLength = Math.hypot(m.x, m.y, m.z) || 1
    f[104] = m.x / moonLength
    f[105] = m.y / moonLength
    f[106] = m.z / moonLength
    f[107] = sky.moonPhase
    f[108] = sky.coverage
    f[109] = sky.drift[0]
    f[110] = sky.drift[1]
    f[111] = sky.cloudShadow
    f[112] = tint.r
    f[113] = tint.g
    f[114] = tint.b
    f[115] = sky.stars
    f[116] = sky.night.r
    f[117] = sky.night.g
    f[118] = sky.night.b
    f[119] = sky.starTurn
    f[120] = sky.fogDensity
    f[121] = sky.fogBase
    f[122] = sky.fogFalloff
    f[123] = 1
  }

  /** Bins the scene's lights into the light grid around the camera, for every view drawn this frame. */
  private writeLights(scene: Scene, f: Float32Array) {
    const grid = this.lightGrid
    grid.build(scene.lights.live, this.camX, this.camY, this.camZ, this.lightVisible)
    f[88] = grid.count
    f[92] = grid.originX
    f[93] = grid.originZ
    f[94] = 1 / LIGHT_CELL
    f[95] = this.showLightGrid ? 1 : 0
  }

  private uploadDrawList() {
    if (this.drawList.length * 4 > this.drawListBuffer.size) {
      this.drawListBuffer.destroy()
      this.drawListBuffer = this.makeDrawListBuffer(this.drawList.length)
    }
    if (this.drawCount) this.device.queue.writeBuffer(this.drawListBuffer, 0, this.drawList, 0, this.drawCount)
  }

  private makeSceneGroup(frame: GPUBuffer): GPUBindGroup {
    return this.device.createBindGroup({
      label: 'scene',
      layout: this.sceneLayout,
      entries: [
        { binding: 0, resource: { buffer: frame } },
        { binding: 1, resource: { buffer: this.objects.buffer } },
        { binding: 2, resource: { buffer: this.materials.buffer } },
        { binding: 3, resource: { buffer: this.drawListBuffer } },
        { binding: 4, resource: { buffer: this.bones.buffer } },
        { binding: 5, resource: { buffer: this.craters.list } },
        { binding: 6, resource: { buffer: this.craters.grid } },
        { binding: 7, resource: { buffer: this.lights.records } },
        { binding: 8, resource: { buffer: this.lights.cells } },
        { binding: 9, resource: this.sky.skyView.createView() },
        { binding: 10, resource: this.sky.cloudMap.createView() },
        { binding: 11, resource: this.sky.skySampler },
        { binding: 12, resource: this.sky.cloudSampler },
      ],
    })
  }

  private bindScene() {
    const key = `${this.objects.generation}:${this.materials.generation}:${this.bones.generation}:${this.drawListBuffer.size}`
    if (!this.sceneGroup || key !== this.sceneGroupKey) {
      this.sceneGroupKey = key
      this.sceneGroup = this.makeSceneGroup(this.frameBuffer)
      this.reflectionGroup = this.makeSceneGroup(this.reflectionFrameBuffer)
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
      depthSamples: this.sampleCount,
      half: false,
      colorFormat: HDR_FORMAT,
      depthFormat: DEPTH_FORMAT,
      targetsVersion: this.targetsVersion,
    }
  }

  /** Mirrors the view in the reflection plane and sizes the target, or turns the reflection off. */
  private setupReflection(camera: PerspectiveCamera) {
    const h = this.reflectionHeight
    // Under the surface there is nothing to mirror.
    this.reflectionOn = h !== null && this.camY > h
    if (!this.reflectionOn) return
    const m = this.mirror
    mat4.identity(m)
    m[5] = -1
    m[13] = 2 * h!
    mat4.multiply(this.reflectionView, camera.view, m)
    // A mirror reverses winding, and so does flipping the image: do both and the pipelines' culling still holds.
    // The water samples the result flipped back.
    const p = this.reflectionProjection
    p.set(camera.projection)
    p[0] = -p[0]!; p[4] = -p[4]!; p[8] = -p[8]!; p[12] = -p[12]!
    mat4.multiply(this.reflectionViewProjection, p, this.reflectionView)
    this.reflectionFrustum.setFromMatrix(this.reflectionViewProjection)
    this.ensureReflectionTarget()
  }

  private ensureReflectionTarget() {
    const width = Math.max(1, Math.round(this.width * this.reflectionScale))
    const height = Math.max(1, Math.round(this.height * this.reflectionScale))
    const t = this.reflectionTarget
    if (t && t.width === width && t.height === height) return
    t?.color?.destroy()
    t?.depth.destroy()
    t?.resolve.destroy()
    const size = [width, height]
    const msaa = this.sampleCount > 1
    const resolve = this.device.createTexture({ label: 'reflection', size, format: HDR_FORMAT, usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING })
    this.reflectionTarget = {
      width,
      height,
      color: msaa ? this.device.createTexture({ label: 'reflection msaa', size, format: HDR_FORMAT, sampleCount: this.sampleCount, usage: GPUTextureUsage.RENDER_ATTACHMENT }) : null,
      depth: this.device.createTexture({ label: 'reflection depth', size, format: DEPTH_FORMAT, sampleCount: this.sampleCount, usage: GPUTextureUsage.RENDER_ATTACHMENT }),
      resolve,
    }
    // Hand the texture to whatever material samples `reflection`: its bind group is rebuilt when the version moves.
    this.reflection.version++
    this.reflection.gpu = { texture: resolve, view: resolve.createView(), version: this.reflection.version, device: this.device } satisfies TextureSlot
  }

  private reflectionPass(encoder: GPUCommandEncoder, scene: Scene) {
    const t = this.reflectionTarget!
    const bg = scene.background
    const pass = encoder.beginRenderPass({
      label: 'reflection',
      colorAttachments: [{
        view: (t.color ?? t.resolve).createView(),
        resolveTarget: t.color ? t.resolve.createView() : undefined,
        // Alpha 0 is sky: the water draws its own there.
        clearValue: { r: bg.r, g: bg.g, b: bg.b, a: 0 },
        loadOp: 'clear',
        storeOp: t.color ? 'discard' : 'store',
      }],
      depthStencilAttachment: { view: t.depth.createView(), depthClearValue: 0, depthLoadOp: 'clear', depthStoreOp: 'discard' },
    })
    this.drawBatches(pass, this.reflectionBatches, this.reflectionGroup!, 0, this.reflectionBatches.length)
    pass.end()
  }

  private mainPass(encoder: GPUCommandEncoder, scene: Scene) {
    const bg = scene.background
    const msaa = this.colorTarget !== null
    // With effects to come the multisampled image is kept for them and resolved at the end of their pass.
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
    // Without effects the overlay goes last in this pass; with them, last in theirs. The sky goes in where
    // nothing opaque drew, before anything translucent is laid over it.
    const end = this.effects.length ? this.overlayFrom : this.mainBatches.length
    this.drawBatches(pass, this.mainBatches, this.sceneGroup!, 0, this.translucentFrom)
    if (scene.sky) {
      this.sky.draw(pass, this.sceneGroup!)
      this.stats.drawCalls++
    }
    this.drawBatches(pass, this.mainBatches, this.sceneGroup!, this.translucentFrom, end)
    pass.end()
  }

  /** Draws `batches[from..to)` into `pass`, binding only what changes between them. */
  private drawBatches(pass: GPURenderPassEncoder, batches: Batch[], scene: GPUBindGroup, from: number, to: number) {
    pass.setBindGroup(0, scene)
    pass.setVertexBuffer(0, this.arena.vertexBuffer)
    pass.setIndexBuffer(this.arena.indexBuffer, 'uint32')
    let current = -1
    // What is bound at groups 1 and 2: -1 for the lighting group, else a resource group id.
    let group1 = -2
    let group2 = -2
    for (let i = from; i < to; i++) {
      const batch = batches[i]!
      if (batch.pipeline !== current) {
        pass.setPipeline(this.pipelines[batch.pipeline]!)
        current = batch.pipeline
      }
      const kind = this.pipelineKind[batch.pipeline]
      if (kind !== 'standard') {
        if (group1 !== batch.group) {
          pass.setBindGroup(1, this.resources.group(batch.group))
          group1 = batch.group
        }
        if (kind === 'shadowed' && group2 !== -1) {
          pass.setBindGroup(2, this.lightingGroup!)
          group2 = -1
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
  }

  /**
   * Particles and other effects, over everything else, with the scene's depth bound read-only; then the
   * overlay, which ignores depth, and the image resolves. The overlay has no pass of its own because
   * storing and reloading the multisampled HDR image between them costs far more than its few draws.
   */
  private effectsPass(encoder: GPUCommandEncoder, ctx: EffectContext, composite: boolean) {
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
    if (composite) {
      this.halfResolution.composite(pass, this.sampleCount, DEPTH_FORMAT)
      this.stats.drawCalls++
    }
    for (const effect of this.effects) if (!effect.halfResolution) this.stats.drawCalls += effect.draw(ctx, pass)
    if (this.overlayFrom < this.mainBatches.length) this.drawBatches(pass, this.mainBatches, this.sceneGroup!, this.overlayFrom, this.mainBatches.length)
    pass.end()
  }

  /** The effects that ask for it, at half the screen's size, into a target cleared to nothing. */
  private halfResolutionPass(encoder: GPUCommandEncoder, ctx: EffectContext) {
    this.halfResolution.ensure(this.width, this.height)
    const pass = encoder.beginRenderPass({
      label: 'effects half',
      colorAttachments: [{ view: this.halfResolution.view!, loadOp: 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 0 } }],
      timestampWrites: this.timer.pass('effectsHalf'),
    })
    const half: EffectContext = { ...ctx, sampleCount: 1, half: true }
    for (const effect of this.effects) if (effect.halfResolution) this.stats.drawCalls += effect.draw(half, pass)
    pass.end()
  }

  private outputPass(encoder: GPUCommandEncoder, scene: Scene, camera: PerspectiveCamera, screen: GPUTextureView) {
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
    o[12] = scene.bloom?.strength ?? 0
    this.device.queue.writeBuffer(this.outputUniform, 0, o)
    const pass = encoder.beginRenderPass({
      label: 'output',
      colorAttachments: [{ view: screen, loadOp: 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 1 } }],
      timestampWrites: this.timer.pass('output'),
    })
    pass.setPipeline(this.outputPipeline)
    pass.setBindGroup(0, this.outputGroup!)
    pass.draw(3)
    if (this.hud.size) this.stats.drawCalls += this.hudPass.draw(pass, this.hud)
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
    this.sky.destroy()
    this.device.destroy()
  }
}

function byStateThenGeometry(a: DrawItem, b: DrawItem) {
  return a.order - b.order || a.pipeline - b.pipeline || a.group - b.group || a.slot.firstIndex - b.slot.firstIndex
}

function byOrderThenFarToNear(a: DrawItem, b: DrawItem) {
  return a.order - b.order || b.depth - a.depth
}

/**
 * Each instance's bounding sphere in world space, four numbers an instance (x, y, z, radius), worked out
 * again only when the instances or the mesh's place change.
 */
function instanceSpheres(mesh: InstancedMesh): Float32Array {
  let out = mesh.spheres
  if (out && out.length >= mesh.count * 4 && mesh.spheresVersion === mesh.version && mesh.spheresWorld === mesh.worldVersion) return out
  if (!out || out.length < mesh.count * 4) out = mesh.spheres = new Float32Array(mesh.capacity * 4)
  const g = mesh.geometry.bounds
  const m = mesh.matrices
  const w = mesh.matrixWorld
  const worldScale = mat4.maxScale(w)
  for (let i = 0; i < mesh.count; i++) {
    const o = i * 16
    const x = m[o]! * g.x + m[o + 4]! * g.y + m[o + 8]! * g.z + m[o + 12]!
    const y = m[o + 1]! * g.x + m[o + 5]! * g.y + m[o + 9]! * g.z + m[o + 13]!
    const z = m[o + 2]! * g.x + m[o + 6]! * g.y + m[o + 10]! * g.z + m[o + 14]!
    const sx = m[o]! * m[o]! + m[o + 1]! * m[o + 1]! + m[o + 2]! * m[o + 2]!
    const sy = m[o + 4]! * m[o + 4]! + m[o + 5]! * m[o + 5]! + m[o + 6]! * m[o + 6]!
    const sz = m[o + 8]! * m[o + 8]! + m[o + 9]! * m[o + 9]! + m[o + 10]! * m[o + 10]!
    out[i * 4] = w[0]! * x + w[4]! * y + w[8]! * z + w[12]!
    out[i * 4 + 1] = w[1]! * x + w[5]! * y + w[9]! * z + w[13]!
    out[i * 4 + 2] = w[2]! * x + w[6]! * y + w[10]! * z + w[14]!
    out[i * 4 + 3] = g.radius * Math.sqrt(Math.max(sx, sy, sz)) * worldScale
  }
  mesh.spheresVersion = mesh.version
  mesh.spheresWorld = mesh.worldVersion
  return out
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
