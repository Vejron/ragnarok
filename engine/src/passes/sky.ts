import { COMMON_WGSL } from '../materials/common.wgsl'
import { CLOUD_MAP_SIZE } from '../materials/sky.wgsl'
import { ATMOSPHERE_WGSL } from '../scene/atmosphere'
import type { Sky } from '../scene/sky'

const TRANSMITTANCE_SIZE = [256, 64] as const
const MULTI_SCATTER_SIZE = 32
const SKY_VIEW_SIZE = [192, 108] as const
const LUT_FORMAT: GPUTextureFormat = 'rgba16float'
/** The sky-view table is rebuilt when the sun's height (the sine of its elevation) moves by more than this. */
const SUN_MOVE = 2e-4

/** What the table-building shaders share: the atmosphere's matter and the transmittance table's layout. */
const ATMOSPHERE_FUNCTIONS = /* wgsl */ `
${ATMOSPHERE_WGSL}
const PI = 3.141592653589793;

struct Medium {
  rayleigh: vec3f,
  mie: f32,
  extinction: vec3f,
}

// The air, haze and ozone at \`h\` km up.
fn medium(h: f32) -> Medium {
  let r = exp(-h / ATMO_RAYLEIGH_HEIGHT);
  let m = exp(-h / ATMO_MIE_HEIGHT);
  let o = max(0.0, 1.0 - abs(h - ATMO_OZONE_CENTRE) / ATMO_OZONE_WIDTH);
  return Medium(ATMO_RAYLEIGH * r, ATMO_MIE_SCATTER * m, ATMO_RAYLEIGH * r + vec3f(ATMO_MIE_EXTINCTION * m) + ATMO_OZONE * o);
}

// How far a ray from radius \`r\` at cosine \`mu\` to the vertical goes to the sphere of radius \`big\`, km, or -1.
fn toSphere(r: f32, mu: f32, big: f32) -> f32 {
  let disc = r * r * (mu * mu - 1.0) + big * big;
  if (disc < 0.0) {
    return -1.0;
  }
  let s = sqrt(disc);
  let near = -r * mu - s;
  if (near > 0.0) {
    return near;
  }
  let far = -r * mu + s;
  return select(-1.0, far, far > 0.0);
}

// The horizontal distance from the ground's horizon to the atmosphere's top, km.
fn topHorizon() -> f32 {
  return sqrt(ATMO_TOP * ATMO_TOP - ATMO_BOTTOM * ATMO_BOTTOM);
}

// Where the transmittance table holds radius \`r\` and cosine \`mu\` (Bruneton's layout: fine near the horizon and the ground).
fn transmittanceUv(r: f32, mu: f32) -> vec2f {
  let big = topHorizon();
  let rho = sqrt(max(r * r - ATMO_BOTTOM * ATMO_BOTTOM, 0.0));
  let disc = r * r * (mu * mu - 1.0) + ATMO_TOP * ATMO_TOP;
  let d = max(-r * mu + sqrt(max(disc, 0.0)), 0.0);
  let dMin = ATMO_TOP - r;
  let dMax = rho + big;
  return vec2f((d - dMin) / (dMax - dMin), rho / big);
}

fn cornetteShanks(nu: f32, g: f32) -> f32 {
  let k = 3.0 / (8.0 * PI) * (1.0 - g * g) / (2.0 + g * g);
  return k * (1.0 + nu * nu) / pow(1.0 + g * g - 2.0 * g * nu, 1.5);
}
`

/** The share of sunlight that gets through the atmosphere from its top to each radius and angle. */
const TRANSMITTANCE_WGSL = /* wgsl */ `
${ATMOSPHERE_FUNCTIONS}
@group(0) @binding(0) var table: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
  let size = textureDimensions(table);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let uv = (vec2f(id.xy) + 0.5) / vec2f(size);
  let big = topHorizon();
  let rho = big * uv.y;
  let r = sqrt(rho * rho + ATMO_BOTTOM * ATMO_BOTTOM);
  let dMin = ATMO_TOP - r;
  let dMax = rho + big;
  let d = dMin + uv.x * (dMax - dMin);
  let mu = select(clamp((big * big - rho * rho - d * d) / (2.0 * r * d), -1.0, 1.0), 1.0, d < 1e-4);
  let steps = 40u;
  let dt = d / f32(steps);
  var depth = vec3f(0.0);
  for (var i = 0u; i < steps; i++) {
    let t = (f32(i) + 0.5) * dt;
    let h = sqrt(r * r + t * t + 2.0 * r * t * mu) - ATMO_BOTTOM;
    depth += medium(h).extinction * dt;
  }
  textureStore(table, id.xy, vec4f(exp(-depth), 1.0));
}
`

/** Reading the transmittance table, for the two shaders after it. */
const READ_TRANSMITTANCE = /* wgsl */ `
@group(0) @binding(1) var transmittanceTable: texture_2d<f32>;
@group(0) @binding(2) var tableSampler: sampler;

// Sunlight reaching radius \`r\` from cosine \`mu\` to the vertical: none where the ground is in the way.
fn sunTransmittance(r: f32, mu: f32) -> vec3f {
  if (toSphere(r, mu, ATMO_BOTTOM) > 0.0) {
    return vec3f(0.0);
  }
  return textureSampleLevel(transmittanceTable, tableSampler, transmittanceUv(r, mu), 0.0).rgb;
}
`

/**
 * Light scattered more than once, per radius and sun angle, for a sun of illuminance 1: Hillaire's
 * Ψ_ms, the second order of scattering summed as a geometric series (his section 5.5).
 */
const MULTI_SCATTER_WGSL = /* wgsl */ `
${ATMOSPHERE_FUNCTIONS}
@group(0) @binding(0) var table: texture_storage_2d<rgba16float, write>;
${READ_TRANSMITTANCE}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
  let size = textureDimensions(table);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let uv = (vec2f(id.xy) + 0.5) / vec2f(size);
  let muS = uv.x * 2.0 - 1.0;
  let r = ATMO_BOTTOM + uv.y * (ATMO_TOP - ATMO_BOTTOM);
  let sun = vec3f(sqrt(max(1.0 - muS * muS, 0.0)), muS, 0.0);
  let origin = vec3f(0.0, r, 0.0);
  let around = 8u;
  let steps = 20u;
  var second = vec3f(0.0);
  var transfer = vec3f(0.0);
  for (var i = 0u; i < around; i++) {
    for (var j = 0u; j < around; j++) {
      let cosT = 1.0 - 2.0 * (f32(i) + 0.5) / f32(around);
      let sinT = sqrt(max(1.0 - cosT * cosT, 0.0));
      let phi = 2.0 * PI * (f32(j) + 0.5) / f32(around);
      let dir = vec3f(sinT * cos(phi), cosT, sinT * sin(phi));
      let ground = toSphere(r, dir.y, ATMO_BOTTOM);
      let reach = select(toSphere(r, dir.y, ATMO_TOP), ground, ground > 0.0);
      let dt = reach / f32(steps);
      var through = vec3f(1.0);
      for (var s = 0u; s < steps; s++) {
        let p = origin + dir * ((f32(s) + 0.5) * dt);
        let rr = length(p);
        let m = medium(rr - ATMO_BOTTOM);
        let scatter = m.rayleigh + vec3f(m.mie);
        let ext = max(m.extinction, vec3f(1e-7));
        let step = exp(-ext * dt);
        let light = scatter * sunTransmittance(rr, dot(p / rr, sun)) / (4.0 * PI);
        second += through * (light - light * step) / ext;
        transfer += through * (scatter - scatter * step) / ext;
        through *= step;
      }
      if (ground > 0.0) {
        let p = origin + dir * ground;
        let rr = length(p);
        let mu = dot(p / rr, sun);
        second += through * sunTransmittance(rr, mu) * ATMO_GROUND_ALBEDO / PI * max(mu, 0.0);
      }
    }
  }
  let n = f32(around * around);
  second /= n;
  transfer /= n;
  textureStore(table, id.xy, vec4f(second / (1.0 - transfer), 1.0));
}
`

/**
 * The sky's radiance around the eye for every direction, for a sun of illuminance 1 at the height in
 * `params.x` (the sine of its elevation): Hillaire's sky-view table. See `skyViewUv` in sky.wgsl.
 */
const SKY_VIEW_WGSL = /* wgsl */ `
${ATMOSPHERE_FUNCTIONS}
@group(0) @binding(0) var table: texture_storage_2d<rgba16float, write>;
${READ_TRANSMITTANCE}
@group(0) @binding(3) var multiScatterTable: texture_2d<f32>;
@group(0) @binding(4) var<uniform> params: vec4f;

fn multiScatter(r: f32, mu: f32) -> vec3f {
  let uv = vec2f(mu * 0.5 + 0.5, (r - ATMO_BOTTOM) / (ATMO_TOP - ATMO_BOTTOM));
  return textureSampleLevel(multiScatterTable, tableSampler, uv, 0.0).rgb;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
  let size = textureDimensions(table);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let uv = (vec2f(id.xy) + 0.5) / vec2f(size);
  let r = ATMO_BOTTOM + ATMO_EYE;
  let horizon = sqrt(r * r - ATMO_BOTTOM * ATMO_BOTTOM);
  let beta = acos(horizon / r);
  let zenithHorizon = PI - beta;
  var zenith: f32;
  if (uv.y < 0.5) {
    let c = 1.0 - 2.0 * uv.y;
    zenith = zenithHorizon * (1.0 - c * c);
  } else {
    let c = uv.y * 2.0 - 1.0;
    zenith = zenithHorizon + beta * c * c;
  }
  let cosPhi = -(uv.x * uv.x * 2.0 - 1.0);
  let sinPhi = sqrt(max(1.0 - cosPhi * cosPhi, 0.0));
  let sinZ = sin(zenith);
  let dir = vec3f(cosPhi * sinZ, cos(zenith), sinPhi * sinZ);
  let muS = params.x;
  let sun = vec3f(sqrt(max(1.0 - muS * muS, 0.0)), muS, 0.0);
  let nu = dot(dir, sun);
  let rayleighPhase = 3.0 / (16.0 * PI) * (1.0 + nu * nu);
  let miePhase = cornetteShanks(nu, ATMO_MIE_G);
  let ground = toSphere(r, dir.y, ATMO_BOTTOM);
  let reach = select(toSphere(r, dir.y, ATMO_TOP), ground, ground > 0.0);
  let origin = vec3f(0.0, r, 0.0);
  let steps = 32u;
  var light = vec3f(0.0);
  var through = vec3f(1.0);
  var before = 0.0;
  for (var i = 0u; i < steps; i++) {
    // Samples crowd towards the eye, where the air is thick.
    let after = reach * pow((f32(i) + 1.0) / f32(steps), 2.0);
    let dt = after - before;
    let p = origin + dir * (before + 0.3 * dt);
    before = after;
    let rr = length(p);
    let mu = dot(p / rr, sun);
    let m = medium(rr - ATMO_BOTTOM);
    let ext = max(m.extinction, vec3f(1e-7));
    let step = exp(-ext * dt);
    let s = (m.rayleigh * rayleighPhase + vec3f(m.mie * miePhase)) * sunTransmittance(rr, mu) + (m.rayleigh + vec3f(m.mie)) * multiScatter(rr, mu);
    light += through * (s - s * step) / ext;
    through *= step;
  }
  textureStore(table, id.xy, vec4f(light, 1.0));
}
`

/** Texels a side of the sky panorama: the upper half of the sky, folded into a square (`skyPanoramaUv`). */
export const PANORAMA_SIZE = 256

/**
 * The sky panorama: the sky's backdrop in every direction above the horizon, once a frame, for what
 * mirrors the sky (water) to read in one sample where working the sky out cost every one of its pixels
 * as much as the sky's own. The discs of the sun and the moon are left out, being too small for its
 * texels, and alpha says how much of them would show: a reader adds them exactly (`skyFrom`).
 */
const PANORAMA_WGSL = /* wgsl */ `
${COMMON_WGSL}

@group(1) @binding(0) var panorama: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
  let size = textureDimensions(panorama);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let uv = (vec2f(id.xy) + 0.5) / vec2f(size);
  // The clouds a little blurred, as rippled water shows them.
  textureStore(panorama, id.xy, skyBackdrop(skyPanoramaDir(uv), 2.0));
}
`

/** The sky drawn behind the world: a triangle over the screen at the far plane, so only where nothing else drew. */
const DRAW_WGSL = /* wgsl */ `
${COMMON_WGSL}

struct SkyDraw {
  inverseViewProj: mat4x4f,
}
@group(1) @binding(0) var<uniform> draw: SkyDraw;

struct SkyOut {
  @builtin(position) clip: vec4f,
  @location(0) ndc: vec2f,
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> SkyOut {
  let uv = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
  var out: SkyOut;
  out.ndc = uv * 2.0 - 1.0;
  // Reversed depth: 0 is the far plane, so the depth test passes only where nothing was drawn.
  out.clip = vec4f(out.ndc, 0.0, 1.0);
  return out;
}

@fragment
fn fs(in: SkyOut) -> @location(0) vec4f {
  let near = draw.inverseViewProj * vec4f(in.ndc, 1.0, 1.0);
  let dir = normalize(near.xyz / near.w - frame.cameraPos.xyz);
  // The cloud map's mip level, from how many of its texels a pixel spans there: before any branch.
  let texel = cloudTexel(dir);
  let span = max(length(dpdx(texel)), length(dpdy(texel)));
  let lod = clamp(log2(max(span, 1e-4)), 0.0, 7.0);
  return vec4f(skyColor(dir, lod), 1.0);
}
`

/** The sky's shaders, for checking them outside a browser. */
export const SKY_SHADERS = { transmittance: TRANSMITTANCE_WGSL, multiScatter: MULTI_SCATTER_WGSL, skyView: SKY_VIEW_WGSL, draw: DRAW_WGSL }

/**
 * The sky's GPU side: the atmosphere's lookup tables (transmittance and multiple scattering once, the
 * sky-view table again whenever the sun has moved), the cloud map, and the draw of the sky itself.
 * Its views are bound in the scene group (bindings 9 to 12) for every shader's fog and reflections.
 */
export class SkyPass {
  readonly skyView: GPUTexture
  readonly cloudMap: GPUTexture
  readonly skySampler: GPUSampler
  readonly cloudSampler: GPUSampler
  /** The sky panorama (\`PANORAMA_WGSL\`), redrawn by \`drawPanorama\`. */
  readonly panorama: GPUTexture
  private readonly panoramaPipeline: GPUComputePipeline
  private readonly panoramaGroup: GPUBindGroup
  private readonly transmittance: GPUTexture
  private readonly multiScatter: GPUTexture
  private readonly transmittancePipeline: GPUComputePipeline
  private readonly multiScatterPipeline: GPUComputePipeline
  private readonly skyViewPipeline: GPUComputePipeline
  private readonly transmittanceGroup: GPUBindGroup
  private readonly multiScatterGroup: GPUBindGroup
  private readonly skyViewGroup: GPUBindGroup
  private readonly params: GPUBuffer
  private readonly paramsData = new Float32Array(4)
  private readonly drawPipeline: GPURenderPipeline
  private readonly drawUniform: GPUBuffer
  private readonly drawData = new Float32Array(16)
  private readonly drawGroup: GPUBindGroup
  private built = false
  private sunHeight = NaN

  constructor(private readonly device: GPUDevice, sceneLayout: GPUBindGroupLayout, sampleCount: number, colorFormat: GPUTextureFormat, depthFormat: GPUTextureFormat) {
    const usage = GPUTextureUsage.STORAGE_BINDING | GPUTextureUsage.TEXTURE_BINDING
    this.transmittance = device.createTexture({ label: 'sky transmittance', size: TRANSMITTANCE_SIZE, format: LUT_FORMAT, usage })
    this.multiScatter = device.createTexture({ label: 'sky multiple scattering', size: [MULTI_SCATTER_SIZE, MULTI_SCATTER_SIZE], format: LUT_FORMAT, usage })
    this.skyView = device.createTexture({ label: 'sky view', size: SKY_VIEW_SIZE, format: LUT_FORMAT, usage })
    this.skySampler = device.createSampler({ magFilter: 'linear', minFilter: 'linear', addressModeU: 'clamp-to-edge', addressModeV: 'clamp-to-edge' })
    this.cloudSampler = device.createSampler({ magFilter: 'linear', minFilter: 'linear', mipmapFilter: 'linear', addressModeU: 'repeat', addressModeV: 'repeat' })
    this.cloudMap = makeCloudMap(device)
    this.params = device.createBuffer({ label: 'sky view params', size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })

    const compute = (label: string, code: string) =>
      device.createComputePipeline({ label, layout: 'auto', compute: { module: device.createShaderModule({ label, code }), entryPoint: 'main' } })
    this.transmittancePipeline = compute('sky transmittance', TRANSMITTANCE_WGSL)
    this.multiScatterPipeline = compute('sky multiple scattering', MULTI_SCATTER_WGSL)
    this.skyViewPipeline = compute('sky view', SKY_VIEW_WGSL)
    this.transmittanceGroup = device.createBindGroup({
      layout: this.transmittancePipeline.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: this.transmittance.createView() }],
    })
    this.multiScatterGroup = device.createBindGroup({
      layout: this.multiScatterPipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: this.multiScatter.createView() },
        { binding: 1, resource: this.transmittance.createView() },
        { binding: 2, resource: this.skySampler },
      ],
    })
    this.skyViewGroup = device.createBindGroup({
      layout: this.skyViewPipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: this.skyView.createView() },
        { binding: 1, resource: this.transmittance.createView() },
        { binding: 2, resource: this.skySampler },
        { binding: 3, resource: this.multiScatter.createView() },
        { binding: 4, resource: { buffer: this.params } },
      ],
    })

    const drawLayout = device.createBindGroupLayout({
      label: 'sky draw',
      entries: [{ binding: 0, visibility: GPUShaderStage.FRAGMENT, buffer: { type: 'uniform' } }],
    })
    const module = device.createShaderModule({ label: 'sky', code: DRAW_WGSL })
    this.drawPipeline = device.createRenderPipeline({
      label: 'sky',
      layout: device.createPipelineLayout({ bindGroupLayouts: [sceneLayout, drawLayout] }),
      vertex: { module, entryPoint: 'vs' },
      fragment: { module, entryPoint: 'fs', targets: [{ format: colorFormat }] },
      primitive: { topology: 'triangle-list' },
      depthStencil: { format: depthFormat, depthWriteEnabled: false, depthCompare: 'greater-equal' },
      multisample: { count: sampleCount },
    })
    this.drawUniform = device.createBuffer({ label: 'sky draw', size: 64, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.panorama = device.createTexture({ label: 'sky panorama', size: [PANORAMA_SIZE, PANORAMA_SIZE], format: 'rgba16float', usage })
    const panoramaLayout = device.createBindGroupLayout({
      label: 'sky panorama',
      entries: [{ binding: 0, visibility: GPUShaderStage.COMPUTE, storageTexture: { access: 'write-only', format: 'rgba16float' } }],
    })
    this.panoramaPipeline = device.createComputePipeline({
      label: 'sky panorama',
      layout: device.createPipelineLayout({ bindGroupLayouts: [sceneLayout, panoramaLayout] }),
      compute: { module: device.createShaderModule({ label: 'sky panorama', code: PANORAMA_WGSL }), entryPoint: 'main' },
    })
    this.panoramaGroup = device.createBindGroup({ layout: panoramaLayout, entries: [{ binding: 0, resource: this.panorama.createView() }] })
    this.drawGroup = device.createBindGroup({ layout: drawLayout, entries: [{ binding: 0, resource: { buffer: this.drawUniform } }] })
  }

  /** Builds the fixed tables the first time, and the sky-view table again if the sun has moved since. */
  update(encoder: GPUCommandEncoder, sky: Sky) {
    const height = sky.sunDir.y / (Math.hypot(sky.sunDir.x, sky.sunDir.y, sky.sunDir.z) || 1)
    const rebuild = !this.built || !(Math.abs(height - this.sunHeight) < SUN_MOVE)
    if (!rebuild) return
    this.sunHeight = height
    this.paramsData[0] = height
    this.device.queue.writeBuffer(this.params, 0, this.paramsData)
    const pass = encoder.beginComputePass({ label: 'sky tables' })
    if (!this.built) {
      this.built = true
      pass.setPipeline(this.transmittancePipeline)
      pass.setBindGroup(0, this.transmittanceGroup)
      pass.dispatchWorkgroups(Math.ceil(TRANSMITTANCE_SIZE[0] / 8), Math.ceil(TRANSMITTANCE_SIZE[1] / 8))
      pass.setPipeline(this.multiScatterPipeline)
      pass.setBindGroup(0, this.multiScatterGroup)
      pass.dispatchWorkgroups(MULTI_SCATTER_SIZE / 8, MULTI_SCATTER_SIZE / 8)
    }
    pass.setPipeline(this.skyViewPipeline)
    pass.setBindGroup(0, this.skyViewGroup)
    pass.dispatchWorkgroups(Math.ceil(SKY_VIEW_SIZE[0] / 8), Math.ceil(SKY_VIEW_SIZE[1] / 8))
    pass.end()
  }

  /** Sets the camera's inverse view-projection for this frame's draw. */
  setView(inverseViewProj: ArrayLike<number>) {
    this.drawData.set(inverseViewProj)
    this.device.queue.writeBuffer(this.drawUniform, 0, this.drawData)
  }

  /** Redraws the sky panorama for this frame's sky (the scene group holds the frame's uniforms). */
  drawPanorama(encoder: GPUCommandEncoder, sceneGroup: GPUBindGroup) {
    const pass = encoder.beginComputePass({ label: 'sky panorama' })
    pass.setPipeline(this.panoramaPipeline)
    pass.setBindGroup(0, sceneGroup)
    pass.setBindGroup(1, this.panoramaGroup)
    pass.dispatchWorkgroups(PANORAMA_SIZE / 8, PANORAMA_SIZE / 8)
    pass.end()
  }

  /** Draws the sky into `pass` (after the opaque world, before anything translucent). */
  draw(pass: GPURenderPassEncoder, sceneGroup: GPUBindGroup) {
    pass.setPipeline(this.drawPipeline)
    pass.setBindGroup(0, sceneGroup)
    pass.setBindGroup(1, this.drawGroup)
    pass.draw(3)
  }

  destroy() {
    this.transmittance.destroy()
    this.multiScatter.destroy()
    this.skyView.destroy()
    this.cloudMap.destroy()
    this.panorama.destroy()
  }
}

/**
 * The cloud map: red is broad, billowing cover (six octaves of value noise, evened out so that a cover
 * of c clouds over a share c of the sky), green finer detail (four octaves). Both repeat across the
 * tile. Made once on the CPU, with its mip chain.
 */
function makeCloudMap(device: GPUDevice): GPUTexture {
  const size = CLOUD_MAP_SIZE
  const levels = Math.log2(size) + 1
  const texture = device.createTexture({
    label: 'cloud map',
    size: [size, size],
    format: 'rg8unorm',
    mipLevelCount: levels,
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
  })
  const broad = fbm(size, 4, 6, 0x5eed)
  const fine = fbm(size, 8, 4, 0xc10d)
  equalize(broad)
  stretch(fine)
  let width = size
  let r = broad
  let g = fine
  for (let level = 0; level < levels; level++) {
    const data = new Uint8Array(width * width * 2)
    for (let i = 0; i < width * width; i++) {
      data[i * 2] = Math.round(r[i]! * 255)
      data[i * 2 + 1] = Math.round(g[i]! * 255)
    }
    device.queue.writeTexture({ texture, mipLevel: level }, data, { bytesPerRow: width * 2 }, [width, width])
    if (width > 1) {
      r = halve(r, width)
      g = halve(g, width)
      width /= 2
    }
  }
  return texture
}

/** Repeating fractal value noise over a `size`² grid: `octaves` octaves from `cells` cells across, each twice as fine and half as strong. */
function fbm(size: number, cells: number, octaves: number, seed: number): Float32Array {
  const out = new Float32Array(size * size)
  let amplitude = 1
  for (let o = 0; o < octaves; o++) {
    const period = cells << o
    const lattice = new Float32Array(period * period)
    for (let i = 0; i < lattice.length; i++) lattice[i] = hash(i + seed * 7919 + o * 104729)
    const scale = period / size
    for (let y = 0; y < size; y++) {
      const fy = y * scale
      const iy = Math.floor(fy)
      const ty = smooth(fy - iy)
      const y0 = (iy % period) * period
      const y1 = ((iy + 1) % period) * period
      for (let x = 0; x < size; x++) {
        const fx = x * scale
        const ix = Math.floor(fx)
        const tx = smooth(fx - ix)
        const x0 = ix % period
        const x1 = (ix + 1) % period
        const a = lattice[y0 + x0]!, b = lattice[y0 + x1]!, c = lattice[y1 + x0]!, d = lattice[y1 + x1]!
        out[y * size + x]! += amplitude * (a + (b - a) * tx + (c - a) * ty + (a - b - c + d) * tx * ty)
      }
    }
    amplitude *= 0.5
  }
  return out
}

function smooth(t: number) {
  return t * t * t * (t * (t * 6 - 15) + 10)
}

/** A hash of an integer, 0 to 1. */
function hash(n: number): number {
  let h = Math.imul(n ^ 0x9e3779b9, 0x85ebca6b)
  h ^= h >>> 13
  h = Math.imul(h, 0xc2b2ae35)
  h ^= h >>> 16
  return (h >>> 0) / 4294967296
}

/** Replaces each value by its rank, 0 to 1, so the values are spread evenly. */
function equalize(values: Float32Array) {
  const order = new Uint32Array(values.length)
  for (let i = 0; i < order.length; i++) order[i] = i
  order.sort((a, b) => values[a]! - values[b]!)
  const last = values.length - 1
  const ranks = new Float32Array(values.length)
  for (let i = 0; i < order.length; i++) ranks[order[i]!] = i / last
  values.set(ranks)
}

/** Stretches the values to fill 0 to 1. */
function stretch(values: Float32Array) {
  let lo = Infinity, hi = -Infinity
  for (const v of values) {
    lo = Math.min(lo, v)
    hi = Math.max(hi, v)
  }
  const range = hi - lo || 1
  for (let i = 0; i < values.length; i++) values[i] = (values[i]! - lo) / range
}

/** A `width`² grid averaged down to half its width. */
function halve(values: Float32Array, width: number): Float32Array {
  const half = width / 2
  const out = new Float32Array(half * half)
  for (let y = 0; y < half; y++) {
    for (let x = 0; x < half; x++) {
      const i = y * 2 * width + x * 2
      out[y * half + x] = (values[i]! + values[i + 1]! + values[i + width]! + values[i + width + 1]!) / 4
    }
  }
  return out
}
