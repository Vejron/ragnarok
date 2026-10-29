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
  // alive (0 or 1), flags (PF_*), spin in radians per second (f32 bits), rotation at birth (f32 bits).
  state: vec4u,
  // linear rgb at death, unused.
  colorEnd: vec4f,
  // stretch (metres of length per m/s of speed), HDR glow multiplier, unused, unused.
  look: vec4f,
}

const PF_BOUNCE = 1u;
const PF_ADDITIVE = 2u;
const PF_NO_FOG = 4u;
const PF_PUFF = 8u;
const PF_DROP = 16u;
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
  // colour at death, HDR glow multiplier.
  g: vec4f,
  // spin min, spin max (radians per second), stretch, unused.
  h: vec4f,
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
  let spin = mix(c.h.x, c.h.y, random(seed, k, 12u));
  let rotation = random(seed, k, 13u) * 6.2831853;
  p.state = vec4u(1u, c.f.w, bitcast<u32>(spin), bitcast<u32>(rotation));
  p.colorEnd = vec4f(c.g.rgb, 0.0);
  p.look = vec4f(c.h.z, c.g.w, 0.0, 0.0);
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
export function particleRenderWgsl(multisampled: boolean, half = false) {
  // At half resolution one pixel stands for four of the scene's: the nearest depth of the four, so smoke is
  // hidden a little too much at an edge rather than drawn over the thing in front of it.
  const sceneDepth = half
    ? `let c = vec2i(in.clip.xy) * 2;
  let d = max(max(textureLoad(sceneDepth, c, 0), textureLoad(sceneDepth, c + vec2i(1, 0), 0)),
    max(textureLoad(sceneDepth, c + vec2i(0, 1), 0), textureLoad(sceneDepth, c + vec2i(1, 1), 0)));
  let scene = linearDepth(d);`
    : 'let scene = linearDepth(textureLoad(sceneDepth, vec2i(in.clip.xy), 0));'
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
  @location(5) @interpolate(flat) seed: f32,
  @location(6) rot: vec2f,
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
  let angle = bitcast<f32>(p.state.w) + bitcast<f32>(p.state.z) * p.posAge.w;
  var local = corner * size;
  var rot = vec2f(cos(angle), sin(angle));
  let stretch = p.look.x;
  if (stretch > 0.0) {
    // Velocity-aligned: long along the particle's motion across the screen, size wide.
    let vv = (frame.view * vec4f(p.velLife.xyz, 0.0)).xy;
    let speed = length(p.velLife.xyz);
    let l = length(vv);
    if (l > 1e-4) {
      let axis = vv / l;
      let side = vec2f(-axis.y, axis.x);
      let len = size + stretch * speed;
      local = axis * (corner.x * len) + side * (corner.y * size);
      rot = vec2f(1.0, 0.0);
    }
  } else {
    local = vec2f(corner.x * rot.x - corner.y * rot.y, corner.x * rot.y + corner.y * rot.x) * size;
  }
  let world = p.posAge.xyz + right * local.x + up * local.y;
  var out: Out;
  out.clip = frame.viewProj * vec4f(world, 1.0);
  out.uv = corner + 0.5;
  // Fades in over the first tenth of its life and out over the rest.
  let rgb = mix(p.color.rgb, p.colorEnd.rgb, t) * p.look.y;
  // A puff that has swallowed the screen is all overdraw and no picture: fade it out as it nears doing so.
  let centre = frame.viewProj * vec4f(p.posAge.xyz, 1.0);
  let top = frame.viewProj * vec4f(p.posAge.xyz + up * size * 0.5, 1.0);
  let cover = abs(top.y - centre.y) / max(centre.w, 0.01);
  let near = 1.0 - smoothstep(0.7, 1.4, cover);
  var fade = min(1.0, t * 10.0) * (1.0 - t);
  if ((p.state.y & PF_DROP) != 0u) {
    // Liquid stays whole all the way: it is gone only where it lands, at the end of its life.
    fade = 1.0 - smoothstep(0.92, 1.0, t);
  }
  out.color = vec4f(rgb, p.color.a * fade * near);
  out.seed = f32(alive[instance] % 97u);
  out.rot = rot;
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

// A hash without sin: a puff evaluates this eight times a pixel, and sin made smoke the most expensive thing on screen.
fn hash(p: vec2f) -> f32 {
  var q = fract(vec3f(p.x, p.y, p.x) * 0.1031);
  q += dot(q, q.yzx + 33.33);
  return fract((q.x + q.y) * q.z);
}

fn noise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2f(1.0, 0.0)), u.x), mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), u.x), u.y);
}

@fragment
fn fs(in: Out) -> @location(0) vec4f {
  var edge = smoothstep(0.5, 0.15, length(in.uv - 0.5));
  if ((in.flags & PF_PUFF) != 0u) {
    // Billowy edge: noise in the particle's own (spinning) frame, so each puff is a different cloud.
    let q = in.uv - 0.5;
    let r = vec2f(q.x * in.rot.x + q.y * in.rot.y, -q.x * in.rot.y + q.y * in.rot.x);
    // Past 0.67 the noise cannot bring the edge back, so the corners of the quad skip it.
    let d = length(q);
    if (d > 0.67) {
      discard;
    }
    let n = noise(r * 7.0 + in.seed);
    edge = smoothstep(0.5, 0.12, d + (n - 0.5) * 0.34);
  }
  var shine = 0.0;
  if ((in.flags & PF_DROP) != 0u) {
    // A bead of liquid: a crisp edge and a glint up toward the light.
    let q = in.uv - 0.5;
    edge = 1.0 - smoothstep(0.36, 0.5, length(q));
    shine = (1.0 - smoothstep(0.0, 0.16, length(q - vec2f(-0.12, 0.14)))) * 0.6;
  }
  var alpha = in.color.a * edge;
  if (alpha < 0.01) {
    discard;
  }
  // Soft particles: fade out where the billboard nears what is behind it instead of cutting into it.
  ${sceneDepth}
  alpha *= clamp((scene - in.viewDepth) / max(in.size * 0.5, 0.05), 0.0, 1.0);
  var rgb = in.color.rgb + frame.sunColor.rgb * shine * 0.15;
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
