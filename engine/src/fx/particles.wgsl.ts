import { COMMON_WGSL } from '../materials/common.wgsl'

/** Per-particle state, shared by the compute and render shaders. */
const PARTICLE_STRUCT = /* wgsl */ `
struct Particle {
  // xyz position, w age in seconds.
  posAge: vec4f,
  // xyz velocity, w lifetime in seconds.
  velLife: vec4f,
  // linear rgb, opacity at birth.
  color: vec4f,
  // size at birth, size at death, gravity, drag.
  size: vec4f,
  // alive (0 or 1), flags (PF_*), unused, unused.
  state: vec4u,
}

const PF_BOUNCE = 1u;
const PF_ADDITIVE = 2u;
const PF_NO_FOG = 4u;
`

/**
 * Emission and simulation. Emit commands are expanded one thread per new particle: each finds its
 * command by binary search over the commands' first-particle offsets, pops a free slot off the dead
 * stack and rolls its own random values from the command's seed. Simulation is one thread per slot:
 * live particles integrate and are appended to their blend class's draw list; expired ones are pushed
 * back onto the dead stack.
 */
export const PARTICLE_COMPUTE_WGSL = /* wgsl */ `
${PARTICLE_STRUCT}

struct Command {
  // position, random radius around it.
  a: vec4f,
  // velocity, random-direction speed up to w.
  b: vec4f,
  // per-axis velocity jitter (±), drag.
  c: vec4f,
  // life min, life max, size at birth, size at death.
  d: vec4f,
  // size scale min, size scale max, opacity, gravity.
  e: vec4f,
  // count, first particle index, seed, flags.
  f: vec4u,
  colorA: vec4f,
  colorB: vec4f,
}

struct Sim {
  dt: f32,
  commandCount: u32,
  spawnCount: u32,
  capacity: u32,
  // ground origin x, origin z, cell size, on (0 or 1).
  ground: vec4f,
}

struct Counters {
  dead: atomic<i32>,
}

@group(0) @binding(0) var<uniform> sim: Sim;
@group(0) @binding(1) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(2) var<storage, read_write> deadList: array<u32>;
@group(0) @binding(3) var<storage, read_write> counters: Counters;
@group(0) @binding(4) var<storage, read_write> args: array<atomic<u32>, 8>;
@group(0) @binding(5) var<storage, read_write> alive: array<u32>;
@group(0) @binding(6) var<storage, read> commands: array<Command>;
@group(0) @binding(7) var groundHeights: texture_2d<f32>;

fn pcg(v: u32) -> u32 {
  let state = v * 747796405u + 2891336453u;
  let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
  return (word >> 22u) ^ word;
}

fn random(seed: u32, k: u32, n: u32) -> f32 {
  return f32(pcg(seed ^ pcg(k * 7919u + n * 104729u))) / 4294967295.0;
}

fn unitVector(seed: u32, k: u32, n: u32) -> vec3f {
  let z = random(seed, k, n) * 2.0 - 1.0;
  let a = random(seed, k, n + 1u) * 6.2831853;
  let r = sqrt(max(0.0, 1.0 - z * z));
  return vec3f(r * cos(a), z, r * sin(a));
}

@compute @workgroup_size(64)
fn emit(@builtin(global_invocation_id) id: vec3u) {
  let i = id.x;
  if (i >= sim.spawnCount) {
    return;
  }
  var lo = 0u;
  var hi = sim.commandCount - 1u;
  while (lo < hi) {
    let mid = (lo + hi + 1u) / 2u;
    if (commands[mid].f.y <= i) {
      lo = mid;
    } else {
      hi = mid - 1u;
    }
  }
  let c = commands[lo];
  let k = i - c.f.y;
  let top = atomicSub(&counters.dead, 1);
  if (top <= 0) {
    // Pool full: this particle is dropped.
    atomicAdd(&counters.dead, 1);
    return;
  }
  let slot = deadList[u32(top - 1)];
  let seed = c.f.z;
  let radius = c.a.w * pow(random(seed, k, 0u), 1.0 / 3.0);
  let position = c.a.xyz + unitVector(seed, k, 1u) * radius;
  let jitter = (vec3f(random(seed, k, 3u), random(seed, k, 4u), random(seed, k, 5u)) * 2.0 - 1.0) * c.c.xyz;
  let velocity = c.b.xyz + jitter + unitVector(seed, k, 6u) * c.b.w * random(seed, k, 8u);
  let life = mix(c.d.x, c.d.y, random(seed, k, 9u));
  let scale = mix(c.e.x, c.e.y, random(seed, k, 10u));
  let color = mix(c.colorA.rgb, c.colorB.rgb, random(seed, k, 11u));
  var p: Particle;
  p.posAge = vec4f(position, 0.0);
  p.velLife = vec4f(velocity, max(life, 0.001));
  p.color = vec4f(color, c.e.z);
  p.size = vec4f(c.d.z * scale, c.d.w * scale, c.e.w, c.c.w);
  p.state = vec4u(1u, c.f.w, 0u, 0u);
  particles[slot] = p;
}

fn groundHeight(x: f32, z: f32) -> f32 {
  if (sim.ground.w == 0.0) {
    return -1e9;
  }
  let size = vec2f(textureDimensions(groundHeights));
  let g = (vec2f(x, z) - sim.ground.xy) / sim.ground.z;
  if (any(g < vec2f(0.0)) || any(g > size - 1.0)) {
    return -1e9;
  }
  let i = vec2u(floor(g));
  let f = fract(g);
  let j = min(i + 1u, vec2u(size - 1.0));
  let h00 = textureLoad(groundHeights, i, 0).r;
  let h10 = textureLoad(groundHeights, vec2u(j.x, i.y), 0).r;
  let h01 = textureLoad(groundHeights, vec2u(i.x, j.y), 0).r;
  let h11 = textureLoad(groundHeights, j, 0).r;
  return mix(mix(h00, h10, f.x), mix(h01, h11, f.x), f.y);
}

@compute @workgroup_size(64)
fn simulate(@builtin(global_invocation_id) id: vec3u) {
  let i = id.x;
  if (i >= sim.capacity) {
    return;
  }
  var p = particles[i];
  if (p.state.x == 0u) {
    return;
  }
  let dt = sim.dt;
  let age = p.posAge.w + dt;
  if (age >= p.velLife.w) {
    particles[i].state.x = 0u;
    let d = atomicAdd(&counters.dead, 1);
    deadList[u32(d)] = i;
    return;
  }
  // The same integration as the CPU pool it replaces: drag as a share of speed per second, then gravity.
  let keep = max(0.0, 1.0 - p.size.w * dt);
  var v = p.velLife.xyz * keep;
  v.y -= p.size.z * dt;
  var pos = p.posAge.xyz + v * dt;
  if ((p.state.y & PF_BOUNCE) != 0u) {
    let ground = groundHeight(pos.x, pos.z) + 0.02;
    if (pos.y < ground) {
      pos.y = ground;
      v = vec3f(v.x * 0.5, abs(v.y) * 0.3, v.z * 0.5);
    }
  }
  particles[i].posAge = vec4f(pos, age);
  particles[i].velLife = vec4f(v, p.velLife.w);
  let blend = select(0u, 1u, (p.state.y & PF_ADDITIVE) != 0u);
  let n = atomicAdd(&args[blend * 4u + 1u], 1u);
  alive[blend * sim.capacity + n] = i;
}
`

/** Billboards built from the particle buffer: no vertex attributes, six vertices per particle. */
export function particleRenderWgsl(multisampled: boolean) {
  return /* wgsl */ `
${COMMON_WGSL}
${PARTICLE_STRUCT}

@group(1) @binding(0) var<storage, read> particles: array<Particle>;
// This blend class's live particles (the binding starts at the class's part of the list).
@group(1) @binding(1) var<storage, read> alive: array<u32>;
@group(1) @binding(2) var sceneDepth: ${multisampled ? 'texture_depth_multisampled_2d' : 'texture_depth_2d'};

struct Out {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) color: vec4f,
  @location(2) viewDepth: f32,
  @location(3) @interpolate(flat) flags: u32,
  @location(4) size: f32,
}

@vertex
fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> Out {
  let p = particles[alive[instance]];
  var corners = array<vec2f, 6>(vec2f(-0.5, -0.5), vec2f(0.5, -0.5), vec2f(0.5, 0.5), vec2f(-0.5, -0.5), vec2f(0.5, 0.5), vec2f(-0.5, 0.5));
  let corner = corners[v];
  let t = p.posAge.w / p.velLife.w;
  let size = mix(p.size.x, p.size.y, t);
  // Camera right and up are the view matrix's first two rows.
  let right = vec3f(frame.view[0].x, frame.view[1].x, frame.view[2].x);
  let up = vec3f(frame.view[0].y, frame.view[1].y, frame.view[2].y);
  let world = p.posAge.xyz + (right * corner.x + up * corner.y) * size;
  var out: Out;
  out.clip = frame.viewProj * vec4f(world, 1.0);
  out.uv = corner + 0.5;
  // Fades in over the first tenth of its life and out over the rest.
  out.color = vec4f(p.color.rgb, p.color.a * min(1.0, t * 10.0) * (1.0 - t));
  out.viewDepth = -(frame.view * vec4f(p.posAge.xyz, 1.0)).z;
  out.flags = p.state.y;
  out.size = size;
  return out;
}

// View distance of a reversed-depth buffer value.
fn linearDepth(d: f32) -> f32 {
  let near = frame.clip.x;
  let far = frame.clip.y;
  return near * far / (d * (far - near) + near);
}

@fragment
fn fs(in: Out) -> @location(0) vec4f {
  let edge = smoothstep(0.5, 0.15, length(in.uv - 0.5));
  var alpha = in.color.a * edge;
  if (alpha < 0.01) {
    discard;
  }
  // Soft particles: fade out where the billboard nears what is behind it instead of cutting into it.
  let scene = linearDepth(textureLoad(sceneDepth, vec2i(in.clip.xy), 0));
  alpha *= clamp((scene - in.viewDepth) / max(in.size * 0.5, 0.05), 0.0, 1.0);
  var rgb = in.color.rgb;
  if ((in.flags & PF_NO_FOG) == 0u && frame.fog.z > 0.0) {
    let f = smoothstep(frame.fog.x, frame.fog.y, in.viewDepth);
    if ((in.flags & PF_ADDITIVE) != 0u) {
      // Light added through fog fades away rather than turning fog-coloured.
      alpha *= 1.0 - f;
    } else {
      rgb = mix(rgb, frame.fogColor.rgb, f);
    }
  }
  return vec4f(rgb, alpha);
}
`
}
