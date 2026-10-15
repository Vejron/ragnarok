/**
 * GPU time per render pass from timestamp queries, when the device has 'timestamp-query'. Results arrive
 * a few frames late (they are read back asynchronously) and are smoothed. Without the feature every
 * method is a no-op and `ms` stays empty.
 */
export class GpuTimer {
  /** Smoothed milliseconds per pass name. */
  readonly ms: Record<string, number> = {}
  private readonly querySet: GPUQuerySet | null
  private readonly resolveBuffer: GPUBuffer | null
  private readonly readBuffers: GPUBuffer[] = []
  private readonly idle: GPUBuffer[] = []
  private used: string[] = []
  private pending: GPUBuffer | null = null
  private pendingNames: string[] = []

  constructor(private readonly device: GPUDevice, private readonly maxPasses = 8) {
    if (!device.features.has('timestamp-query')) {
      this.querySet = null
      this.resolveBuffer = null
      return
    }
    this.querySet = device.createQuerySet({ type: 'timestamp', count: maxPasses * 2 })
    this.resolveBuffer = device.createBuffer({
      size: maxPasses * 16,
      usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC,
    })
    for (let i = 0; i < 3; i++) {
      const buffer = device.createBuffer({ size: maxPasses * 16, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST })
      this.readBuffers.push(buffer)
      this.idle.push(buffer)
    }
  }

  get supported() {
    return this.querySet !== null
  }

  /** Call at the start of a frame. */
  begin() {
    this.used = []
  }

  /** The timestampWrites for a pass named `name`, or undefined when not timing. */
  pass(name: string): GPURenderPassTimestampWrites | undefined {
    if (!this.querySet || this.used.length >= this.maxPasses) return undefined
    const i = this.used.length
    this.used.push(name)
    return { querySet: this.querySet, beginningOfPassWriteIndex: i * 2, endOfPassWriteIndex: i * 2 + 1 }
  }

  /** Records the copy of this frame's timestamps; call on the frame's encoder before finishing it. */
  resolve(encoder: GPUCommandEncoder) {
    if (!this.querySet || !this.resolveBuffer || !this.used.length) return
    const target = this.idle.pop()
    if (!target) {
      // All read buffers are still being mapped: skip timing this frame.
      this.used = []
      return
    }
    encoder.resolveQuerySet(this.querySet, 0, this.used.length * 2, this.resolveBuffer, 0)
    encoder.copyBufferToBuffer(this.resolveBuffer, 0, target, 0, this.used.length * 16)
    this.pending = target
    this.pendingNames = this.used
  }

  /** Call after submitting the frame: starts reading back what `resolve` copied. */
  read() {
    const buffer = this.pending
    if (!buffer) return
    const names = this.pendingNames
    this.pending = null
    buffer.mapAsync(GPUMapMode.READ).then(
      () => {
        const times = new BigInt64Array(buffer.getMappedRange())
        names.forEach((name, i) => {
          const ns = Number(times[i * 2 + 1]! - times[i * 2]!)
          const ms = ns / 1e6
          // Some implementations quantise or report junk on the first frames; ignore negative spans.
          if (ms >= 0 && ms < 1000) this.ms[name] = (this.ms[name] ?? ms) * 0.9 + ms * 0.1
        })
        buffer.unmap()
        this.idle.push(buffer)
      },
      () => this.idle.push(buffer),
    )
  }
}
