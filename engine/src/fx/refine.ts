import { Effect, type EffectContext } from './effect'
import { Geometry } from '../scene/geometry'
import { VERTEX_STRIDE, type GeometrySlot } from '../gpu/arena'

/** Each source triangle's edges are cut into this many, so it becomes LEVELS² triangles. */
export const REFINE_LEVELS = 3
/** Vertices and triangles one source triangle becomes. */
const VERTS = ((REFINE_LEVELS + 1) * (REFINE_LEVELS + 2)) / 2
const TRIS = REFINE_LEVELS * REFINE_LEVELS
/** Words in an arena vertex (`VERTEX_STRIDE` bytes). */
const WORDS = VERTEX_STRIDE / 4
/** The most source meshes it takes at once. */
const MOST_SOURCES = 16

const REFINE_WGSL = /* wgsl */ `
struct Params {
  // The square refined: its corner (world x, z) and side, metres.
  area: vec4f,
  // Source triangles, the most triangles the output holds, how many sources.
  counts: vec4u,
}

// One source mesh: its first triangle among all the sources', and its first vertex.
struct Source {
  firstTriangle: u32,
  firstVertex: u32,
}

const LEVELS = ${REFINE_LEVELS}u;
const VERTS = ${VERTS}u;
const WORDS = ${WORDS}u;

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> sources: array<Source, ${MOST_SOURCES}>;
@group(0) @binding(2) var<storage, read> positions: array<f32>;
@group(0) @binding(3) var<storage, read> custom: array<vec4f>;
@group(0) @binding(4) var<storage, read> indices: array<u32>;
@group(0) @binding(5) var<storage, read_write> outVertices: array<u32>;
@group(0) @binding(6) var<storage, read_write> outIndices: array<u32>;
@group(0) @binding(7) var<storage, read_write> counter: atomic<u32>;
@group(0) @binding(8) var<storage, read> marks: array<f32>;
@group(0) @binding(9) var<storage, read> normals: array<f32>;

fn position(v: u32) -> vec3f {
  return vec3f(positions[v * 3u], positions[v * 3u + 1u], positions[v * 3u + 2u]);
}

fn normalOf(v: u32) -> vec3f {
  return vec3f(normals[v * 3u], normals[v * 3u + 1u], normals[v * 3u + 2u]);
}

// Where vertex (i, j) of a refined triangle is in its list: row i holds LEVELS + 1 - i of them.
fn corner(i: u32, j: u32) -> u32 {
  return i * (LEVELS + 1u) - i * (i - 1u) / 2u + j;
}

// One thread a source triangle: one whose middle is in the square is cut into LEVELS² triangles, its
// new vertices blended from its corners and written in the arena's layout, with uv's x 1 to mark them
// and its y (\`marks\`) and its normal blended from the corners'.
@compute @workgroup_size(64)
fn refine(@builtin(global_invocation_id) id: vec3u) {
  let t = id.x;
  if (t >= params.counts.x) {
    return;
  }
  var s = 0u;
  for (var k = 1u; k < params.counts.z; k++) {
    if (sources[k].firstTriangle <= t) {
      s = k;
    }
  }
  let first = sources[s].firstVertex;
  let ia = indices[t * 3u] + first;
  let ib = indices[t * 3u + 1u] + first;
  let ic = indices[t * 3u + 2u] + first;
  let a = position(ia);
  let b = position(ib);
  let c = position(ic);
  let rel = (a.xz + b.xz + c.xz) / 3.0 - params.area.xy;
  if (rel.x < 0.0 || rel.y < 0.0 || rel.x >= params.area.z || rel.y >= params.area.z) {
    return;
  }
  let slot = atomicAdd(&counter, 1u);
  if (slot >= params.counts.y) {
    return;
  }
  let base = slot * VERTS;
  for (var i = 0u; i <= LEVELS; i++) {
    for (var j = 0u; j <= LEVELS - i; j++) {
      let wb = f32(i) / f32(LEVELS);
      let wc = f32(j) / f32(LEVELS);
      let wa = 1.0 - wb - wc;
      let p = a * wa + b * wb + c * wc;
      let q = custom[ia] * wa + custom[ib] * wb + custom[ic] * wc;
      let mark = marks[ia] * wa + marks[ib] * wb + marks[ic] * wc;
      let facing = normalOf(ia) * wa + normalOf(ib) * wb + normalOf(ic) * wc;
      let o = (base + corner(i, j)) * WORDS;
      outVertices[o] = bitcast<u32>(p.x);
      outVertices[o + 1u] = bitcast<u32>(p.y);
      outVertices[o + 2u] = bitcast<u32>(p.z);
      outVertices[o + 3u] = bitcast<u32>(facing.x);
      outVertices[o + 4u] = bitcast<u32>(facing.y);
      outVertices[o + 5u] = bitcast<u32>(facing.z);
      outVertices[o + 6u] = bitcast<u32>(1.0);
      outVertices[o + 7u] = bitcast<u32>(mark);
      outVertices[o + 8u] = 0xffffffffu;
      outVertices[o + 9u] = 0xffffffffu;
      outVertices[o + 10u] = bitcast<u32>(q.x);
      outVertices[o + 11u] = bitcast<u32>(q.y);
      outVertices[o + 12u] = bitcast<u32>(q.z);
      outVertices[o + 13u] = bitcast<u32>(q.w);
      outVertices[o + 14u] = 0u;
      outVertices[o + 15u] = 0u;
    }
  }
  // Wound as the source is: (i, j), (i + 1, j), (i, j + 1) runs as a, b, c do.
  var n = slot * ${TRIS * 3}u;
  for (var i = 0u; i < LEVELS; i++) {
    for (var j = 0u; j < LEVELS - i; j++) {
      outIndices[n] = base + corner(i, j);
      outIndices[n + 1u] = base + corner(i + 1u, j);
      outIndices[n + 2u] = base + corner(i, j + 1u);
      n += 3u;
      if (j + 1u < LEVELS - i) {
        outIndices[n] = base + corner(i + 1u, j);
        outIndices[n + 1u] = base + corner(i + 1u, j + 1u);
        outIndices[n + 2u] = base + corner(i, j + 1u);
        n += 3u;
      }
    }
  }
}
`

/**
 * Cuts the triangles of some meshes that lie in a square into finer ones on the GPU, for a surface whose
 * vertices move by something finer than its own mesh (water under a wave field). Each source triangle
 * whose middle is in the square becomes `REFINE_LEVELS`² triangles, their corners blended from its own
 * position and `custom`, and marked with 1 in uv's x so a shader can tell them from the source's. Uv's y
 * is blended from the corners' too, for a source that keeps a value of its own there, and so is the
 * normal (straight up for a source without normals).
 *
 * The result is `geometry`, for a mesh of the caller's with the sources' material: a fixed slot in the
 * geometry arena that a compute pass fills, then copies in, whenever the square or the sources change.
 * Triangles past its capacity are left out, and its unused triangles are empty. The sources must keep
 * their arrays (not `uploadOnce`): they are uploaded from them, once each time the set changes.
 */
export class MeshRefiner extends Effect {
  readonly geometry: Geometry
  /** Whether the output holds the refined square as it was last set. */
  ready = false
  private taken: Geometry[] = []
  private readonly area: [number, number, number] = [0, 0, 0]
  private sourcesChanged = false
  private dirty = false
  private gpu: RefineGpu | null = null
  /**
   * The arena's buffers its output was last copied into. The arena replaces them when it grows, copying
   * the old ones over at once, so a copy this frame made into them before a later geometry grew the arena
   * is lost: the square would stand empty (and what is under the water show through) until it next moved.
   * Seeing new buffers, it copies its output in again.
   */
  private wroteTo: [GPUBuffer, GPUBuffer] | null = null

  /** `capacity` is the most source triangles the square may hold. */
  constructor(readonly capacity: number) {
    super()
    this.geometry = new Geometry({
      positions: new Float32Array(capacity * VERTS * 3),
      indices: new Uint32Array(capacity * TRIS * 3),
      uploadOnce: true,
    })
    this.geometry.gpuFilled = true
  }

  /** The geometries it refines now. */
  get sources(): readonly Geometry[] {
    return this.taken
  }

  /** The meshes refined: their geometries, each with positions, `custom` and 32-bit indices. */
  setSources(sources: readonly Geometry[]) {
    const list = sources.slice(0, MOST_SOURCES)
    if (list.length === this.taken.length && list.every((g, i) => g === this.taken[i])) return
    this.taken = list
    this.sourcesChanged = true
    this.dirty = true
    this.ready = false
  }

  /** The square refined: its corner `(x0, z0)` and side, metres. */
  setArea(x0: number, z0: number, size: number) {
    if (this.area[0] === x0 && this.area[1] === z0 && this.area[2] === size) return
    this.area[0] = x0
    this.area[1] = z0
    this.area[2] = size
    this.dirty = true
    this.ready = false
  }

  prepare(ctx: EffectContext, encoder: GPUCommandEncoder) {
    if (this.gpu && this.gpu.device !== ctx.device) {
      this.gpu = null
      this.sourcesChanged = this.dirty = true
    }
    if (this.wroteTo && (this.wroteTo[0] !== ctx.vertices || this.wroteTo[1] !== ctx.indices)) this.dirty = true
    if (!this.dirty) return
    const slot = this.geometry.gpu as GeometrySlot | undefined
    // Its slot is made when its mesh is first drawn; until then there is nowhere to put it. A slot
    // left from another device's arena is not a place in this one: written there, the output would
    // land on whatever other meshes hold those vertices now.
    if (!slot || slot.owner.vertexBuffer !== ctx.vertices) return
    const gpu = (this.gpu ??= new RefineGpu(ctx.device, this.capacity))
    if (this.sourcesChanged) {
      gpu.upload(this.taken)
      this.sourcesChanged = false
    }
    const p = gpu.params
    const u = new Uint32Array(p.buffer)
    p[0] = this.area[0]
    p[1] = this.area[1]
    p[2] = this.area[2]
    u[4] = gpu.triangles
    u[5] = this.capacity
    u[6] = gpu.sourceCount
    ctx.device.queue.writeBuffer(gpu.uniforms, 0, p)
    encoder.clearBuffer(gpu.counter)
    encoder.clearBuffer(gpu.outIndices)
    if (gpu.triangles > 0) {
      const pass = encoder.beginComputePass({ label: 'refine' })
      pass.setPipeline(gpu.pipeline)
      pass.setBindGroup(0, gpu.group!)
      pass.dispatchWorkgroups(Math.ceil(gpu.triangles / 64))
      pass.end()
    }
    encoder.copyBufferToBuffer(gpu.outVertices, 0, ctx.vertices, slot.baseVertex * VERTEX_STRIDE, this.capacity * VERTS * VERTEX_STRIDE)
    encoder.copyBufferToBuffer(gpu.outIndices, 0, ctx.indices, slot.firstIndex * 4, this.capacity * TRIS * 3 * 4)
    this.wroteTo = [ctx.vertices, ctx.indices]
    this.dirty = false
    this.ready = true
  }

  override get idle() {
    return true
  }

  draw(): number {
    return 0
  }

  dispose() {
    this.gpu?.destroy()
    this.gpu = null
    this.geometry.dispose()
  }
}

/** The GPU half of a MeshRefiner: the sources' data, the pass and its output. */
class RefineGpu {
  readonly params = new Float32Array(8)
  readonly uniforms: GPUBuffer
  readonly counter: GPUBuffer
  readonly outVertices: GPUBuffer
  readonly outIndices: GPUBuffer
  readonly sourceTable: GPUBuffer
  readonly pipeline: GPUComputePipeline
  private readonly layout: GPUBindGroupLayout
  private positions: GPUBuffer | null = null
  private custom: GPUBuffer | null = null
  private marks: GPUBuffer | null = null
  private normals: GPUBuffer | null = null
  private indices: GPUBuffer | null = null
  group: GPUBindGroup | null = null
  triangles = 0
  sourceCount = 0

  constructor(readonly device: GPUDevice, capacity: number) {
    const storage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST
    this.uniforms = device.createBuffer({ label: 'refine', size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.counter = device.createBuffer({ label: 'refine counter', size: 4, usage: storage })
    this.outVertices = device.createBuffer({ label: 'refine vertices', size: capacity * VERTS * VERTEX_STRIDE, usage: storage | GPUBufferUsage.COPY_SRC })
    this.outIndices = device.createBuffer({ label: 'refine indices', size: capacity * TRIS * 3 * 4, usage: storage | GPUBufferUsage.COPY_SRC })
    this.sourceTable = device.createBuffer({ label: 'refine sources', size: MOST_SOURCES * 8, usage: storage })
    const compute = GPUShaderStage.COMPUTE
    const read = { type: 'read-only-storage' } as const
    const write = { type: 'storage' } as const
    this.layout = device.createBindGroupLayout({
      label: 'refine',
      entries: [
        { binding: 0, visibility: compute, buffer: { type: 'uniform' } },
        { binding: 1, visibility: compute, buffer: read },
        { binding: 2, visibility: compute, buffer: read },
        { binding: 3, visibility: compute, buffer: read },
        { binding: 4, visibility: compute, buffer: read },
        { binding: 5, visibility: compute, buffer: write },
        { binding: 6, visibility: compute, buffer: write },
        { binding: 7, visibility: compute, buffer: write },
        { binding: 8, visibility: compute, buffer: read },
        { binding: 9, visibility: compute, buffer: read },
      ],
    })
    const module = device.createShaderModule({ label: 'refine', code: REFINE_WGSL })
    this.pipeline = device.createComputePipeline({ label: 'refine', layout: device.createPipelineLayout({ bindGroupLayouts: [this.layout] }), compute: { module, entryPoint: 'refine' } })
  }

  /** Uploads the sources' positions, custom, uv y's, normals and indices end to end, and the table of where each starts. */
  upload(sources: readonly Geometry[]) {
    let vertices = 0
    let triangles = 0
    const table = new Uint32Array(MOST_SOURCES * 2)
    sources.forEach((g, i) => {
      table[i * 2] = triangles
      table[i * 2 + 1] = vertices
      vertices += g.positions.length / 3
      triangles += (g.indices?.length ?? 0) / 3
    })
    this.triangles = triangles
    this.sourceCount = sources.length
    this.device.queue.writeBuffer(this.sourceTable, 0, table)
    const storage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST
    const fit = (buffer: GPUBuffer | null, bytes: number, label: string) => {
      const size = Math.max(16, Math.ceil(bytes / 16) * 16)
      if (buffer && buffer.size >= size) return buffer
      buffer?.destroy()
      this.group = null
      return this.device.createBuffer({ label, size: Math.max(size, (buffer?.size ?? 0) * 1.5), usage: storage })
    }
    this.positions = fit(this.positions, vertices * 12, 'refine positions')
    this.custom = fit(this.custom, vertices * 16, 'refine custom')
    this.marks = fit(this.marks, vertices * 4, 'refine marks')
    this.normals = fit(this.normals, vertices * 12, 'refine normals')
    this.indices = fit(this.indices, triangles * 12, 'refine indices in')
    let v = 0
    let t = 0
    for (const g of sources) {
      const count = g.positions.length / 3
      this.device.queue.writeBuffer(this.positions, v * 12, g.positions as Float32Array<ArrayBuffer>)
      if (g.custom) this.device.queue.writeBuffer(this.custom, v * 16, g.custom as Float32Array<ArrayBuffer>)
      // A source's uv y, or 0 for one without uvs (the buffer may hold an earlier source's).
      const marks = new Float32Array(count)
      if (g.uvs) for (let i = 0; i < count; i++) marks[i] = g.uvs[i * 2 + 1]!
      this.device.queue.writeBuffer(this.marks, v * 4, marks)
      // A source's normals, or straight up for one without any.
      let facing = g.normals
      if (!facing) {
        facing = new Float32Array(count * 3)
        for (let i = 0; i < count; i++) facing[i * 3 + 1] = 1
      }
      this.device.queue.writeBuffer(this.normals, v * 12, facing as Float32Array<ArrayBuffer>)
      const indices = g.indices instanceof Uint32Array ? g.indices : Uint32Array.from(g.indices ?? [])
      if (indices.length) this.device.queue.writeBuffer(this.indices, t * 12, indices as Uint32Array<ArrayBuffer>)
      v += count
      t += indices.length / 3
    }
    this.group ??= this.device.createBindGroup({
      label: 'refine',
      layout: this.layout,
      entries: [
        { binding: 0, resource: { buffer: this.uniforms } },
        { binding: 1, resource: { buffer: this.sourceTable } },
        { binding: 2, resource: { buffer: this.positions } },
        { binding: 3, resource: { buffer: this.custom } },
        { binding: 4, resource: { buffer: this.indices } },
        { binding: 5, resource: { buffer: this.outVertices } },
        { binding: 6, resource: { buffer: this.outIndices } },
        { binding: 7, resource: { buffer: this.counter } },
        { binding: 8, resource: { buffer: this.marks } },
        { binding: 9, resource: { buffer: this.normals } },
      ],
    })
  }

  destroy() {
    for (const b of [this.uniforms, this.counter, this.outVertices, this.outIndices, this.sourceTable, this.positions, this.custom, this.marks, this.normals, this.indices]) b?.destroy()
  }
}
