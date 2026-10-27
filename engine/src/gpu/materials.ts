import { RangeAllocator } from './allocator'
import type { Material, Ramp } from '../scene/material'

/** Floats per material record: colour vec4, emissive vec4, params vec4u. Matches `Material` in common.wgsl. */
export const MATERIAL_FLOATS = 12
const MAT_LIT = 1
const MAT_VERTEX_COLORS = 2
const MAT_FOG = 4
const MAT_NORMAL_MAP = 8
const MAT_NORMAL_TONE = 16
const MAT_CRATERS = 32

export const RAMP_WIDTH = 256
const RAMP_ROWS = 64

interface MaterialSlot {
  index: number
  version: number
  /** The table it was allocated in: a slot from another (a lost device's) is ignored and the record rewritten. */
  owner: MaterialTable
}

/**
 * Every material's parameters, in one storage buffer indexed by the object record. Changing a material
 * rewrites its 48 bytes; nothing is rebound. Toon ramps are rows of one small texture.
 */
export class MaterialTable {
  buffer: GPUBuffer
  readonly ramps: GPUTexture
  /** Bumped when `buffer` is replaced, so bind groups holding it are rebuilt. */
  generation = 0
  private data: Float32Array<ArrayBuffer>
  private words: Uint32Array
  private readonly slots = new RangeAllocator(256)
  private dirtyMin = Infinity
  private dirtyMax = -1
  private readonly rampRows = new Map<number, number>()
  private readonly live = new Set<Material>()

  constructor(private readonly device: GPUDevice) {
    this.data = new Float32Array(256 * MATERIAL_FLOATS)
    this.words = new Uint32Array(this.data.buffer)
    this.buffer = this.makeBuffer(256)
    this.ramps = device.createTexture({
      label: 'toon ramps',
      size: [RAMP_WIDTH, RAMP_ROWS],
      format: 'r8unorm',
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    })
    // Row 0 is for materials without a ramp: three's default toon step, 0.7 below the middle and 1 above.
    const fallback = new Uint8Array(RAMP_WIDTH)
    for (let i = 0; i < RAMP_WIDTH; i++) fallback[i] = i / RAMP_WIDTH < 0.7 ? 179 : 255
    this.writeRow(0, fallback)
  }

  /** The material's record index, uploading it first if it is new or changed. */
  sync(material: Material): number {
    let slot = material.gpu as MaterialSlot | undefined
    if (slot && slot.owner !== this) slot = undefined
    if (slot && slot.version === material.version) return slot.index
    if (!slot) {
      let index = this.slots.alloc(1)
      if (index < 0) {
        this.grow(this.slots.capacity * 2)
        index = this.slots.alloc(1)
      }
      slot = { index, version: -1, owner: this }
      material.gpu = slot
      this.live.add(material)
    }
    if (material.disposed) {
      // Disposed materials keep drawing as they were until their meshes go; the slot is freed by `collect`.
      slot.version = material.version
      return slot.index
    }
    slot.version = material.version
    this.writeRecord(slot.index, material)
    return slot.index
  }

  /** Frees the slots of disposed materials. Call once per frame, after drawing. */
  collect() {
    for (const material of this.live) {
      if (!material.disposed) continue
      const slot = material.gpu as MaterialSlot
      this.slots.release(slot.index, 1)
      material.gpu = undefined
      this.live.delete(material)
    }
  }

  /** Uploads the records changed since the last flush. */
  flush() {
    if (this.dirtyMax < 0) return
    const start = this.dirtyMin * MATERIAL_FLOATS
    const end = (this.dirtyMax + 1) * MATERIAL_FLOATS
    this.device.queue.writeBuffer(this.buffer, start * 4, this.data, start, end - start)
    this.dirtyMin = Infinity
    this.dirtyMax = -1
  }

  get count() {
    return this.live.size
  }

  private writeRecord(index: number, m: Material) {
    const f = index * MATERIAL_FLOATS
    this.data[f] = m.color.r
    this.data[f + 1] = m.color.g
    this.data[f + 2] = m.color.b
    this.data[f + 3] = m.opacity
    this.data[f + 4] = m.emissive.r
    this.data[f + 5] = m.emissive.g
    this.data[f + 6] = m.emissive.b
    this.data[f + 7] = 0
    this.words[f + 8] = (m.lit ? MAT_LIT : 0) | (m.vertexColors ? MAT_VERTEX_COLORS : 0) | (m.fog ? MAT_FOG : 0) | (m.normalMap ? MAT_NORMAL_MAP : 0) | (m.normalMap && m.normalMapTone ? MAT_NORMAL_TONE : 0) | (m.craters ? MAT_CRATERS : 0)
    this.words[f + 9] = m.ramp ? this.rampRow(m.ramp) : 0
    this.words[f + 10] = 0
    this.words[f + 11] = 0
    this.dirtyMin = Math.min(this.dirtyMin, index)
    this.dirtyMax = Math.max(this.dirtyMax, index)
  }

  private rampRow(ramp: Ramp): number {
    let row = this.rampRows.get(ramp.id)
    if (row !== undefined) return row
    row = this.rampRows.size + 1
    if (row >= RAMP_ROWS) return 0
    this.rampRows.set(ramp.id, row)
    // Expanded to 256 texels with nearest lookup, so band i covers coordinates [i/n, (i+1)/n), as a
    // nearest-filtered n-texel gradient map does in three.
    const bands = ramp.bands
    const texels = new Uint8Array(RAMP_WIDTH)
    for (let i = 0; i < RAMP_WIDTH; i++) texels[i] = bands[Math.min(bands.length - 1, Math.floor((i / RAMP_WIDTH) * bands.length))]!
    this.writeRow(row, texels)
    return row
  }

  private writeRow(row: number, texels: Uint8Array<ArrayBuffer>) {
    this.device.queue.writeTexture({ texture: this.ramps, origin: [0, row] }, texels, { bytesPerRow: RAMP_WIDTH }, [RAMP_WIDTH, 1])
  }

  private grow(capacity: number) {
    const data = new Float32Array(capacity * MATERIAL_FLOATS)
    data.set(this.data)
    this.data = data
    this.words = new Uint32Array(data.buffer)
    this.slots.grow(capacity)
    this.buffer.destroy()
    this.buffer = this.makeBuffer(capacity)
    this.generation++
    this.dirtyMin = 0
    this.dirtyMax = capacity - 1
  }

  private makeBuffer(capacity: number) {
    return this.device.createBuffer({
      label: 'materials',
      size: capacity * MATERIAL_FLOATS * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    })
  }
}
