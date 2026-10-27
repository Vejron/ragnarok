import { LIGHT_FLOATS, LIGHT_GRID, type LightGrid } from '../scene/lightgrid'

/** The frame's light grid (scene/lightgrid.ts) on the GPU: the records and the cells, at group 0 bindings 7 and 8. */
export class LightBuffers {
  readonly records: GPUBuffer
  readonly cells: GPUBuffer

  constructor(private readonly device: GPUDevice, grid: LightGrid) {
    const usage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST
    this.records = device.createBuffer({ label: 'lights', size: grid.records.byteLength, usage })
    this.cells = device.createBuffer({ label: 'light grid', size: grid.grid.byteLength, usage })
  }

  /** Writes what `grid` lists this frame. With no light listed the shaders look at neither buffer, so nothing goes up. */
  upload(grid: LightGrid) {
    if (grid.count === 0) return
    const queue = this.device.queue
    queue.writeBuffer(this.records, 0, grid.records, 0, grid.count * LIGHT_FLOATS)
    queue.writeBuffer(this.cells, 0, grid.grid, 0, LIGHT_GRID * LIGHT_GRID + grid.indexCount)
  }
}
