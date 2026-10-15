import { InstancedMesh, type Mesh } from '../scene/mesh'
import { RangeAllocator } from './allocator'

/** Floats per object record: model mat4, tint vec4, misc vec4u. Matches `Object` in common.wgsl. */
export const OBJECT_FLOATS = 24
export const OBJ_RECEIVE_SHADOW = 1

/** A mesh's run of records (one, or one per instance) and what they were last written from. */
interface Records {
  first: number
  count: number
  worldVersion: number
  instanceVersion: number
  material: number
  flags: number
  bones: number
  tint: Float32Array
  /** Frame the mesh was last drawn in; meshes unseen for a while give their records back. */
  seen: number
}

/** Frames a mesh may go undrawn before its records are freed (it gets new ones if it comes back). */
const RETAIN_FRAMES = 120

/**
 * Persistent per-object records. A mesh keeps its slot across frames and its record is rewritten only
 * when its transform, instances, material or tint change, so a still scene uploads almost nothing.
 * Draws do not read records in order: each frame's draw list maps instance_index to a record.
 */
export class ObjectTable {
  buffer: GPUBuffer
  /** Bumped when `buffer` is replaced, so bind groups holding it are rebuilt. */
  generation = 0
  private data: Float32Array<ArrayBuffer>
  private words: Uint32Array<ArrayBuffer>
  private readonly slots: RangeAllocator
  private readonly live = new Map<Mesh, Records>()
  private dirtyMin = Infinity
  private dirtyMax = -1
  /** Records rewritten in the last flush, for stats. */
  written = 0
  private writing = 0

  constructor(private readonly device: GPUDevice, capacity = 4096) {
    this.slots = new RangeAllocator(capacity)
    this.data = new Float32Array(capacity * OBJECT_FLOATS)
    this.words = new Uint32Array(this.data.buffer)
    this.buffer = this.makeBuffer(capacity)
  }

  /** The first record of `mesh`, rewriting its records if anything they hold has changed. */
  sync(mesh: Mesh, material: number, frame: number, bones = 0): number {
    const instanced = mesh instanceof InstancedMesh
    const count = instanced ? mesh.count : 1
    const flags = mesh.receiveShadow ? OBJ_RECEIVE_SHADOW : 0
    let r = this.live.get(mesh)
    if (r && r.count !== count) {
      this.slots.release(r.first, r.count)
      this.live.delete(mesh)
      r = undefined
    }
    if (!r) {
      r = {
        first: this.alloc(count),
        count,
        worldVersion: -1,
        instanceVersion: -1,
        material: -1,
        flags: -1,
        bones: -1,
        tint: new Float32Array(4),
        seen: frame,
      }
      this.live.set(mesh, r)
    }
    r.seen = frame
    const t = mesh.tint
    const tint = r.tint
    const instanceVersion = instanced ? mesh.version : 0
    if (
      r.worldVersion !== mesh.worldVersion || r.instanceVersion !== instanceVersion || r.material !== material || r.flags !== flags || r.bones !== bones ||
      tint[0] !== t[0] || tint[1] !== t[1] || tint[2] !== t[2] || tint[3] !== t[3]
    ) {
      r.worldVersion = mesh.worldVersion
      r.instanceVersion = instanceVersion
      r.material = material
      r.flags = flags
      r.bones = bones
      tint.set(t)
      if (instanced) for (let i = 0; i < count; i++) this.writeInstance(r.first + i, mesh, i, material, flags)
      else this.write(r.first, mesh.matrixWorld, t, material, flags, bones)
      this.dirtyMin = Math.min(this.dirtyMin, r.first)
      this.dirtyMax = Math.max(this.dirtyMax, r.first + count - 1)
      this.writing += count
    }
    return r.first
  }

  /** Frees the records of meshes not drawn for a while. Call once per frame. */
  sweep(frame: number) {
    if (frame % 30) return
    for (const [mesh, r] of this.live) {
      if (frame - r.seen < RETAIN_FRAMES) continue
      this.slots.release(r.first, r.count)
      this.live.delete(mesh)
    }
  }

  /** Uploads the records written since the last flush. */
  flush() {
    this.written = this.writing
    this.writing = 0
    if (this.dirtyMax < 0) return
    const start = this.dirtyMin * OBJECT_FLOATS
    const end = (this.dirtyMax + 1) * OBJECT_FLOATS
    this.device.queue.writeBuffer(this.buffer, start * 4, this.data, start, end - start)
    this.dirtyMin = Infinity
    this.dirtyMax = -1
  }

  get count() {
    return this.slots.used
  }

  private alloc(count: number): number {
    let first = this.slots.alloc(count)
    while (first < 0) {
      const capacity = Math.max(this.slots.capacity * 2, this.slots.capacity + count)
      const data = new Float32Array(capacity * OBJECT_FLOATS)
      data.set(this.data)
      this.data = data
      this.words = new Uint32Array(data.buffer)
      this.slots.grow(capacity)
      this.buffer.destroy()
      this.buffer = this.makeBuffer(capacity)
      this.generation++
      // The new buffer is empty: everything already written must go up again.
      this.dirtyMin = 0
      this.dirtyMax = Math.max(this.dirtyMax, this.slots.capacity - 1)
      first = this.slots.alloc(count)
    }
    return first
  }

  private write(index: number, model: Float32Array, tint: Float32Array, material: number, flags: number, bones: number) {
    const o = index * OBJECT_FLOATS
    const d = this.data
    for (let k = 0; k < 16; k++) d[o + k] = model[k]!
    d[o + 16] = tint[0]!
    d[o + 17] = tint[1]!
    d[o + 18] = tint[2]!
    d[o + 19] = tint[3]!
    this.words[o + 20] = material
    this.words[o + 21] = flags
    this.words[o + 22] = bones
  }

  private writeInstance(index: number, mesh: InstancedMesh, i: number, material: number, flags: number) {
    const o = index * OBJECT_FLOATS
    const d = this.data
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
    this.words[o + 20] = material
    this.words[o + 21] = flags
    this.words[o + 22] = 0
  }

  private makeBuffer(capacity: number) {
    return this.device.createBuffer({
      label: 'objects',
      size: capacity * OBJECT_FLOATS * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    })
  }
}
