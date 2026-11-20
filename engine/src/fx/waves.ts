import { Effect, type EffectContext } from './effect'
import { Texture } from '../scene/texture'
import type { TextureSlot } from '../gpu/textures'

/**
 * Something moving through the water this frame: a hull (a rounded box `halfWidth` by `halfLength`
 * metres, its length along `ahead`) or, with both halves equal, a round body (a wader). It sits `depth`
 * metres into the water and moves at `velocity`; `churn` is how much foam it whips up a second.
 */
export interface WaveStamp {
  x: number
  z: number
  /** Unit direction of its length, level: `[x, z]`. */
  ahead: readonly [number, number]
  halfWidth: number
  halfLength: number
  /** Metres. */
  depth: number
  /** Metres a second, level: `[x, z]`. */
  velocity: readonly [number, number]
  /**
   * How deep it sat in the last frame that stepped, metres; `depth` when left out. The change between
   * the two moves the water where it is, so a hull settling in pushes the water down under it and one
   * lifting out lets it back up, rather than leaving a hole or a heap where it began or stopped.
   */
  was?: number
  /** Foam a second inside it, 0 to a few. */
  churn: number
}

/** How the field is laid out and how its water behaves. */
export interface WaveFieldParams {
  /** Texels a side. */
  grid?: number
  /** Metres a texel. */
  texel?: number
  /** How fast ripples run out, m/s. */
  speed?: number
  /** Share of a wave's height lost per second. */
  damping?: number
  /** Seconds foam takes to fade to a third. */
  foamFade?: number
}

/** Steps a second the water is worked out at, whatever the frame rate. */
const RATE = 60
/** The most steps one frame catches up, after a stall. */
const MOST_STEPS = 4
/** The most stamps one frame takes. */
export const MOST_STAMPS = 64
/** Floats a stamp takes in the buffer. */
const STAMP_FLOATS = 12
/** Bytes between the uniforms of one step and the next (WebGPU's dynamic offset alignment). */
const UNIFORM_STRIDE = 256
/** Floats the uniforms of one step take (`Sim` in the shader). */
const SIM_FLOATS = 20
/** Texels a side of the current's map (`WaveField.setFlow`). */
export const FLOW_GRID = 64
/** Seconds with nothing pressing in after which the water has settled and the steps stop. */
const SETTLE = 10

const WAVES_WGSL = /* wgsl */ `
struct Sim {
  // The field's corner (world x, z), metres a texel, the wave equation's (c dt / dx)^2.
  frame: vec4f,
  // How far the field moved since the step before, texels (x, z), how many stamps, texels a side.
  shift: vec4i,
  // Seconds a step, the share of a wave's motion it keeps, the share of foam it keeps, 1 in the frame's first step.
  step: vec4f,
  // How far through the frame's steps this one starts and ends (0 to 1), for stamps whose depth changes.
  share: vec4f,
  // The current's map: its corner (world x, z), one over its side (1/metres), and the m/s one step of
  // a byte in it stands for (0 with no current).
  flow: vec4f,
}

struct Stamp {
  // x, z, and the unit direction of its length.
  at: vec4f,
  // Half its width and length, how deep it sits (or a splash knocks the water down), foam a second (or a splash's foam).
  shape: vec4f,
  // Its velocity (x, z, m/s), 1 for a splash, and how deep it sat in the last frame that stepped.
  motion: vec4f,
}

// How high the water a stamp pushes aside heaps up round it, against how deep it sits.
const RIM_HEIGHT = 0.6;
// How far out the heap reaches against half the stamp's width, and its least and most, metres.
const RIM_REACH = 0.8;
const RIM_LEAST = 0.4;
const RIM_MOST = 2.0;
// A hull (longer than this many times its width) narrows to a point over the front of its length,
// from this share of the half length ahead of its middle, to this share of its beam at the stem.
const HULL_LONG = 1.5;
const BOW_FROM = 0.2;
const BOW_POINT = 0.25;
// How deep a hull sits at its sides against at its keel: a rounded bottom.
const BILGE = 0.35;
// Water does not stack up without end: a crest or trough nears this height, metres, and no further,
// and a slope steeper than BREAK_STEEP spends part of the water's motion each step as it breaks.
// Without them a hull at just the ripples' speed keeps adding to its own wave until it is metres high.
const BREAK = 0.9;
const BREAK_STEEP = vec2f(0.5, 1.0);

@group(0) @binding(0) var<uniform> sim: Sim;
@group(0) @binding(1) var<storage, read> stamps: array<Stamp>;
@group(0) @binding(2) var before: texture_2d<f32>;
@group(0) @binding(3) var after: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var flowMap: texture_2d<f32>;
@group(0) @binding(5) var flowSampler: sampler;

// The current that carries the water at \`world\`, m/s (x, z): none without a map of it, or off the map.
fn drift(world: vec2f) -> vec2f {
  if (sim.flow.w <= 0.0) {
    return vec2f(0.0);
  }
  let uv = (world - sim.flow.xy) * sim.flow.z;
  if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
    return vec2f(0.0);
  }
  return (textureSampleLevel(flowMap, flowSampler, uv, 0.0).xy * 255.0 - 128.0) * sim.flow.w;
}

// The water before this step at texel \`c\` of this step's field: moved with the field, still beyond its edge.
fn was(c: vec2i) -> vec4f {
  let n = sim.shift.w;
  let s = c + sim.shift.xy;
  if (s.x < 0 || s.y < 0 || s.x >= n || s.y >= n) {
    return vec4f(0.0);
  }
  return textureLoad(before, s, 0);
}

// The water before this step at \`p\`, in texels of this step's field (a texel's middle is at its index):
// blended from the four round it.
fn wasBetween(p: vec2f) -> vec4f {
  let i = floor(p);
  let f = p - i;
  let c = vec2i(i);
  return mix(mix(was(c), was(c + vec2i(1, 0)), f.x), mix(was(c + vec2i(0, 1)), was(c + vec2i(1, 1)), f.x), f.y);
}

// How far out the water a stamp pushes aside heaps up, metres: further for a wider hull.
fn rimOut(stamp: Stamp) -> f32 {
  return clamp(stamp.shape.x * RIM_REACH, RIM_LEAST, RIM_MOST);
}

// Where \`p\` is against a stamp: the signed distance from its outline (metres), and how far across its
// beam it is there (0 on the keel line, 1 at its side). A hull's outline is a box rounded at the stern
// that narrows to a point at the bow, as its waterline does; a round body's is a circle.
fn outline(stamp: Stamp, p: vec2f) -> vec2f {
  let d = p - stamp.at.xy;
  let ahead = stamp.at.zw;
  let local = vec2f(dot(d, vec2f(-ahead.y, ahead.x)), dot(d, ahead));
  let half = stamp.shape.xy;
  if (half.y > half.x * HULL_LONG) {
    // Its beam here: full aft, narrowing toward the stem. The taper is gentle enough over a long hull
    // that the distance across to its side is still near enough the true one.
    let beam = half.x * mix(1.0, BOW_POINT, smoothstep(BOW_FROM * half.y, half.y, local.y));
    let q = vec2f(abs(local.x) - beam, abs(local.y) - half.y);
    return vec2f(length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0), min(abs(local.x) / beam, 1.0));
  }
  let r = min(half.x, half.y) * 0.9;
  let q = abs(local) - (half - r);
  return vec2f(length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r, 0.0);
}

// How a stamp moves the water at \`p\` (metres): down under it, deepest along a hull's keel, and heaped
// in a rim round it.
fn displaced(stamp: Stamp, p: vec2f, depth: f32) -> f32 {
  let o = outline(stamp, p);
  let reach = rimOut(stamp);
  let bottom = select(1.0, mix(1.0, BILGE, o.y * o.y), stamp.shape.y > stamp.shape.x * HULL_LONG);
  let inside = (1.0 - smoothstep(-0.25, 0.35, o.x)) * bottom;
  let rim = smoothstep(0.0, reach * 0.25, o.x) * (1.0 - smoothstep(reach * 0.35, reach, o.x));
  return depth * (RIM_HEIGHT * rim - inside);
}

// One step of the water: a damped wave equation (height in x, the height a step before in y), with foam
// (z) that stamps and steep water whip up and that spreads and fades. A moving hull adds only how much
// the water it displaces changed in the step (iWave's way): down where it arrives, with the rim ahead
// of it up (its bow wave), and back up where it left, so it never holds a channel open behind it. A
// splash knocks its hole in once. Waves running into the field's edge are soaked up rather than
// thrown back.
//
// In a current (\`drift\`) the water is carried along: each texel takes the water that was upstream of it
// a step ago, and a stamp pushes by how it moves through that water, not over the ground. So something
// standing still in a river (a boulder under the surface) keeps pushing the water that runs onto it,
// and its waves trail away downstream, while a boat drifting with the river makes none.
@compute @workgroup_size(8, 8)
fn advance(@builtin(global_invocation_id) id: vec3u) {
  let n = sim.shift.w;
  let c = vec2i(id.xy);
  if (c.x >= n || c.y >= n) {
    return;
  }
  let world = sim.frame.xy + (vec2f(c) + 0.5) * sim.frame.z;
  let carried = drift(world);
  var here = was(c);
  var l = was(c - vec2i(1, 0));
  var r = was(c + vec2i(1, 0));
  var u = was(c - vec2i(0, 1));
  var d = was(c + vec2i(0, 1));
  if (dot(carried, carried) > 0.0001) {
    // All five from where this water was: the texel and its neighbours moved together. Carrying the
    // middle alone feeds the waves instead of moving them, and the field fills with noise.
    let source = vec2f(c) - carried * sim.step.x / sim.frame.z;
    here = wasBetween(source);
    l = wasBetween(source - vec2f(1.0, 0.0));
    r = wasBetween(source + vec2f(1.0, 0.0));
    u = wasBetween(source - vec2f(0.0, 1.0));
    d = wasBetween(source + vec2f(0.0, 1.0));
  }
  let edge = f32(min(min(c.x, c.y), min(n - 1 - c.x, n - 1 - c.y)));
  let soak = 1.0 - smoothstep(0.0, 16.0, edge);
  let keep = sim.step.y * (1.0 - soak * 0.2);
  var h = here.x + (here.x - here.y) * keep + sim.frame.w * (l.x + r.x + u.x + d.x - 4.0 * here.x);
  h *= 1.0 - soak * 0.05;
  var foam = mix(here.z, (l.z + r.z + u.z + d.z) * 0.25, 0.12) * sim.step.z;
  // What stamps move the water by this step: it is moved, not thrown, so it goes into the height a step
  // before as well, and the water then flows back from the shape it was pushed into as waves.
  var pushed = 0.0;
  for (var i = 0; i < sim.shift.z; i++) {
    let stamp = stamps[i];
    var moved = stamp.motion.xy * sim.step.x;
    if (stamp.motion.z <= 0.5) {
      moved -= carried * sim.step.x;
    }
    let sd = outline(stamp, world).x;
    if (sd > rimOut(stamp) + length(moved)) {
      continue;
    }
    let inside = 1.0 - smoothstep(-0.25, 0.35, sd);
    if (stamp.motion.z > 0.5) {
      if (sim.step.w > 0.5) {
        pushed += displaced(stamp, world, stamp.shape.z);
        foam += stamp.shape.w * inside;
      }
      continue;
    }
    // Where it is now, less where it was a step ago: its outline then is this one moved back, and its
    // depth then is that much less of the way from the last frame's to this one's.
    let before = mix(stamp.motion.w, stamp.shape.z, sim.share.x);
    let after = mix(stamp.motion.w, stamp.shape.z, sim.share.y);
    pushed += displaced(stamp, world, after) - displaced(stamp, world + moved, before);
    foam += stamp.shape.w * inside * sim.step.x;
  }
  let slope = vec2f(r.x - l.x, d.x - u.x) / (2.0 * sim.frame.z);
  foam += smoothstep(0.25, 0.7, length(slope)) * 2.5 * sim.step.x;
  h += pushed;
  // Breaking: the motion of steep water is partly spent, and the height eased under BREAK.
  let breaking = smoothstep(BREAK_STEEP.x, BREAK_STEEP.y, length(slope));
  h = mix(h, here.x + pushed, breaking * 0.3);
  h = BREAK * tanh(h / BREAK);
  foam += breaking * 3.0 * sim.step.x;
  textureStore(after, c, vec4f(clamp(h, -1.5, 1.5), clamp(here.x + pushed, -1.5, 1.5), clamp(foam, 0.0, 1.5), 0.0));
}

// What the water's shader reads: height, slope (dh/dx, dh/dz) and foam.
@compute @workgroup_size(8, 8)
fn surface(@builtin(global_invocation_id) id: vec3u) {
  let n = sim.shift.w;
  let c = vec2i(id.xy);
  if (c.x >= n || c.y >= n) {
    return;
  }
  let top = vec2i(n - 1);
  let h = textureLoad(before, c, 0);
  let l = textureLoad(before, max(c - vec2i(1, 0), vec2i(0)), 0).x;
  let r = textureLoad(before, min(c + vec2i(1, 0), top), 0).x;
  let u = textureLoad(before, max(c - vec2i(0, 1), vec2i(0)), 0).x;
  let d = textureLoad(before, min(c + vec2i(0, 1), top), 0).x;
  let slope = vec2f(r - l, d - u) / (2.0 * sim.frame.z);
  textureStore(after, c, vec4f(h.x, slope, min(h.z, 1.0)));
}
`

/**
 * Water that moves when something moves through it, worked out on the GPU over a square about the
 * camera: a damped wave equation on a grid of heights, stepped at a fixed rate. Hulls and bodies moving
 * through it (`stamp`) push the water aside as they go, so one moving fast raises a bow wave and a V of
 * waves behind it at the angle its speed and the waves' make; something landing in it (`splash`) sends
 * out rings. Foam builds where the water is churned or steep, spreads and fades. In a river the water
 * is carried along by the current (`setFlow`), and what stands still in it makes waves as what moves
 * through still water does.
 *
 * Each frame's result is `texture` (height, slope x, slope z, foam, in metres), covering `area`, for a
 * water material to read. The grid stays put in whole texels as the camera moves, so nothing crawls.
 */
export class WaveField extends Effect {
  readonly grid: number
  readonly texel: number
  /** The map the water reads: one placeholder texel until the GPU has drawn the first. */
  readonly texture = new Texture({ data: new Uint8Array(4), width: 1, height: 1 }, { srgb: false, filter: 'linear', wrap: 'clamp', mipmaps: false })
  /** Where the map's corner is (x, z) and how wide it is, metres. */
  readonly area: [number, number, number, number]
  private readonly k: number
  private readonly damping: number
  private readonly foamFade: number
  /** Stamps taken since the last `update`, and the ones it set aside for the frame being drawn. */
  private readonly stamps = new Float32Array(MOST_STAMPS * STAMP_FLOATS)
  private stampCount = 0
  private readonly ready = new Float32Array(MOST_STAMPS * STAMP_FLOATS)
  private readyCount = 0
  /** Steps owed to the next `prepare`, the time not yet stepped, and how far the field moved since the last step (texels). */
  private steps = 0
  private owed = 0
  private readonly shift: [number, number] = [0, 0]
  private quiet = Infinity
  private gpu: WaveGpu | null = null
  /** The current's map (`setFlow`), where it lies and what a byte's step is worth; null with none. */
  private flow: { bytes: Uint8Array<ArrayBuffer>; area: [number, number, number]; step: number } | null = null
  private flowChanged = false

  constructor(params: WaveFieldParams = {}) {
    super()
    this.grid = params.grid ?? 256
    this.texel = params.texel ?? 0.375
    const ratio = (params.speed ?? 6) / RATE / this.texel
    // Past 1/sqrt(2) the wave equation blows up: ripples may not cross a texel in a step.
    if (ratio > 0.7) throw new Error('WaveField: the waves outrun the grid; lower the speed or coarsen the texel')
    this.k = ratio * ratio
    this.damping = params.damping ?? 0.5
    this.foamFade = params.foamFade ?? 1.6
    this.area = [0, 0, this.grid * this.texel, 0]
    // Nothing for the culler to measure: it is in the scene only to be prepared each frame.
    this.renderOrder = -1
  }

  /** How many stamps have been taken since the last `update`. */
  get stamped() {
    return this.stampCount
  }

  /** How many stamps the next `prepare` presses in. */
  get pressing() {
    return this.readyCount
  }

  /** How many steps the next `prepare` will take. */
  get pending() {
    return this.steps
  }

  /**
   * The current that carries the water: `bytes` is `FLOW_GRID` × `FLOW_GRID` texels of rgba, row by row
   * from the corner `(x0, z0)` over a square `side` metres wide, the velocity's x in red and z in green,
   * each `128 + v / step` (so `step` is the m/s a count stands for). Null for still water. The map need
   * not cover the field: water off it is still.
   */
  setFlow(bytes: Uint8Array<ArrayBuffer> | null, x0 = 0, z0 = 0, side = 1, step = 0) {
    this.flow = bytes ? { bytes, area: [x0, z0, side], step } : null
    this.flowChanged = true
  }

  /** Moves `stamp` through the water for this frame; past `MOST_STAMPS` it is dropped. */
  stamp(stamp: WaveStamp) {
    if (this.stampCount >= MOST_STAMPS) return
    const s = this.stamps
    const o = this.stampCount * STAMP_FLOATS
    const length = Math.hypot(stamp.ahead[0], stamp.ahead[1]) || 1
    s[o] = stamp.x
    s[o + 1] = stamp.z
    s[o + 2] = stamp.ahead[0] / length
    s[o + 3] = stamp.ahead[1] / length
    s[o + 4] = Math.max(0.05, stamp.halfWidth)
    s[o + 5] = Math.max(0.05, stamp.halfLength)
    s[o + 6] = stamp.depth
    s[o + 7] = stamp.churn
    s[o + 8] = stamp.velocity[0]
    s[o + 9] = stamp.velocity[1]
    s[o + 10] = 0
    s[o + 11] = stamp.was ?? stamp.depth
    this.stampCount++
  }

  /**
   * Something landing in the water at `(x, z)`: it knocks a hole of `radius` metres, `depth` deep
   * (negative heaps the water up), which runs out as rings, and leaves `foam` (0 to 1) there.
   */
  splash(x: number, z: number, radius: number, depth: number, foam: number) {
    if (this.stampCount >= MOST_STAMPS) return
    this.stamp({ x, z, ahead: [0, 1], halfWidth: radius, halfLength: radius, depth, velocity: [0, 0], churn: foam })
    this.stamps[(this.stampCount - 1) * STAMP_FLOATS + 10] = 1
  }

  /**
   * Lets `dt` seconds pass with the camera at `(x, z)`: works out how many steps the next frame takes
   * and, when it takes any, moves the field to stay about the camera. `area` then describes the map
   * that frame will draw. Once nothing has pressed in for a while the water has settled and it rests.
   */
  update(dt: number, x: number, z: number) {
    this.quiet = this.stampCount > 0 ? 0 : this.quiet + dt
    this.owed = Math.min(this.owed + dt, MOST_STEPS / RATE)
    this.steps = Math.floor(this.owed * RATE)
    this.owed -= this.steps / RATE
    if (this.quiet > SETTLE) this.steps = 0
    this.readyCount = 0
    // A frame that takes no step keeps its splashes for the next that does, so none is missed; its hulls
    // are stamped again next frame where they are then, and kept twice they would push twice.
    if (this.steps === 0) {
      let kept = 0
      for (let i = 0; i < this.stampCount; i++) {
        if (this.stamps[i * STAMP_FLOATS + 10] !== 1) continue
        this.stamps.copyWithin(kept * STAMP_FLOATS, i * STAMP_FLOATS, (i + 1) * STAMP_FLOATS)
        kept++
      }
      this.stampCount = kept
      return
    }
    // These stamps go to the GPU; what is stamped from here on is the next frame's.
    this.ready.set(this.stamps.subarray(0, this.stampCount * STAMP_FLOATS))
    this.readyCount = this.stampCount
    this.stampCount = 0
    const size = this.grid * this.texel
    const x0 = Math.floor(x / this.texel) * this.texel - size / 2
    const z0 = Math.floor(z / this.texel) * this.texel - size / 2
    this.shift[0] += Math.round((x0 - this.area[0]) / this.texel)
    this.shift[1] += Math.round((z0 - this.area[1]) / this.texel)
    this.area[0] = x0
    this.area[1] = z0
  }

  /** The placeholder's GPU copy that the map replaced, to destroy once the frame that may hold it is submitted. */
  private stale: GPUTexture | null = null

  prepare(ctx: EffectContext, encoder: GPUCommandEncoder) {
    if (this.gpu && this.gpu.device !== ctx.device) this.gpu = null
    if (!this.gpu) {
      this.flowChanged = true
      this.gpu = new WaveGpu(ctx.device, this.grid)
      // Hand the material the GPU's map in place of the placeholder; the new version makes it bind again.
      // The placeholder's own GPU copy, if the material has bound it, is in this frame's bind group: the
      // water was listed before this ran. Destroyed now, the whole frame would be refused, and with it
      // what the frame makes once (the sky's tables, the refined mesh). It goes after the submit.
      const old = this.texture.gpu as TextureSlot | undefined
      if (old?.device === ctx.device) this.stale = old.texture
      this.texture.version++
      this.texture.gpu = { texture: this.gpu.map, view: this.gpu.map.createView(), version: this.texture.version, device: ctx.device } satisfies TextureSlot
    }
    const gpu = this.gpu
    if (this.steps > 0) {
      const device = ctx.device
      if (this.readyCount) device.queue.writeBuffer(gpu.stamps, 0, this.ready, 0, this.readyCount * STAMP_FLOATS)
      if (this.flowChanged && this.flow) device.queue.writeTexture({ texture: gpu.flow }, this.flow.bytes, { bytesPerRow: FLOW_GRID * 4 }, [FLOW_GRID, FLOW_GRID])
      this.flowChanged = false
      const dt = 1 / RATE
      const keep = 1 - this.damping * dt
      const foamKeep = Math.exp(-dt / this.foamFade)
      for (let i = 0; i < this.steps; i++) {
        const f = gpu.uniform
        const w = new Int32Array(f.buffer)
        f[0] = this.area[0]
        f[1] = this.area[1]
        f[2] = this.texel
        f[3] = this.k
        // The field moves once, in the frame's first step.
        w[4] = i === 0 ? this.shift[0] : 0
        w[5] = i === 0 ? this.shift[1] : 0
        w[6] = this.readyCount
        w[7] = this.grid
        f[8] = dt
        f[9] = keep
        f[10] = foamKeep
        f[11] = i === 0 ? 1 : 0
        f[12] = i / this.steps
        f[13] = (i + 1) / this.steps
        f[16] = this.flow?.area[0] ?? 0
        f[17] = this.flow?.area[1] ?? 0
        f[18] = this.flow ? 1 / this.flow.area[2] : 0
        f[19] = this.flow?.step ?? 0
        device.queue.writeBuffer(gpu.uniforms, i * UNIFORM_STRIDE, f)
      }
      this.shift[0] = 0
      this.shift[1] = 0
      const groups = Math.ceil(this.grid / 8)
      const pass = encoder.beginComputePass({ label: 'waves' })
      pass.setPipeline(gpu.advance)
      for (let i = 0; i < this.steps; i++) {
        pass.setBindGroup(0, gpu.stepGroups[gpu.current]!, [i * UNIFORM_STRIDE])
        pass.dispatchWorkgroups(groups, groups)
        gpu.current = 1 - gpu.current
      }
      pass.setPipeline(gpu.surface)
      pass.setBindGroup(0, gpu.surfaceGroups[gpu.current]!, [0])
      pass.dispatchWorkgroups(groups, groups)
      pass.end()
      this.steps = 0
    }
  }

  override get idle() {
    return true
  }

  override afterSubmit() {
    this.stale?.destroy()
    this.stale = null
  }

  draw(): number {
    return 0
  }
}

/** The GPU half of a WaveField, made on first use: two states stepped one into the other, and the map. */
class WaveGpu {
  readonly states: GPUTexture[]
  readonly map: GPUTexture
  readonly stamps: GPUBuffer
  readonly uniforms: GPUBuffer
  readonly uniform = new Float32Array(SIM_FLOATS)
  /** The current's map (`WaveField.setFlow`). */
  readonly flow: GPUTexture
  readonly advance: GPUComputePipeline
  readonly surface: GPUComputePipeline
  /** Step from state i into the other; read state i into the map. */
  readonly stepGroups: GPUBindGroup[]
  readonly surfaceGroups: GPUBindGroup[]
  /** Which state holds the water now. */
  current = 0

  constructor(readonly device: GPUDevice, grid: number) {
    const usage = GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.STORAGE_BINDING
    const texture = (label: string, more = 0) => device.createTexture({ label, size: [grid, grid], format: 'rgba16float', usage: usage | more })
    this.states = [texture('waves a'), texture('waves b')]
    // Copied from only to be looked at (tests, debugging).
    this.map = texture('waves map', GPUTextureUsage.COPY_SRC)
    this.stamps = device.createBuffer({ label: 'wave stamps', size: MOST_STAMPS * STAMP_FLOATS * 4, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST })
    this.uniforms = device.createBuffer({ label: 'waves', size: MOST_STEPS * UNIFORM_STRIDE, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
    this.flow = device.createTexture({ label: 'waves flow', size: [FLOW_GRID, FLOW_GRID], format: 'rgba8unorm', usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST })
    const flowSampler = device.createSampler({ label: 'waves flow', magFilter: 'linear', minFilter: 'linear' })
    const compute = GPUShaderStage.COMPUTE
    const layout = device.createBindGroupLayout({
      label: 'waves',
      entries: [
        { binding: 0, visibility: compute, buffer: { type: 'uniform', hasDynamicOffset: true, minBindingSize: SIM_FLOATS * 4 } },
        { binding: 1, visibility: compute, buffer: { type: 'read-only-storage' } },
        { binding: 2, visibility: compute, texture: { sampleType: 'unfilterable-float' } },
        { binding: 3, visibility: compute, storageTexture: { access: 'write-only', format: 'rgba16float' } },
        { binding: 4, visibility: compute, texture: { sampleType: 'float' } },
        { binding: 5, visibility: compute, sampler: { type: 'filtering' } },
      ],
    })
    const module = device.createShaderModule({ label: 'waves', code: WAVES_WGSL })
    const pipelineLayout = device.createPipelineLayout({ bindGroupLayouts: [layout] })
    this.advance = device.createComputePipeline({ label: 'waves advance', layout: pipelineLayout, compute: { module, entryPoint: 'advance' } })
    this.surface = device.createComputePipeline({ label: 'waves surface', layout: pipelineLayout, compute: { module, entryPoint: 'surface' } })
    const group = (from: GPUTexture, to: GPUTexture) => device.createBindGroup({
      label: 'waves',
      layout,
      entries: [
        { binding: 0, resource: { buffer: this.uniforms, size: SIM_FLOATS * 4 } },
        { binding: 1, resource: { buffer: this.stamps } },
        { binding: 2, resource: from.createView() },
        { binding: 3, resource: to.createView() },
        { binding: 4, resource: this.flow.createView() },
        { binding: 5, resource: flowSampler },
      ],
    })
    this.stepGroups = [group(this.states[0]!, this.states[1]!), group(this.states[1]!, this.states[0]!)]
    this.surfaceGroups = [group(this.states[0]!, this.map), group(this.states[1]!, this.map)]
  }
}
