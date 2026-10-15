import type { Geometry } from '../scene/geometry'
import { RangeAllocator } from './allocator'

/**
 * One vertex layout for every mesh: position f32×3, normal f32×3, uv f32×2, colour unorm16×4 (linear,
 * so 8 bits would band in the darks). Missing attributes are filled with defaults (up, 0, white).
 */
export const VERTEX_STRIDE = 40
export const VERTEX_LAYOUT: GPUVertexBufferLayout = {
  arrayStride: VERTEX_STRIDE,
  attributes: [
    { shaderLocation: 0, offset: 0, format: 'float32x3' },
    { shaderLocation: 1, offset: 12, format: 'float32x3' },
    { shaderLocation: 2, offset: 24, format: 'float32x2' },
    { shaderLocation: 3, offset: 32, format: 'unorm16x4' },
  ],
}

/** Where a geometry's data sits in the arena. */
export interface GeometrySlot {
  baseVertex: number
  vertexCount: number
  firstIndex: number
  indexCount: number
  version: number
}

const INITIAL_VERTICES = 1 << 18
const INITIAL_INDICES = 1 << 20

/**
 * All mesh geometry lives in one vertex buffer and one index buffer, so the renderer binds them once per
 * pass and a draw is just offsets into them. Buffers grow by doubling, copying the old contents on the GPU.
 */
export class GeometryArena {
  vertexBuffer: GPUBuffer
  indexBuffer: GPUBuffer
  /** Bumped when the buffers are replaced by growth, so recorded bundles know to re-record. */
  generation = 0
  private readonly vertices = new RangeAllocator(INITIAL_VERTICES)
  private readonly indices = new RangeAllocator(INITIAL_INDICES)

  constructor(private readonly device: GPUDevice) {
    this.vertexBuffer = this.makeVertexBuffer(INITIAL_VERTICES)
    this.indexBuffer = this.makeIndexBuffer(INITIAL_INDICES)
  }

  /** Uploads `geometry` if it is new or changed; returns its slot, or null once disposed. */
  sync(geometry: Geometry): GeometrySlot | null {
    let slot = geometry.gpu as GeometrySlot | undefined
    if (slot && slot.version === geometry.version) return slot
    if (geometry.disposed) {
      if (slot) this.release(slot)
      geometry.gpu = undefined
      return null
    }
    const vertexCount = geometry.vertexCount
    const indexCount = geometry.indexCount
    if (slot && (slot.vertexCount !== vertexCount || slot.indexCount !== indexCount)) {
      this.release(slot)
      slot = undefined
    }
    if (!slot) {
      slot = {
        baseVertex: this.allocVertices(vertexCount),
        vertexCount,
        firstIndex: this.allocIndices(indexCount),
        indexCount,
        version: geometry.version,
      }
      geometry.gpu = slot
    }
    slot.version = geometry.version
    this.write(geometry, slot)
    return slot
  }

  release(slot: GeometrySlot) {
    this.vertices.release(slot.baseVertex, slot.vertexCount)
    this.indices.release(slot.firstIndex, slot.indexCount)
  }

  get usedBytes() {
    return this.vertices.used * VERTEX_STRIDE + this.indices.used * 4
  }

  private write(geometry: Geometry, slot: GeometrySlot) {
    const n = slot.vertexCount
    const data = new ArrayBuffer(n * VERTEX_STRIDE)
    const f32 = new Float32Array(data)
    const u16 = new Uint16Array(data)
    const { positions, normals, uvs, colors } = geometry
    const colorSize = colors ? colors.length / n : 0
    for (let i = 0; i < n; i++) {
      const f = i * 10
      f32[f] = positions[i * 3]!
      f32[f + 1] = positions[i * 3 + 1]!
      f32[f + 2] = positions[i * 3 + 2]!
      if (normals) {
        f32[f + 3] = normals[i * 3]!
        f32[f + 4] = normals[i * 3 + 1]!
        f32[f + 5] = normals[i * 3 + 2]!
      } else {
        f32[f + 4] = 1
      }
      if (uvs) {
        f32[f + 6] = uvs[i * 2]!
        f32[f + 7] = uvs[i * 2 + 1]!
      }
      const c = i * 20 + 16
      if (colors) {
        u16[c] = unorm16(colors[i * colorSize]!)
        u16[c + 1] = unorm16(colors[i * colorSize + 1]!)
        u16[c + 2] = unorm16(colors[i * colorSize + 2]!)
        u16[c + 3] = colorSize === 4 ? unorm16(colors[i * 4 + 3]!) : 65535
      } else {
        u16[c] = u16[c + 1] = u16[c + 2] = u16[c + 3] = 65535
      }
    }
    this.device.queue.writeBuffer(this.vertexBuffer, slot.baseVertex * VERTEX_STRIDE, data)
    // Indices are stored relative to the geometry; draws add baseVertex.
    const indices = new Uint32Array(slot.indexCount)
    if (geometry.indices) indices.set(geometry.indices)
    else for (let i = 0; i < indices.length; i++) indices[i] = i
    this.device.queue.writeBuffer(this.indexBuffer, slot.firstIndex * 4, indices)
  }

  private allocVertices(count: number): number {
    let offset = this.vertices.alloc(count)
    while (offset < 0) {
      const capacity = Math.max(this.vertices.capacity * 2, this.vertices.capacity + count)
      this.vertexBuffer = this.grow(this.vertexBuffer, this.makeVertexBuffer(capacity), this.vertices.capacity * VERTEX_STRIDE)
      this.vertices.grow(capacity)
      offset = this.vertices.alloc(count)
    }
    return offset
  }

  private allocIndices(count: number): number {
    let offset = this.indices.alloc(count)
    while (offset < 0) {
      const capacity = Math.max(this.indices.capacity * 2, this.indices.capacity + count)
      this.indexBuffer = this.grow(this.indexBuffer, this.makeIndexBuffer(capacity), this.indices.capacity * 4)
      this.indices.grow(capacity)
      offset = this.indices.alloc(count)
    }
    return offset
  }

  private grow(old: GPUBuffer, next: GPUBuffer, bytes: number): GPUBuffer {
    const encoder = this.device.createCommandEncoder()
    encoder.copyBufferToBuffer(old, 0, next, 0, bytes)
    this.device.queue.submit([encoder.finish()])
    old.destroy()
    this.generation++
    return next
  }

  private makeVertexBuffer(vertices: number) {
    return this.device.createBuffer({
      label: 'arena vertices',
      size: vertices * VERTEX_STRIDE,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
    })
  }

  private makeIndexBuffer(indices: number) {
    return this.device.createBuffer({
      label: 'arena indices',
      size: indices * 4,
      usage: GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
    })
  }
}

function unorm16(v: number) {
  return Math.round(Math.min(1, Math.max(0, v)) * 65535)
}
