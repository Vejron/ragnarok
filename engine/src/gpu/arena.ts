import type { Geometry } from '../scene/geometry'
import { RangeAllocator } from './allocator'

/**
 * One vertex layout for every mesh: position f32×3, normal f32×3, uv f32×2, colour unorm16×4 (linear,
 * so 8 bits would band in the darks), a free f32×4 for custom shaders, and skinning joints uint8×4 and
 * weights unorm8×4. Missing attributes are filled with defaults (up, 0, white, 0, unskinned).
 */
export const VERTEX_STRIDE = 64
export const VERTEX_LAYOUT: GPUVertexBufferLayout = {
  arrayStride: VERTEX_STRIDE,
  attributes: [
    { shaderLocation: 0, offset: 0, format: 'float32x3' },
    { shaderLocation: 1, offset: 12, format: 'float32x3' },
    { shaderLocation: 2, offset: 24, format: 'float32x2' },
    { shaderLocation: 3, offset: 32, format: 'unorm16x4' },
    { shaderLocation: 4, offset: 40, format: 'float32x4' },
    { shaderLocation: 5, offset: 56, format: 'uint8x4' },
    { shaderLocation: 6, offset: 60, format: 'unorm8x4' },
  ],
}

/** Where a geometry's data sits in the arena. */
export interface GeometrySlot {
  baseVertex: number
  vertexCount: number
  firstIndex: number
  indexCount: number
  version: number
  /** The arena it was allocated in: a slot from another (a lost device's) is ignored and the geometry re-uploaded. */
  owner: GeometryArena
}

const INITIAL_VERTICES = 1 << 18
const INITIAL_INDICES = 1 << 20
/**
 * The most a buffer grows by at once, bytes, unless one geometry needs more. A buffer that doubled
 * would stand half empty after its last growth: 512 MB held for a town's 295 MB of vertices.
 */
const GROWTH_BYTES = 64 << 20
/**
 * The most bytes of an upload that are put together in a JavaScript array and sent with `writeBuffer`:
 * a few changed quads, a small mesh. Below this the browser's own copy is cheaper than a mapped
 * buffer's round trip (16 KB took 0.03 ms that way and 0.06 ms mapped, in headless Chrome on Metal).
 */
const SMALL_UPLOAD_BYTES = 64 << 10
/**
 * The size of each staging buffer in the ring that bigger uploads are interleaved straight into,
 * bytes. Mapped, they skip the copy out of a JavaScript array (a 1 MB upload took 0.65 ms instead of
 * 1.2 ms). An upload bigger than this, a tile's buildings say, gets a mapped buffer of its own, so that
 * the ring does not hold the biggest mesh's size for good.
 */
const STAGING_BYTES = 4 << 20
/** How many staging buffers are kept mapped and waiting between frames; more are destroyed when they come back. */
const STAGING_KEPT = 3

/**
 * All mesh geometry lives in one vertex buffer and one index buffer, so the renderer binds them once per
 * pass and a draw is just offsets into them. Buffers grow by half, and by `GROWTH_BYTES` at most once
 * they are big, copying the old contents on the GPU; they never shrink.
 */
export class GeometryArena {
  vertexBuffer: GPUBuffer
  indexBuffer: GPUBuffer
  /** Bumped when the buffers are replaced by growth, so recorded bundles know to re-record. */
  generation = 0
  /** Bytes of vertices and indices uploaded since the arena was made: the renderer reports each frame's share. */
  uploaded = 0
  private readonly vertices = new RangeAllocator(INITIAL_VERTICES)
  private readonly indices = new RangeAllocator(INITIAL_INDICES)
  /** Geometries holding a slot here, so disposed ones are freed even when nothing draws them again. */
  private readonly live = new Set<Geometry>()
  /** Where small uploads are put together before `writeBuffer` copies them out. */
  private readonly scratch = new ArrayBuffer(SMALL_UPLOAD_BYTES)
  /** Staging buffers mapped and ready for the next uploads. */
  private readonly ready: GPUBuffer[] = []
  /** The staging buffer being filled, its mapped range and the bytes of it used so far. */
  private current: GPUBuffer | null = null
  private currentRange: ArrayBuffer | null = null
  private cursor = 0
  /** Staging buffers filled and unmapped, waiting for `flush` to submit their copies. */
  private readonly filled: GPUBuffer[] = []
  /** Mapped buffers made for one upload too big for the ring, destroyed once their copy is submitted. */
  private readonly oversize: GPUBuffer[] = []
  /** The copies out of the staging buffers since the last `flush`. */
  private encoder: GPUCommandEncoder | null = null
  /** The buffer `stage` last put an upload in (null for `scratch`), and that upload's mapped range. */
  private stagedIn: GPUBuffer | null = null
  private stagedRange: ArrayBuffer = this.scratch

  constructor(private readonly device: GPUDevice) {
    this.vertexBuffer = this.makeVertexBuffer(INITIAL_VERTICES)
    this.indexBuffer = this.makeIndexBuffer(INITIAL_INDICES)
  }

  /** Uploads `geometry` if it is new or changed; returns its slot, or null once disposed. */
  sync(geometry: Geometry): GeometrySlot | null {
    let slot = geometry.gpu as GeometrySlot | undefined
    if (slot && slot.owner !== this) slot = undefined
    if (slot && slot.version === geometry.version) {
      const ranges = geometry.dirtyRanges
      for (let i = 0; i < ranges.length; i += 2) this.writeVertices(geometry, slot, ranges[i]!, ranges[i + 1]!)
      ranges.length = 0
      const collapsed = geometry.collapsedRanges
      for (let i = 0; i < collapsed.length; i += 2) this.collapse(slot, collapsed[i]!, collapsed[i + 1]!)
      collapsed.length = 0
      return slot
    }
    geometry.dirtyRanges.length = 0
    geometry.collapsedRanges.length = 0
    if (geometry.disposed) {
      if (slot) this.release(slot)
      geometry.gpu = undefined
      this.live.delete(geometry)
      return null
    }
    // Its arrays are gone and the copy was another device's: nothing to draw until it is made again.
    // One filled on the GPU is given room all the same: its arrays never held what is drawn.
    if (geometry.released && !geometry.gpuFilled) return null
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
        owner: this,
      }
      geometry.gpu = slot
      this.live.add(geometry)
    }
    slot.version = geometry.version
    if (!geometry.released) this.write(geometry, slot)
    // Packed alongside arrays it keeps: the packed copy has done its work.
    geometry.dropStalePacked()
    if (geometry.uploadOnce) geometry.release()
    return slot
  }

  /**
   * Frees the slots of disposed geometries. Call once per frame, after drawing: a geometry is usually
   * disposed once its mesh has left the scene, so `sync` never sees it again.
   */
  collect() {
    for (const geometry of this.live) {
      if (!geometry.disposed) continue
      const slot = geometry.gpu as GeometrySlot | undefined
      if (slot?.owner === this) this.release(slot)
      geometry.gpu = undefined
      this.live.delete(geometry)
    }
  }

  release(slot: GeometrySlot) {
    this.vertices.release(slot.baseVertex, slot.vertexCount)
    this.indices.release(slot.firstIndex, slot.indexCount)
  }

  /**
   * Submits the copies out of the staging buffers and starts mapping them again for the next uploads.
   * Call before submitting the frame that draws what was synced.
   */
  flush() {
    if (!this.encoder) return
    if (this.current) {
      this.current.unmap()
      this.filled.push(this.current)
      this.current = this.currentRange = null
    }
    for (const buffer of this.oversize) buffer.unmap()
    this.device.queue.submit([this.encoder.finish()])
    this.encoder = null
    for (const buffer of this.oversize) buffer.destroy()
    this.oversize.length = 0
    for (const buffer of this.filled) {
      buffer.mapAsync(GPUMapMode.WRITE).then(() => {
        if (this.ready.length < STAGING_KEPT) this.ready.push(buffer)
        else buffer.destroy()
      }, () => buffer.destroy())
    }
    this.filled.length = 0
  }

  get usedBytes() {
    return this.vertices.used * VERTEX_STRIDE + this.indices.used * 4
  }

  private write(geometry: Geometry, slot: GeometrySlot) {
    this.writeVertices(geometry, slot, 0, slot.vertexCount)
    // Indices are stored relative to the geometry; draws add baseVertex.
    const given = geometry.indices
    const bytes = slot.indexCount * 4
    if (given instanceof Uint32Array && bytes <= SMALL_UPLOAD_BYTES && !this.encoder) {
      this.device.queue.writeBuffer(this.indexBuffer, slot.firstIndex * 4, given as Uint32Array<ArrayBuffer>)
      return
    }
    const at = this.stage(bytes)
    const indices = new Uint32Array(this.stagedRange, at, slot.indexCount)
    if (given) indices.set(given)
    else for (let i = 0; i < indices.length; i++) indices[i] = i
    this.send(this.indexBuffer, slot.firstIndex * 4, at, bytes)
  }

  /**
   * Finds room for an upload of `bytes`, zeroed unless `zero` is false (it is all to be written over),
   * and returns where in `stagedRange` it starts: the
   * scratch array for a small one, the staging ring for a bigger one, and a mapped buffer of its own
   * for one too big for the ring. `send` then takes it to the arena.
   *
   * A `writeBuffer` runs before any command buffer submitted after it, so once a copy is waiting for
   * `flush`, small uploads go through the ring as well: one sent later would otherwise land first, and
   * a waiting copy to the same place (a slot freed and given again) would write over it.
   */
  private stage(bytes: number, zero = true): number {
    this.uploaded += bytes
    if (bytes <= SMALL_UPLOAD_BYTES && !this.encoder) {
      if (zero) new Uint8Array(this.scratch, 0, bytes).fill(0)
      this.stagedIn = null
      this.stagedRange = this.scratch
      return 0
    }
    if (bytes > STAGING_BYTES) {
      // Made mapped, so it starts zeroed.
      const buffer = this.makeStaging(bytes)
      this.oversize.push(buffer)
      this.stagedIn = buffer
      this.stagedRange = buffer.getMappedRange()
      return 0
    }
    if (this.current && this.cursor + bytes > STAGING_BYTES) {
      this.current.unmap()
      this.filled.push(this.current)
      this.current = this.currentRange = null
    }
    if (!this.current) {
      // A buffer back from mapAsync holds what was last written into it, so the range used is zeroed below.
      this.current = this.ready.pop() ?? this.makeStaging(STAGING_BYTES)
      this.currentRange = this.current.getMappedRange()
      this.cursor = 0
    }
    const at = this.cursor
    this.cursor += bytes
    if (zero) new Uint8Array(this.currentRange!, at, bytes).fill(0)
    this.stagedIn = this.current
    this.stagedRange = this.currentRange!
    return at
  }

  /** Takes the `bytes` that `stage` found room for at `at` to `offset` in `target`. */
  private send(target: GPUBuffer, offset: number, at: number, bytes: number) {
    if (!this.stagedIn) {
      this.device.queue.writeBuffer(target, offset, this.scratch, at, bytes)
      return
    }
    this.encoder ??= this.device.createCommandEncoder({ label: 'arena uploads' })
    this.encoder.copyBufferToBuffer(this.stagedIn, at, target, offset, bytes)
  }

  private makeStaging(bytes: number) {
    return this.device.createBuffer({ label: 'arena staging', size: bytes, usage: GPUBufferUsage.MAP_WRITE | GPUBufferUsage.COPY_SRC, mappedAtCreation: true })
  }

  /** Puts vertices `first` to `first + count - 1` on one point (`Geometry.collapseRange`). */
  private collapse(slot: GeometrySlot, first: number, count: number) {
    first = Math.max(0, first)
    count = Math.min(count, slot.vertexCount - first)
    if (count <= 0) return
    const bytes = count * VERTEX_STRIDE
    this.send(this.vertexBuffer, (slot.baseVertex + first) * VERTEX_STRIDE, this.stage(bytes), bytes)
  }

  /** Interleaves (or, packed already, copies) and uploads vertices `first` to `first + count - 1`. */
  private writeVertices(geometry: Geometry, slot: GeometrySlot, first: number, count: number) {
    first = Math.max(0, first)
    count = Math.min(count, slot.vertexCount - first)
    if (count <= 0) return
    const bytes = count * VERTEX_STRIDE
    const packed = geometry.packed
    const at = this.stage(bytes, !packed)
    if (packed) new Uint8Array(this.stagedRange, at, bytes).set(new Uint8Array(packed, first * VERTEX_STRIDE, bytes))
    else interleave(geometry, slot.vertexCount, first, count, this.stagedRange, at)
    this.send(this.vertexBuffer, (slot.baseVertex + first) * VERTEX_STRIDE, at, bytes)
  }

  private allocVertices(count: number): number {
    let offset = this.vertices.alloc(count)
    while (offset < 0) {
      const capacity = grown(this.vertices.capacity, count, GROWTH_BYTES / VERTEX_STRIDE)
      this.vertexBuffer = this.grow(this.vertexBuffer, this.makeVertexBuffer(capacity), this.vertices.capacity * VERTEX_STRIDE)
      this.vertices.grow(capacity)
      offset = this.vertices.alloc(count)
    }
    return offset
  }

  private allocIndices(count: number): number {
    let offset = this.indices.alloc(count)
    while (offset < 0) {
      const capacity = grown(this.indices.capacity, count, GROWTH_BYTES / 4)
      this.indexBuffer = this.grow(this.indexBuffer, this.makeIndexBuffer(capacity), this.indices.capacity * 4)
      this.indices.grow(capacity)
      offset = this.indices.alloc(count)
    }
    return offset
  }

  private grow(old: GPUBuffer, next: GPUBuffer, bytes: number): GPUBuffer {
    // Past the device's limit the new buffer would be invalid and every draw after it fail: a black screen.
    if (next.size > this.device.limits.maxBufferSize) {
      next.destroy()
      throw new Error(`Geometry arena needs ${next.size} bytes, over the GPU's ${this.device.limits.maxBufferSize}`)
    }
    // The staged copies into the old buffer go first, or the growth would copy it over without them.
    this.flush()
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

/** The capacity a buffer of `capacity` grows to, to take `count` more: half as much again, `most` more at most, and `count` more at least. */
function grown(capacity: number, count: number, most: number): number {
  return capacity + Math.max(count, Math.min(capacity >> 1, most))
}

/** A geometry's vertex arrays, as `interleave` reads them (`GeometryData`'s). */
export interface VertexArrays {
  positions: Float32Array
  normals?: Float32Array | undefined
  uvs?: Float32Array | undefined
  colors?: Float32Array | undefined
  custom?: Float32Array | undefined
  joints?: Uint8Array | Uint16Array | undefined
  weights?: Float32Array | undefined
}

/**
 * `arrays`' vertices in the arena's layout (`VERTEX_LAYOUT`), `VERTEX_STRIDE` bytes each: what the
 * arena would make of them when it uploads them, made beforehand (in a worker, say) so that it only
 * copies them (`GeometryData.packed`).
 */
export function packVertices(arrays: VertexArrays): ArrayBuffer {
  const count = arrays.positions.length / 3
  const out = new ArrayBuffer(count * VERTEX_STRIDE)
  interleave(arrays, count, 0, count, out, 0)
  return out
}

/**
 * Writes vertices `first` to `first + count - 1` of `arrays` (which hold `vertexCount`) into `target`
 * from byte `at`, in the arena's layout. `target` must be zeroed there: what an array leaves out stays
 * its default.
 */
function interleave(arrays: VertexArrays, vertexCount: number, first: number, count: number, target: ArrayBuffer, at: number) {
  const bytes = count * VERTEX_STRIDE
  const f32 = new Float32Array(target, at, count * 16)
  const u16 = new Uint16Array(target, at, count * 32)
  const u8 = new Uint8Array(target, at, bytes)
  const { positions, normals, uvs, colors, custom, joints, weights } = arrays
  const colorSize = colors ? colors.length / vertexCount : 0
  for (let k = 0; k < count; k++) {
    const i = first + k
    const f = k * 16
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
    if (custom) {
      f32[f + 10] = custom[i * 4]!
      f32[f + 11] = custom[i * 4 + 1]!
      f32[f + 12] = custom[i * 4 + 2]!
      f32[f + 13] = custom[i * 4 + 3]!
    }
    if (joints && weights) {
      const b = k * 64 + 56
      for (let j = 0; j < 4; j++) {
        u8[b + j] = joints[i * 4 + j]!
        u8[b + 4 + j] = Math.round(Math.min(1, Math.max(0, weights[i * 4 + j]!)) * 255)
      }
    }
    const c = k * 32 + 16
    if (colors) {
      u16[c] = unorm16(colors[i * colorSize]!)
      u16[c + 1] = unorm16(colors[i * colorSize + 1]!)
      u16[c + 2] = unorm16(colors[i * colorSize + 2]!)
      u16[c + 3] = colorSize === 4 ? unorm16(colors[i * 4 + 3]!) : 65535
    } else {
      u16[c] = u16[c + 1] = u16[c + 2] = u16[c + 3] = 65535
    }
  }
}
