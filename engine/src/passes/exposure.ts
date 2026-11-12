import type { Adaptation } from '../scene/scene'

/** Samples a side the metering takes of the image: 32 × 32, a 16 × 16 workgroup taking four each. */
const GRID = 32

/**
 * The eye's adaptation: one workgroup meters the resolved HDR image (the log-average luminance of a
 * 32 × 32 grid, weighted toward the middle of the screen), eases the adapted value toward it, and
 * writes the exposure the output pass multiplies by. All of it stays on the GPU; `read` brings the
 * numbers back now and then for the stats.
 */
const WGSL = /* wgsl */ `
struct Params {
  // the key, the strength, the least and the most exposure.
  curve: vec4f,
  // seconds since the last frame, the rate when the view darkens and when it brightens (per second), whether to start over (0 or 1).
  time: vec4f,
}

// The adapted log-luminance, the exposure, whether it has been set (0 or 1), the metered luminance this frame.
@group(0) @binding(0) var<storage, read_write> state: array<f32, 4>;
@group(0) @binding(1) var hdr: texture_2d<f32>;
@group(0) @binding(2) var<uniform> params: Params;

var<workgroup> logs: array<f32, 256>;
var<workgroup> weights: array<f32, 256>;

@compute @workgroup_size(16, 16)
fn main(@builtin(local_invocation_id) id: vec3u, @builtin(local_invocation_index) index: u32) {
  let size = vec2f(textureDimensions(hdr));
  var sum = 0.0;
  var weight = 0.0;
  for (var i = 0u; i < 4u; i++) {
    let cell = vec2f(id.xy * 2u + vec2u(i & 1u, i >> 1u)) + 0.5;
    let uv = cell / ${GRID}.0;
    let c = textureLoad(hdr, vec2i(uv * size), 0).rgb;
    let luminance = dot(c, vec3f(0.2126, 0.7152, 0.0722));
    // The middle of the screen counts about three times as much as its corners.
    let w = 1.0 - 0.65 * min(length(uv - 0.5) * 1.41, 1.0);
    sum += log(max(luminance, 1e-4)) * w;
    weight += w;
  }
  logs[index] = sum;
  weights[index] = weight;
  workgroupBarrier();
  for (var stride = 128u; stride > 0u; stride >>= 1u) {
    if (index < stride) {
      logs[index] += logs[index + stride];
      weights[index] += weights[index + stride];
    }
    workgroupBarrier();
  }
  if (index == 0u) {
    let metered = logs[0] / weights[0];
    var adapted = state[0];
    if (state[2] < 0.5 || params.time.w > 0.5) {
      adapted = metered;
    } else {
      let rate = select(params.time.z, params.time.y, metered < adapted);
      adapted += (metered - adapted) * (1.0 - exp(-params.time.x * rate));
    }
    let curve = params.curve;
    let exposure = clamp(pow(curve.x / exp(adapted), curve.y), curve.z, curve.w);
    state[0] = adapted;
    state[1] = exposure;
    state[2] = 1.0;
    state[3] = exp(metered);
  }
}
`

export class EyeAdaptation {
  /** The adapted state; binding it read-only, the output pass takes the exposure from element 1. */
  readonly state: GPUBuffer
  /** The last numbers read back: the exposure, and the metered average luminance. */
  exposure = 1
  luminance = 0
  private readonly pipeline: GPUComputePipeline
  private readonly uniform: GPUBuffer
  private readonly data = new Float32Array(8)
  private readonly readback: GPUBuffer
  private reading = false
  private group: GPUBindGroup | null = null
  private wasOn = false

  constructor(private readonly device: GPUDevice) {
    this.state = device.createBuffer({ label: 'adaptation', size: 16, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST })
    // An exposure of 1 until the first metering.
    device.queue.writeBuffer(this.state, 0, new Float32Array([0, 1, 0, 0]))
    this.uniform = device.createBuffer({ label: 'adaptation', size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.readback = device.createBuffer({ label: 'adaptation readback', size: 16, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST })
    this.pipeline = device.createComputePipeline({
      label: 'adaptation',
      layout: 'auto',
      compute: { module: device.createShaderModule({ label: 'adaptation', code: WGSL }), entryPoint: 'main' },
    })
  }

  /** Binds the HDR image to meter, after the render targets are (re)made. */
  resize(hdr: GPUTexture) {
    this.group = this.device.createBindGroup({
      layout: this.pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: this.state } },
        { binding: 1, resource: hdr.createView() },
        { binding: 2, resource: { buffer: this.uniform } },
      ],
    })
  }

  /**
   * Meters this frame and eases the exposure, `dt` seconds after the last. With `settings` null the
   * exposure is left alone (the output pass then ignores it); turned on again, it starts from the view.
   */
  encode(encoder: GPUCommandEncoder, settings: Adaptation | null, dt: number, timestampWrites?: GPUComputePassTimestampWrites) {
    if (!settings || !this.group) {
      this.wasOn = false
      return
    }
    const d = this.data
    d[0] = settings.key
    d[1] = settings.strength
    d[2] = settings.min
    d[3] = settings.max
    d[4] = Math.min(dt, 0.25)
    d[5] = settings.brighten
    d[6] = settings.darken
    d[7] = this.wasOn ? 0 : 1
    this.wasOn = true
    this.device.queue.writeBuffer(this.uniform, 0, d)
    const pass = encoder.beginComputePass({ label: 'adaptation', timestampWrites })
    pass.setPipeline(this.pipeline)
    pass.setBindGroup(0, this.group)
    pass.dispatchWorkgroups(1)
    pass.end()
  }

  /** Copies the state out to be read after the frame is submitted, unless a read is still pending. */
  copyOut(encoder: GPUCommandEncoder): boolean {
    if (this.reading) return false
    this.reading = true
    encoder.copyBufferToBuffer(this.state, 0, this.readback, 0, 16)
    return true
  }

  /** After the submit that `copyOut` went into: reads the numbers when they arrive. */
  read() {
    this.readback.mapAsync(GPUMapMode.READ).then(() => {
      const values = new Float32Array(this.readback.getMappedRange().slice(0))
      this.readback.unmap()
      this.exposure = values[1]!
      this.luminance = values[3]!
      this.reading = false
    }, () => {
      this.reading = false
    })
  }

  destroy() {
    this.state.destroy()
    this.uniform.destroy()
    this.readback.destroy()
  }
}

export const ADAPTATION_WGSL = WGSL
