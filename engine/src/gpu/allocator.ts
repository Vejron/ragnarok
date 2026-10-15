/**
 * Hands out ranges of a linear space (vertices, indices, buffer records): first fit from a sorted free
 * list, with neighbours merged on free. Pure bookkeeping; the owner maps ranges onto a GPU buffer.
 */
export class RangeAllocator {
  /** Free ranges, sorted by offset, never adjacent (adjacent ones are merged). */
  private readonly free: { offset: number; size: number }[]

  constructor(public capacity: number) {
    this.free = capacity > 0 ? [{ offset: 0, size: capacity }] : []
  }

  /** Returns the offset of `size` units, or -1 when no free range is large enough. */
  alloc(size: number): number {
    if (size <= 0) return 0
    for (let i = 0; i < this.free.length; i++) {
      const range = this.free[i]!
      if (range.size < size) continue
      const offset = range.offset
      range.offset += size
      range.size -= size
      if (!range.size) this.free.splice(i, 1)
      return offset
    }
    return -1
  }

  release(offset: number, size: number) {
    if (size <= 0) return
    let i = 0
    while (i < this.free.length && this.free[i]!.offset < offset) i++
    const prev = this.free[i - 1]
    const next = this.free[i]
    if (prev && prev.offset + prev.size === offset) {
      prev.size += size
      if (next && prev.offset + prev.size === next.offset) {
        prev.size += next.size
        this.free.splice(i, 1)
      }
    } else if (next && offset + size === next.offset) {
      next.offset = offset
      next.size += size
    } else {
      this.free.splice(i, 0, { offset, size })
    }
  }

  /** Extends the space; the new tail is free. */
  grow(capacity: number) {
    if (capacity <= this.capacity) return
    this.release(this.capacity, capacity - this.capacity)
    this.capacity = capacity
  }

  /** Units in use, for stats. */
  get used() {
    return this.capacity - this.free.reduce((sum, r) => sum + r.size, 0)
  }
}
