import type { SkinnedMesh } from '../scene/skin'
import { RangeAllocator } from './allocator'

interface BoneSlot {
  first: number
  count: number
  seen: number
}

/** Frames a skinned mesh may go undrawn before its bone matrices are freed. */
const RETAIN_FRAMES = 120

/**
 * The skinning matrices of every skinned mesh drawn this frame (bone world × inverse bind), in one
 * storage buffer. Each mesh keeps its slice across frames so its object record does not change; the
 * matrices themselves are rewritten every frame the mesh is drawn.
 */
export class BoneTable {
  buffer: GPUBuffer
  generation = 0
  private data: Float32Array<ArrayBuffer>
  private readonly slots: RangeAllocator
  private readonly live = new Map<SkinnedMesh, BoneSlot>()
  private dirtyMin = Infinity
  private dirtyMax = -1

  constructor(private readonly device: GPUDevice, capacity = 1024) {
    this.slots = new RangeAllocator(capacity)
    this.data = new Float32Array(capacity * 16)
    this.buffer = this.makeBuffer(capacity)
  }

  /** Writes the mesh's skinning matrices; returns the index of its first one. */
  sync(mesh: SkinnedMesh, frame: number): number {
    const bones = mesh.skeleton.bones
    let slot = this.live.get(mesh)
    if (slot && slot.count !== bones.length) {
      this.slots.release(slot.first, slot.count)
      this.live.delete(mesh)
      slot = undefined
    }
    if (!slot) {
      slot = { first: this.alloc(bones.length), count: bones.length, seen: -1 }
      this.live.set(mesh, slot)
    }
    // Drawn in the shadow pass and the main pass alike: one write a frame.
    if (slot.seen === frame) return slot.first
    slot.seen = frame
    const inverse = mesh.skeleton.inverseBind
    const d = this.data
    for (let b = 0; b < bones.length; b++) {
      const a = bones[b]!.matrixWorld
      const o = (slot.first + b) * 16
      const io = b * 16
      for (let c = 0; c < 4; c++) {
        const b0 = inverse[io + c * 4]!, b1 = inverse[io + c * 4 + 1]!, b2 = inverse[io + c * 4 + 2]!, b3 = inverse[io + c * 4 + 3]!
        d[o + c * 4] = a[0]! * b0 + a[4]! * b1 + a[8]! * b2 + a[12]! * b3
        d[o + c * 4 + 1] = a[1]! * b0 + a[5]! * b1 + a[9]! * b2 + a[13]! * b3
        d[o + c * 4 + 2] = a[2]! * b0 + a[6]! * b1 + a[10]! * b2 + a[14]! * b3
        d[o + c * 4 + 3] = a[3]! * b0 + a[7]! * b1 + a[11]! * b2 + a[15]! * b3
      }
    }
    this.dirtyMin = Math.min(this.dirtyMin, slot.first)
    this.dirtyMax = Math.max(this.dirtyMax, slot.first + slot.count - 1)
    return slot.first
  }

  flush() {
    if (this.dirtyMax < 0) return
    const start = this.dirtyMin * 16
    const end = (this.dirtyMax + 1) * 16
    this.device.queue.writeBuffer(this.buffer, start * 4, this.data, start, end - start)
    this.dirtyMin = Infinity
    this.dirtyMax = -1
  }

  sweep(frame: number) {
    if (frame % 30) return
    for (const [mesh, slot] of this.live) {
      if (frame - slot.seen < RETAIN_FRAMES) continue
      this.slots.release(slot.first, slot.count)
      this.live.delete(mesh)
    }
  }

  private alloc(count: number): number {
    let first = this.slots.alloc(count)
    while (first < 0) {
      const capacity = Math.max(this.slots.capacity * 2, this.slots.capacity + count)
      const data = new Float32Array(capacity * 16)
      data.set(this.data)
      this.data = data
      this.slots.grow(capacity)
      this.buffer.destroy()
      this.buffer = this.makeBuffer(capacity)
      this.generation++
      this.dirtyMin = 0
      this.dirtyMax = capacity - 1
      first = this.slots.alloc(count)
    }
    return first
  }

  private makeBuffer(capacity: number) {
    return this.device.createBuffer({ label: 'bones', size: capacity * 64, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST })
  }
}
