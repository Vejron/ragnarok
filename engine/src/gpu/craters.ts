import { CRATER_FLOATS, CRATER_SLOTS, type Craters } from '../scene/craters'

/** Past this many changed buckets the whole grid goes up at once rather than bucket by bucket. */
const WHOLE_GRID = 256

/** The scene's craters (scene/craters.ts) on the GPU: the list and its grid, at group 0 bindings 5 and 6. */
export class CraterBuffers {
  readonly list: GPUBuffer
  readonly grid: GPUBuffer
  private seen: Craters | null = null
  /** What the grid buffer holds while the craters are hidden: nothing. */
  private empty: Uint32Array<ArrayBuffer> | null = null
  private version = -1

  constructor(private readonly device: GPUDevice, craters: Craters) {
    const usage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST
    this.list = device.createBuffer({ label: 'craters', size: craters.data.byteLength, usage })
    this.grid = device.createBuffer({ label: 'crater grid', size: craters.grid.byteLength, usage })
  }

  /** Brings the buffers up to `craters`: only what changed, unless they last held another list. */
  upload(craters: Craters) {
    if (craters === this.seen && craters.version === this.version) return
    const { all, slots, buckets } = craters.takeDirty()
    const queue = this.device.queue
    if (!craters.visible) {
      // Only the grid: with no crater listed, no shader looks at the list.
      if (craters !== this.seen || all) queue.writeBuffer(this.grid, 0, (this.empty ??= new Uint32Array(craters.grid.length)))
    } else if (craters !== this.seen || all) {
      queue.writeBuffer(this.list, 0, craters.data)
      queue.writeBuffer(this.grid, 0, craters.grid)
    } else {
      for (const slot of slots) queue.writeBuffer(this.list, slot * CRATER_FLOATS * 4, craters.data, slot * CRATER_FLOATS, CRATER_FLOATS)
      if (buckets.length > WHOLE_GRID) queue.writeBuffer(this.grid, 0, craters.grid)
      else for (const b of buckets) queue.writeBuffer(this.grid, b * CRATER_SLOTS * 4, craters.grid, b * CRATER_SLOTS, CRATER_SLOTS)
    }
    this.seen = craters
    this.version = craters.version
  }
}
