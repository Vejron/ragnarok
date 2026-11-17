import { COMMON_WGSL } from '../materials/common.wgsl'
import { LIGHT_CELL_MAX, LIGHT_GRID } from '../scene/lightgrid'

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
  // stretch (metres of length per m/s of speed), HDR glow multiplier, turbulence (m/s of swirl), the share of the wind that carries it.
  look: vec4f,
  // The light the frame's point lights throw on it (linear rgb), unused.
  lit: vec4f,
}

const PF_BOUNCE = 1u;
const PF_ADDITIVE = 2u;
const PF_NO_FOG = 4u;
const PF_PUFF = 8u;
const PF_DROP = 16u;
const PF_GLARE = 32u;
const PF_HOLD = 64u;
const PF_SCATTER = 128u;
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
  // rgb, and the share of the wind that carries it.
  colorA: vec4f,
  colorB: vec4f,
  // colour at death, HDR glow multiplier.
  g: vec4f,
  // spin min, spin max (radians per second), stretch, turbulence (m/s).
  h: vec4f,
}

struct Sim {
  dt: f32,
  commandCount: u32,
  spawnCount: u32,
  capacity: u32,
  // ground origin x, origin z, cell size, on (0 or 1).
  ground: vec4f,
  // The wind (m/s), and the seconds simulated so far.
  wind: vec4f,
  // The light grid's corner (world x, z), 1 / its cell size, the number of lights listed.
  lights: vec4f,
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
// The frame's point lights and their grid over the ground (scene/lightgrid.ts), as the scene shaders read them.
@group(0) @binding(8) var<storage, read> lightRecords: array<vec4f>;
@group(0) @binding(9) var<storage, read> lightCells: array<u32>;

const LIGHT_GRID = ${LIGHT_GRID}i;
const LIGHT_CELL_MAX = ${LIGHT_CELL_MAX}u;

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
  p.look = vec4f(c.h.z, c.g.w, c.h.w, c.colorA.w);
  p.lit = vec4f(0.0);
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

// The air's own churning at a point, a unit or so in size: two sizes of eddy, each a flow with no
// divergence (so smoke carried by it rolls and folds without bunching up), drifting as time passes.
fn swirl(p: vec3f, time: f32) -> vec3f {
  let a = p * 1.7 + vec3f(0.31, 0.47, 0.23) * time;
  let b = p * 4.3 + vec3f(0.9, 1.3, 0.7) * time;
  // Each part is free of its own axis, which is what keeps the flow from gathering or thinning.
  return vec3f(sin(a.z) + cos(a.y), sin(a.x) + cos(a.z), sin(a.y) + cos(a.x)) * 0.5
    + vec3f(cos(b.z) - sin(b.y), cos(b.x) - sin(b.z), cos(b.y) - sin(b.x)) * 0.25;
}

// The light the frame's point and spot lights throw on smoke at \`world\`: all of each, as smoke
// scatters light from any side.
fn lightAt(world: vec3f) -> vec3f {
  if (sim.lights.w < 0.5) {
    return vec3f(0.0);
  }
  let cell = vec2i(floor((world.xz - sim.lights.xy) * sim.lights.z));
  if (any(cell < vec2i(0)) || any(cell >= vec2i(LIGHT_GRID))) {
    return vec3f(0.0);
  }
  let packed = lightCells[u32(cell.y * LIGHT_GRID + cell.x)];
  let first = u32(LIGHT_GRID * LIGHT_GRID) + (packed >> 8u);
  let count = min(packed & 0xffu, LIGHT_CELL_MAX);
  var sum = vec3f(0.0);
  for (var k = 0u; k < count; k++) {
    let i = lightCells[first + k] * 3u;
    let at = lightRecords[i];
    let to = at.xyz - world;
    let d = length(to);
    if (d < at.w) {
      let color = lightRecords[i + 1u];
      let spot = lightRecords[i + 2u];
      let fall = 1.0 - d / at.w;
      let cone = smoothstep(color.w, spot.w, dot(-to / max(d, 1e-3), spot.xyz));
      sum += color.rgb * fall * fall * cone;
    }
  }
  return sum;
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
  // Drag is a share per second of its speed through the air, which moves with the wind as far as this
  // particle is carried by it; then gravity. The air's churning carries it without changing its speed.
  let air = sim.wind.xyz * p.look.w;
  let keep = max(0.0, 1.0 - p.size.w * dt);
  var v = air + (p.velLife.xyz - air) * keep;
  v.y -= p.size.z * dt;
  var pos = p.posAge.xyz + v * dt;
  if (p.look.z > 0.0) {
    pos += swirl(pos, sim.wind.w) * p.look.z * dt;
  }
  if ((p.state.y & PF_BOUNCE) != 0u) {
    let ground = groundHeight(pos.x, pos.z) + 0.02;
    if (pos.y < ground) {
      pos.y = ground;
      v = vec3f(v.x * 0.5, abs(v.y) * 0.3, v.z * 0.5);
    }
  }
  particles[i].posAge = vec4f(pos, age);
  particles[i].velLife = vec4f(v, p.velLife.w);
  if ((p.state.y & (PF_ADDITIVE | PF_PUFF)) == PF_PUFF) {
    particles[i].lit = vec4f(lightAt(pos), 0.0);
  }
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
  // The point lights' light on it, and the way to the sun in the view's frame with how much of it shades the puff.
  @location(7) lit: vec3f,
  @location(8) @interpolate(flat) sun: vec4f,
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
  if ((p.state.y & PF_HOLD) != 0u) {
    // Smoke that hangs: there within a tenth of a second, as thick as it came for the first half of its
    // life, and thinning away over the second.
    fade = min(1.0, p.posAge.w * 10.0) * (1.0 - smoothstep(0.5, 1.0, t));
  }
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
  out.lit = p.lit.rgb;
  // By night the ground light has already darkened the smoke, and there is no side to light.
  out.sun = vec4f((frame.view * vec4f(frame.sunDir.xyz, 0.0)).xyz, clamp(frame.sunDir.y * 5.0, 0.0, 1.0));
  return out;
}

// How much of a point light's strength shows on the smoke it reaches.
const LIT = 0.25;

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
  var shade = vec3f(1.0);
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
    if ((in.flags & PF_ADDITIVE) == 0u) {
      // Smoke has a body: a ball's normal over the puff, lumpy with its noise, lit on the side toward the
      // sun and dark away from it, with its thin rim bright when the sun is behind it.
      let s = vec2f(q.x * in.rot.x - q.y * in.rot.y, q.x * in.rot.y + q.y * in.rot.x) * 2.0;
      let nz = sqrt(max(1.0 - dot(s, s), 0.0));
      let toward = dot(vec3f(s, nz), in.sun.xyz) + (n - 0.5) * 0.5;
      let rim = (1.0 - nz) * (1.0 - nz) * max(-in.sun.z, 0.0);
      shade = vec3f(mix(1.0, 0.5 + 0.7 * clamp(toward * 0.5 + 0.5, 0.0, 1.0) + rim * 0.5, in.sun.w)) + in.lit * LIT;
    }
  }
  var shine = 0.0;
  if ((in.flags & PF_DROP) != 0u) {
    // A bead of liquid: a crisp edge and a glint up toward the light.
    let q = in.uv - 0.5;
    edge = 1.0 - smoothstep(0.36, 0.5, length(q));
    shine = (1.0 - smoothstep(0.0, 0.16, length(q - vec2f(-0.12, 0.14)))) * 0.6;
  }
  if ((in.flags & PF_GLARE) != 0u) {
    // Light: the cube of the distance from its edge, so no edge is ever seen.
    let g = max(1.0 - length(in.uv - 0.5) * 2.0, 0.0);
    edge = g * g * g;
  }
  var alpha = in.color.a * edge;
  if (alpha < 0.01) {
    discard;
  }
  // Soft particles: fade out where the billboard nears what is behind it instead of cutting into it.
  ${sceneDepth}
  alpha *= clamp((scene - in.viewDepth) / max(in.size * 0.5, 0.05), 0.0, 1.0);
  if ((in.flags & PF_SCATTER) != 0u) {
    // Water scatters sunlight forward (Henyey-Greenstein, g = 0.5, times 4 pi): about 0.5 seen from the
    // side, 6 looking into the sun, so spray and mist blaze against the light and drops glint in it.
    let toward = -in.sun.z;
    let phase = 0.75 / pow(1.25 - toward, 1.5);
    shine = shine * (1.0 + phase * 0.5) + phase * 0.25 * in.sun.w;
  }
  var rgb = in.color.rgb * shade + in.lit * LIT * 0.08 + frame.sunColor.rgb * shine * 0.15;
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
