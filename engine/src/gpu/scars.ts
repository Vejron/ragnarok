import { SCAR_SLOT_FLOATS, type Scars } from '../scene/scars'

/** Past this many changed slots the whole list goes up at once. */
const WHOLE_LIST = 64

/** The scene's scars (scene/scars.ts) on the GPU, at group 0 binding 13. */
export class ScarBuffers {
  readonly list: GPUBuffer
  private seen: Scars | null = null
  private version = -1

  constructor(private readonly device: GPUDevice, scars: Scars) {
    this.list = device.createBuffer({ label: 'scars', size: scars.data.byteLength, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST })
  }

  /** Brings the buffer up to `scars`: only the slots that changed, unless it last held another list. */
  upload(scars: Scars) {
    if (scars === this.seen && scars.version === this.version) return
    const slots = scars.takeDirty()
    const queue = this.device.queue
    if (scars !== this.seen || slots.length > WHOLE_LIST) queue.writeBuffer(this.list, 0, scars.data)
    else for (const slot of slots) queue.writeBuffer(this.list, slot * SCAR_SLOT_FLOATS * 4, scars.data, slot * SCAR_SLOT_FLOATS, SCAR_SLOT_FLOATS)
    this.seen = scars
    this.version = scars.version
  }
}
