import { Color, Geometry, Group, Mesh, PANORAMA_SIZE, ShaderMaterial, Texture } from '@ragnarok/engine'
import type { TileGround } from '@/sim/terrain'
import type { TileData } from '@/world'
import type { HeightSource } from './shade'
import { waterBuffers, type WaterBuffers } from './groundmesh'
import { FLOW_BIAS, STREAM_FILL } from './watermesh'
import { GROUND_LIGHT_WGSL } from './groundlight'
import { WAVES_WGSL } from '@/sim/waves'
import { RIVER_TINT, WHITE_WATER_SPEEDS } from '@/sim/consts.generated'

/**
 * `fn wake(p: vec2f) -> vec4f`: the moved water's height at `p` (metres), its slope `(dh/dx, dh/dz)`
 * and how churned it is there (0 to 1), read from the material's `wakeMap` texture (`WaveField.texture`)
 * over its `wakeArea`. It fades out towards the map's edge.
 */
const WAKES_WGSL = /* wgsl */ `
// How far in from the refined square's edge the wakes' height fades in, metres (\`wakeLift\`).
const WAKE_EDGE = 6.0;

fn wake(p: vec2f) -> vec4f {
  let uv = (p - params.wakeArea.xy) / params.wakeArea.z;
  let edge = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
  if (edge <= 0.0) {
    return vec4f(0.0);
  }
  return textureSampleLevel(wakeMap, materialSampler, uv, 0.0) * smoothstep(0.0, 0.08, edge);
}

// How far inside the refined square \`p\` is, metres (negative outside it), while there is one.
fn inRefine(p: vec2f) -> f32 {
  if (params.refineArea.w < 0.5) {
    return -1.0;
  }
  let q = p - params.refineArea.xy;
  return min(min(q.x, params.refineArea.z - q.x), min(q.y, params.refineArea.z - q.y));
}

// The moved water's height at \`p\`, for a vertex of the refined mesh (\`WaterRefine\`, its uv's x is 1),
// whose vertices are as close as the wave field's texels: none for the tiles' own coarser vertices,
// which cannot carry it, and fading out towards the refined square's edge, so the two meet flat.
fn wakeLift(p: vec2f) -> f32 {
  if (vertexUv.x < 0.5) {
    return 0.0;
  }
  return wake(p).x * smoothstep(0.0, WAKE_EDGE, inRefine(p));
}
`

/**
 * Water: lakes and rivers are a surface mesh per tile at the importer's water level, streams are
 * ribbons draped on the ground. Both share one toy-style shader: a tint that deepens with depth,
 * soft light bands drifting over the surface, a foam line along the shore, and a flow along a
 * stream's length. A river is drawn as running water (RIVER-SURFACE.md): its ripples ride its current
 * and lie along it, it is glassy where it runs deep and broken where it runs thin, its colour is its
 * stain's, and its surface leans as the current shapes it. Depth is a vertex attribute (`level - ground`), not a depth-texture read, so the
 * look needs no extra pass. Lakes and rivers carry the waves the sim rides vehicles on (`sim/waves.ts`):
 * the vertices rise and fall with them on the server's clock, they light the surface, and their crests
 * break white. They die away towards the shore; streams and ice have none. On top, whatever moves
 * through the water stirs it (`waterwaves.ts`).
 */

/** Shore foam reaches this deep, metres. */
const FOAM_DEPTH = 0.35
/** On a stream, only this deep, metres: a fine line at the banks, where a lake's would whiten it all. */
const STREAM_FOAM_DEPTH = 0.08
/** The water fades out from this depth to the shoreline, metres: whatever of a cell lies past it is not drawn. */
const SHORE_FADE = 0.04
/** Along-stream speed of the flow, metres per second. */
const FLOW_SPEED = 1.4
/**
 * A river's ripples and foam ride its current: two copies of them, each carried downstream for this long,
 * seconds, then started over, half a cycle apart and crossfaded, so they move without smearing.
 */
const FLOW_CYCLE = 1.2
/**
 * How thick a river's carried foam lies, by how much of the surface the importer found under it (a
 * lake's or river's uv y, `TileData.foam`): from the first there is any to draw, and by the second its
 * flecks have their full white.
 */
const FOAM_COVERS = [0.02, 0.12] as const
/**
 * A river's surface is glassy where its Froude number (its speed over `√(g · depth)`, the speed of a wave
 * on water that deep) is under the first of these and broken from the second: thin fast water over a bar
 * breaks, a deep tongue at the same speed slides by smooth.
 */
const ROUGH_FROUDES = [0.2, 0.75] as const
/**
 * How much a standing wave's slope along the current gathers the foam on its face (and clears it off its
 * back), and how much less foam a rapid has on its level water, where it had it everywhere before.
 */
const WAVE_FOAM = 2.5
/** How much a finer noise breaks a river's foam into lace near the camera, against the streaks' own strength. */
const FOAM_LACE = 0.45
const WAVE_FOAM_LESS = 0.12
/** How much of a rapid's white a smooth, deep tongue of it keeps. */
const TONGUE_WHITE = 0.3
/**
 * How much light a metre of river water passes: the first of these in the colours its stain has none
 * of, the second in the stain's strongest.
 */
const RIVER_PASSES = [0.22, 0.7] as const
/** How far light goes through water to show the bed, in the water's depths: down, and back up at a slant. */
const RIVER_PATH = 2.2

/** How many of the noise's lattice cells the chop map holds a side, after which it repeats. */
const CHOP_CELLS = 64
/** The chop map's texels to a cell: enough that reading it filtered follows the noise's smooth curve. */
const CHOP_TEXELS = 8
/** The largest slope the noise has, per cell, which the chop map's green and blue hold as -1 to 1. */
const CHOP_SLOPE = 3

/**
 * The chop map's pixels: the water shader's `vnoise` at each texel's middle, over a lattice that wraps
 * at `CHOP_CELLS`, its value in red and its slope along x and y in green and blue, each from -1 to 1
 * (the slope over `CHOP_SLOPE`) in a byte. The chop is four octaves of this noise, each turned and
 * drifting its own way, so the map's repeat (580 m for the broadest, 44 m for the finest) does not show.
 */
export function chopPixels(): Uint8Array {
  const size = CHOP_CELLS * CHOP_TEXELS
  const fract = (v: number) => v - Math.floor(v)
  // The shader's \`hash2\`, of a lattice point taken round the map's edge.
  const hash = (x: number, y: number) => {
    const qx = fract((x % CHOP_CELLS) * 0.1031)
    const qy = fract((y % CHOP_CELLS) * 0.103)
    const d = qx * (qy + 33.33) + qy * (qx + 33.33)
    return fract((qx + d + qy + d) * (qx + d))
  }
  const lattice = new Float32Array(CHOP_CELLS * CHOP_CELLS)
  for (let y = 0; y < CHOP_CELLS; y++) for (let x = 0; x < CHOP_CELLS; x++) lattice[y * CHOP_CELLS + x] = hash(x, y)
  const at = (x: number, y: number) => lattice[(y % CHOP_CELLS) * CHOP_CELLS + (x % CHOP_CELLS)]!
  const byte = (v: number) => Math.max(0, Math.min(255, Math.round((v * 0.5 + 0.5) * 255)))
  const out = new Uint8Array(size * size * 4)
  for (let ty = 0; ty < size; ty++) {
    const py = (ty + 0.5) / CHOP_TEXELS
    const iy = Math.floor(py), fy = py - iy
    const uy = fy * fy * (3 - 2 * fy), dy = 6 * fy * (1 - fy)
    for (let tx = 0; tx < size; tx++) {
      const px = (tx + 0.5) / CHOP_TEXELS
      const ix = Math.floor(px), fx = px - ix
      const ux = fx * fx * (3 - 2 * fx), dx = 6 * fx * (1 - fx)
      const a = at(ix, iy), b = at(ix + 1, iy), c = at(ix, iy + 1), d = at(ix + 1, iy + 1)
      const k = a - b - c + d
      const o = (ty * size + tx) * 4
      out[o] = byte((a + (b - a) * ux + (c - a) * uy + k * ux * uy) * 2 - 1)
      out[o + 1] = byte((dx * (b - a + k * uy) * 2) / CHOP_SLOPE)
      out[o + 2] = byte((dy * (c - a + k * ux) * 2) / CHOP_SLOPE)
      out[o + 3] = 255
    }
  }
  return out
}

/** Moves each vertex of a lake or river with the waves. Its functions serve the fragment too. */
const VERTEX = /* wgsl */ `
${WAVES_WGSL}

// How much of the waves this water has: none on a stream or on ice, less towards the shore.
fn waveShare(custom: vec4f) -> f32 {
  return select(waveShore(custom.x), 0.0, custom.w > 0.5) * params.wave.y;
}

// A lake's or river's current, m/s (none on a stream, which keeps its own flow).
fn current(custom: vec4f) -> vec2f {
  return select(vec2f(custom.z, custom.w + ${FLOW_BIAS.toFixed(1)}), vec2f(0.0), custom.w > 0.5);
}

// The surge of a rapid's standing waves, as the sim rides it (none on ice). The waves themselves are in
// the mesh, part of the river's level; their surge lights the surface but does not lift it: lifting the
// vertices by waves worked out here cost 1.3 to 1.8 ms a frame at Retina size over a rapid (gpu-bench,
// RIVERS.md), for a 10 cm heave the foam hides.
fn standing(p: vec2f, custom: vec4f) -> vec3f {
  return churnWaves(p, params.wave.x, current(custom)) * params.wave.y;
}

${WAKES_WGSL}

// How much of the wakes this water shows: none on ice, less where it is too shallow to stir.
fn wakeShare(custom: vec4f) -> f32 {
  return smoothstep(0.05, 0.4, custom.x) * params.wave.y;
}

fn displace(world: vec3f, custom: vec4f, color: vec4f) -> vec3f {
  let lift = waves(world.xz, params.wave.x, custom.y).x * waveShare(custom) + wakeLift(world.xz) * wakeShare(custom);
  // The gusts are patches tens of metres across, far broader than the mesh: worked out at each vertex
  // and handed to the fragment, where they cost two noises a pixel.
  vertexExtra[0] = vec4f(gusts(world.xz, frame.fog.w), 0.0, 0.0, 0.0);
  return world + vec3f(0.0, lift, 0.0);
}
`

const WGSL = /* wgsl */ `
${GROUND_LIGHT_WGSL}

const CHOP_CELLS = ${CHOP_CELLS.toFixed(1)};
const CHOP_TEXELS = ${CHOP_TEXELS.toFixed(1)};
const CHOP_SLOPE = ${CHOP_SLOPE.toFixed(1)};

fn bands(p: vec2f, t: f32) -> f32 {
  let a = sin(p.x * 0.9 + p.y * 0.35 + t * 0.9);
  let b = sin(p.x * -0.4 + p.y * 1.1 - t * 0.7);
  let c = sin((p.x + p.y) * 0.25 + t * 0.4);
  return (a + b + 0.6 * c) / 2.6;
}

// A hash of a lattice point, 0 to 1.
fn hash2(p: vec2f) -> f32 {
  let q = fract(p * vec2f(0.1031, 0.1030));
  let r = q + dot(q, q.yx + 33.33);
  return fract((r.x + r.y) * r.x);
}

// Smooth value noise, -1 to 1, and its gradient: (value, d/dx, d/dy).
fn vnoise(p: vec2f) -> vec3f {
  let i = floor(p);
  let f = p - i;
  let u = f * f * (3.0 - 2.0 * f);
  let du = 6.0 * f * (1.0 - f);
  let a = hash2(i);
  let b = hash2(i + vec2f(1.0, 0.0));
  let c = hash2(i + vec2f(0.0, 1.0));
  let d = hash2(i + vec2f(1.0, 1.0));
  let k = a - b - c + d;
  let v = a + (b - a) * u.x + (c - a) * u.y + k * u.x * u.y;
  let g = du * vec2f(b - a + k * u.y, c - a + k * u.x);
  return vec3f(v * 2.0 - 1.0, g * 2.0);
}

// \`vnoise\` read from the chop map (\`chopMap\`, see \`chopPixels\`): the same noise over a lattice that
// repeats every CHOP_CELLS cells, in one filtered read where working it out is four hashes. \`lod\` is
// the map's mip level, so water far off reads a small, blurred level and not texels scattered over the map.
fn mappedNoise(q: vec2f, lod: f32) -> vec3f {
  let texel = textureSampleLevel(chopMap, materialSampler, q / CHOP_CELLS, lod);
  return vec3f(texel.x * 2.0 - 1.0, (texel.yz * 2.0 - 1.0) * CHOP_SLOPE);
}

// One octave of chop: noise \`scale\` cells a metre, turned by \`angle\` and drifting at \`drift\` m/s, its
// gradient in world metres. \`span\` is how many metres of water a pixel spans, for the map's mip level.
fn octave(p: vec2f, t: f32, scale: f32, angle: f32, drift: vec2f, span: f32) -> vec3f {
  let cs = vec2f(cos(angle), sin(angle));
  let turn = mat2x2f(cs.x, cs.y, -cs.y, cs.x);
  let n = mappedNoise(turn * (p + drift * t) * scale, log2(max(span * scale * CHOP_TEXELS, 1.0)));
  return vec3f(n.x, (transpose(turn) * n.yz) * scale);
}

// The wind's chop over the swell: four octaves, each turned and drifting its own way so nothing lines
// up into a pattern. Height (about -2 to 2) and its slope per metre. \`fine\` and \`mid\` (0 to 1) fade the
// small ones far off, so distant water does not shimmer.
fn chop(p: vec2f, t: f32, fine: f32, mid: f32, span: f32) -> vec3f {
  return octave(p, t, 0.11, 0.0, vec2f(0.9, 0.35), span) + finerChop(p, t, fine, mid, span);
}

// The chop's three finer octaves. The two smallest are left out where they have faded to nothing,
// which is most of a lake seen from its shore.
fn finerChop(p: vec2f, t: f32, fine: f32, mid: f32, span: f32) -> vec3f {
  var c = 0.6 * octave(p, t, 0.26, 1.1, vec2f(1.1, -0.5), span);
  if (mid > 0.0) {
    c += mid * 0.35 * octave(p, t, 0.63, 2.3, vec2f(-0.4, 1.3), span);
  }
  if (fine > 0.0) {
    c += fine * 0.2 * octave(p, t, 1.45, 3.7, vec2f(1.5, 0.8), span);
  }
  return c;
}

// A river's ripples, before the current carries them: three octaves a size down from a lake's chop,
// barely drifting of themselves. A river is too narrow and too sheltered for the wind's broad chop;
// what ruffles it is its own running. \`fine\` and \`mid\` fade the small ones far off, as on a lake.
fn ripples(p: vec2f, t: f32, fine: f32, mid: f32, span: f32) -> vec3f {
  var c = octave(p, t, 0.26, 1.1, vec2f(0.12, -0.05), span);
  if (mid > 0.0) {
    c += mid * 0.6 * octave(p, t, 0.63, 2.3, vec2f(-0.05, 0.14), span);
  }
  if (fine > 0.0) {
    c += fine * 0.35 * octave(p, t, 1.45, 3.7, vec2f(0.16, 0.09), span);
  }
  return c;
}

// Those ripples carried along by \`current\` (m/s), every one of them: two copies, each dragged
// downstream for a \`FLOW_CYCLE\` and started over, half a cycle apart and each weighted most halfway
// through its run, so the whole surface moves with the water without smearing. Near the camera that is
// six noises a pixel where a lake's chop is four, and two far off where a lake's is one.
fn riverChop(p: vec2f, t: f32, fine: f32, mid: f32, current: vec2f, span: f32) -> vec3f {
  let phase = fract(t / ${FLOW_CYCLE.toFixed(2)});
  let weight = 1.0 - abs(1.0 - 2.0 * phase);
  let a = ripples(p - current * phase * ${FLOW_CYCLE.toFixed(2)}, t, fine, mid, span);
  let b = ripples(p - current * fract(phase + 0.5) * ${FLOW_CYCLE.toFixed(2)} + vec2f(13.7, 7.1), t, fine, mid, span);
  // Two copies blended lose some contrast at the crossover: made up here.
  return (a * weight + b * (1.0 - weight)) * 1.3;
}

// Foam on running water at \`q\`, about -1 to 1: streaks drawn out along \`dir\`, the way it runs (the noise
// smeared back along it, so a slow turn of the current never shears it). Far off (\`fine\` 0), where no
// streak can be told, one noise. Read from the chop map at mip \`lod\`, which is a quarter of what
// working the noise out costs over a pool full of foam.
fn frothAt(q: vec2f, dir: vec2f, fine: f32, lod: f32) -> f32 {
  let p = q * 1.3;
  var streak = mappedNoise(p, lod).x;
  if (fine > 0.0) {
    streak = mix(streak, 0.5 * (streak + mappedNoise(p - dir * 1.1, lod).x), fine);
    // And near enough to tell, it is not one sheet: holes and threads a hand's breadth to a pace wide.
    streak += fine * ${FOAM_LACE.toFixed(2)} * mappedNoise(p * 3.3 + vec2f(3.1, 8.7), lod + 1.7).x;
  }
  return streak * 1.1;
}

// That foam carried along by \`current\` as \`riverChop\` carries the ripples. The second copy is turned,
// so the map's repeat (every ${(CHOP_CELLS / 1.3).toFixed(0)} m) never lines up in both. \`span\` is how many metres of water a pixel spans.
fn carriedFroth(p: vec2f, t: f32, current: vec2f, fine: f32, span: f32) -> f32 {
  let dir = current / max(length(current), 1e-3);
  let phase = fract(t / ${FLOW_CYCLE.toFixed(2)});
  let weight = 1.0 - abs(1.0 - 2.0 * phase);
  let lod = log2(max(span * 1.3 * CHOP_TEXELS, 1.0));
  let turn = mat2x2f(0.4536, 0.8912, -0.8912, 0.4536);
  let a = frothAt(p - current * phase * ${FLOW_CYCLE.toFixed(2)}, dir, fine, lod);
  let b = frothAt(turn * (p - current * fract(phase + 0.5) * ${FLOW_CYCLE.toFixed(2)}) + vec2f(5.3, 9.1), turn * dir, fine, lod);
  return a * weight + b * (1.0 - weight);
}

// Where the wind is gusting, 0 (a glassy slick) to 1 (ruffled dark water): broad patches, tens of metres
// to a couple of hundred across, that drift slowly, so water seen from above is never the same twice.
fn gusts(p: vec2f, t: f32) -> f32 {
  let a = vnoise(p * 0.009 + vec2f(t * 0.0045, t * 0.002)).x;
  let b = vnoise(p * 0.027 + vec2f(17.3 - t * 0.007, 4.1 + t * 0.005)).x;
  return clamp(0.55 + 0.6 * a + 0.35 * b, 0.0, 1.0);
}

// What the water mirrors: the sky the engine draws (sky.wgsl), read from its panorama with the sun's and
// the moon's discs added exactly, or without one a horizon-to-zenith sky with a few soft clouds and the
// sun's halo.
fn skyAt(r: vec3f, t: f32) -> vec3f {
  if (frame.heightFog.w > 0.5) {
    let edge = 0.5 / ${PANORAMA_SIZE.toFixed(1)};
    let uv = clamp(skyPanoramaUv(r), vec2f(edge), vec2f(1.0 - edge));
    return skyFrom(r, textureSampleLevel(skyPanorama, materialSampler, uv, 0.0));
  }
  let up = clamp(r.y, 0.0, 1.0);
  var c = mix(params.sky, params.sky * vec3f(0.55, 0.75, 1.05), pow(up, 0.55));
  let cp = r.xz / (r.y + 0.3) * 0.9 + vec2f(t * 0.03, 0.0);
  let cloud = smoothstep(0.5, 0.95, 0.5 + 0.3 * sin(cp.x * 1.3 + sin(cp.y * 1.1) * 1.6) + 0.25 * sin(cp.y * 2.1 + sin(cp.x * 1.7) * 1.2));
  c = mix(c, params.cloud, cloud * 0.4 * smoothstep(0.02, 0.3, r.y));
  let s = max(dot(r, frame.sunDir.xyz), 0.0);
  return c + params.sun * (pow(s, 6.0) * 0.12 + pow(s, 200.0) * 1.2);
}

fn caustics(p: vec2f, t: f32) -> f32 {
  let a = abs(sin(p.x * 1.3 + sin(p.y * 0.9 + t * 0.8) * 1.5));
  let b = abs(sin(p.y * 1.1 + sin(p.x * 1.2 - t * 0.6) * 1.5));
  return pow(1.0 - a * b, 6.0);
}

fn surface(s: Surface) -> vec4f {
  let time = frame.fog.w;
  let depth = s.custom.x;
  let flow = s.custom.yzw;
  let world = s.world.xz;
  // Metres of water a pixel spans, for the chop map's mip level: taken here, before any branch. Across
  // and along the view differ a great deal on water seen at a low angle, so their geometric mean.
  let span = sqrt(length(dpdx(world)) * length(dpdy(world)));
  let stream = flow.z > 0.5;
  // A tile's own surface leaves the refined square to the refined mesh, which covers just its triangles
  // there: the square is on the cells' lines and the water moves only up and down, so the two meet.
  if (!stream && s.uv.x < 0.5 && inRefine(world) >= 0.0) {
    discard;
  }
  // And the refined mesh, which reaches a cell past the square, leaves what is outside it to the tiles.
  if (s.uv.x >= 0.5 && inRefine(world) < 0.0) {
    discard;
  }
  var p = world * 0.7;
  if (stream) {
    p = vec2f(flow.x - time * ${FLOW_SPEED.toFixed(2)}, flow.y * 3.0) * 2.2;
  }
  let to = frame.cameraPos.xyz - s.world;
  let away = length(to);
  let fine = 1.0 - smoothstep(25.0, 110.0, away);
  let mid = 1.0 - smoothstep(80.0, 320.0, away);
  // Lakes and rivers: the wind's chop, stronger in the gusts, carried along by a river's current; how
  // white the water runs with it. A stream keeps its flow's bands.
  let flowing = current(s.custom);
  let speed = length(flowing);
  // Running water, which is a river: a stream keeps its own flow, and ice does not run.
  let river = speed > 0.01 && !stream && params.wave.y > 0.5 && params.wave.w > 0.5;
  // How broken a river's surface is, 0 glassy to 1 broken. Water that is thin for its speed breaks (its
  // Froude number, the speed over that of a wave on water that deep), and so does water under the foam
  // a rapid sent down; a deep run at the same speed slides by smooth, and a pool lies still.
  var rough = 0.0;
  if (river) {
    rough = max(smoothstep(${ROUGH_FROUDES[0].toFixed(2)}, ${ROUGH_FROUDES[1].toFixed(2)}, speed / sqrt(9.81 * max(depth, 0.05))), 0.6 * s.uv.y);
  }
  // How white it runs: the faster the whiter, but a smooth tongue of deep fast water keeps most of its dark.
  let white = smoothstep(${WHITE_WATER_SPEEDS[0].toFixed(2)}, ${WHITE_WATER_SPEEDS[1].toFixed(2)}, speed) * params.wave.y * mix(${TONGUE_WHITE.toFixed(2)}, 1.0, rough);
  // Each kind works out only its own: a \`select\` would run both sides for every pixel.
  var gust = 0.5;
  var chopped = vec3f(0.0);
  var wave = 0.0;
  // The chop's slope: rougher in a gust, nearly flat on a slick. A stream ripples along its flow.
  var grad = vec2f(0.0);
  if (stream) {
    let e = 0.15;
    let h = bands(p, 0.0);
    wave = h;
    grad = vec2f(bands(p + vec2f(e, 0.0), 0.0) - h, bands(p + vec2f(0.0, e), 0.0) - h) / e * 0.5;
  } else if (river) {
    chopped = riverChop(world, time, fine, mid, flowing, span);
    // Running water draws its ripples out along itself: their slope along the current is mostly taken
    // out, the more the faster it runs, which leaves streaks that lie with the flow.
    let dir = flowing / speed;
    let slope = chopped.yz - dir * dot(chopped.yz, dir) * 0.6 * smoothstep(0.3, 1.5, speed);
    // The wind's gusts matter little down between the banks: how ruffled a river is goes by how it runs.
    gust = clamp(0.12 + 0.2 * s.extra[0].x + 0.85 * rough, 0.0, 1.0);
    wave = clamp(chopped.x * 0.5, -1.0, 1.0) * mix(0.7, 1.15, gust);
    grad = slope * mix(0.25, 1.1, rough) * (1.0 + 0.4 * white);
  } else {
    gust = s.extra[0].x;
    chopped = chop(world, time, fine, mid, span);
    wave = clamp(chopped.x * 0.5, -1.0, 1.0) * mix(0.7, 1.15, gust);
    grad = chopped.yz * mix(0.35, 1.1, gust);
  }
  let band = smoothstep(0.35, 0.5, wave) * 0.5 + smoothstep(0.6, 0.7, wave) * 0.5;

  // How deep and how clear the water looks: a stream by its own few hand-breadths, dark down the
  // middle of its channel and clear only at the banks.
  var deep = select(smoothstep(0.0, 2.2, depth), smoothstep(0.0, ${STREAM_FILL.toFixed(2)}, depth), stream);
  var color = mix(params.shallow, params.deep, deep);

  var clear = select(1.0 - smoothstep(0.0, 1.4, depth), 1.0 - smoothstep(0.0, ${STREAM_FILL.toFixed(2)}, depth), stream);
  // How much of this water is stained: a river's is (the importer writes even its slack water as
  // running, so it is the river's colour from bank to bank, fading only where it runs out into a lake),
  // and on a world that says so (\`params.ice.y\`) so are its lakes and sea.
  var stained = 0.0;
  if (!stream && params.wave.w > 0.5) {
    stained = max(params.ice.y, smoothstep(0.0, 0.045, speed) * params.wave.y);
  }
  if (stained < 1.0) {
    // A stream runs over mud and stones, darker than a lake's sandy shallows.
    color = mix(color, select(params.bed, params.bed * vec3f(0.5, 0.45, 0.35), stream), clear * 0.45);
  }
  if (stained > 0.0) {
    // Stained water (\`params.river\` is its colour where it is deep) takes the light out as water does,
    // by how far the light goes through it: down to the bed and back up to the eye. Each metre passes
    // more of the colours the stain is made of, so the bed shows through shallow water in the stain's
    // own warm tone and is lost in a pool.
    let strongest = max(max(params.river.r, params.river.g), max(params.river.b, 1e-3));
    let kept = pow(mix(vec3f(${RIVER_PASSES[0].toFixed(2)}), vec3f(${RIVER_PASSES[1].toFixed(2)}), params.river / strongest), vec3f(${RIVER_PATH.toFixed(1)} * max(depth, 0.0)));
    let seen = (kept.r + kept.g + kept.b) / 3.0;
    color = mix(color, params.river * (1.0 - kept) + params.bed * 0.8 * kept, stained);
    deep = mix(deep, 1.0 - seen, stained);
    clear = mix(clear, seen * seen, stained);
  }
  let bedP = select(world * 0.9, p * 0.8, stream);
  // The light's web on the bed shows only through clear, shallow water.
  var web = 0.0;
  if (clear > 0.0) {
    web = caustics(bedP, select(time * 0.8, time, stream));
  }
  color += params.foam * web * clear * 0.35;
  color = mix(color, params.foam, band * 0.12);
  // The water's own colour and its foam are lit as the flat ground is, so a lake darkens with the shore at night.
  let ground = groundLight(s.world, vec3f(0.0, 1.0, 0.0), 1.0) / PI;
  color *= ground;

  // The real waves' slope, with the small swell's ripples over it. A river's still surface slopes too
  // (\`lie\`, from the mesh's normals): it falls along its course, stands higher where the current is
  // braked and in waves across a rapid, which surge (\`stand\`). A stream's ribbon has no normals.
  let share = waveShare(s.custom);
  let stand = standing(world, s.custom);
  var lie = vec2f(0.0);
  if (!stream) {
    lie = -s.normal.xz * sign(s.normal.y) / max(abs(s.normal.y), 0.2);
  }
  let big = waves(world, params.wave.x, s.custom.y) * share + vec3f(0.0, lie + stand.yz);
  // And what has moved through it: its bow waves, wakes and rings, and their churn.
  let churn = wake(world) * wakeShare(s.custom);
  var n = normalize(vec3f(-grad.x * 0.45 - big.y - churn.y, 1.0, -grad.y * 0.45 - big.z - churn.z));
  if (!s.front) {
    n.y = -n.y;
  }
  let view = normalize(to);

  if (!s.front && params.ice.x < 0.5) {
    // From under the surface (\`n\` faces down, at the camera): the sky shows through a window overhead,
    // bent by the surface; further out than about 49° from straight up the surface mirrors the dark
    // water instead. The scene's mirror is only for eyes above the water.
    let up = refract(-view, n, 1.33);
    let r = select(vec3f(0.0, 1.0, 0.0), normalize(up), dot(up, up) > 1e-4);
    let window = smoothstep(0.6, 0.72, clamp(abs(dot(n, view)), 0.0, 1.0));
    let below = params.deep * ground * 0.45;
    let above = skyAt(vec3f(r.x, abs(r.y), r.z), time);
    let c = mix(below, above, window) + params.foam * band * 0.08 * ground;
    return vec4f(c, mix(0.95, 0.55, window));
  }

  // Schlick: little reflection looking straight down, nearly a mirror at a grazing angle.
  let ndv = clamp(abs(dot(n, view)), 0.0, 1.0);
  let fresnel = 0.03 + 0.97 * pow(1.0 - ndv, 5.0);
  let lit = floor(clamp(dot(n, frame.sunDir.xyz) * 0.5 + 0.5, 0.0, 1.0) * 3.0) / 3.0;
  color *= 0.9 + 0.2 * lit;
  // Ruffled water is a shade darker than a slick.
  color *= mix(1.05, 0.9, gust);
  var bounce = reflect(-view, n);
  bounce.y = abs(bounce.y);
  var mirror = skyAt(bounce, time);
  // The scene mirrored in the water's plane, drawn from the same view and flipped across (see the renderer).
  // Water at another height than the plane, or without a mirror, keeps the sky.
  // The mirror is the view flipped left to right, so the pixel's own place in it is across the screen.
  let screen = s.pixel / frame.viewport.xy;
  let ripple = vec2f(n.x, n.z) * 0.05 * fine;
  let ruv = clamp(vec2f(1.0 - screen.x, screen.y) + ripple, vec2f(0.001), vec2f(0.999));
  let seen = textureSampleLevel(reflection, materialSampler, ruv, 0.0);
  // Measured at the water's level under the waves, so a crest still mirrors the scene.
  let level = params.plane.y * (1.0 - smoothstep(0.2, 0.5, abs(s.world.y - big.x - churn.x - params.plane.x)));
  mirror = mix(mirror, seen.rgb, seen.a * level);
  // Down in its channel a stream mirrors its own banks more than the open sky, and the scene's mirror,
  // made for the lake's level, misses it.
  if (stream) {
    mirror = mix(mirror, color * 0.55, 0.6);
  }
  color = mix(color, mirror, clamp((fresnel * 1.3 + 0.12) * mix(1.1, 0.8, gust), 0.0, 0.95));

  let sunward = max(dot(reflect(-frame.sunDir.xyz, n), view), 0.0);
  let glint = pow(sunward, 90.0);
  let spark = smoothstep(0.35, 0.6, glint);
  // A stream's ripples are small, and so are their glints.
  let glints = select(1.0, 0.35, stream);
  color += params.sun * (spark * 0.8 + pow(sunward, 24.0) * 0.12) * glints;

  let shore = select(${FOAM_DEPTH.toFixed(2)}, ${STREAM_FOAM_DEPTH.toFixed(2)}, stream) * (1.0 + 0.23 * sin(time * 1.3 + world.x * 0.6 + world.y * 0.45));
  let foam = 1.0 - smoothstep(shore * 0.55, shore, depth);
  // Whitecaps: the high, steep crests break, in ragged patches.
  let steep = length(big.yz);
  let crest = smoothstep(0.45, 0.9, big.x) * smoothstep(0.15, 0.4, steep);
  // The lace that breaks up all the foam: two noises, worked out only where there is foam to break up
  // (the shore's line, a breaking crest or a wake), which open water mostly has none of.
  var lace = 0.0;
  if (foam > 0.0 || crest > 0.0 || churn.w > 0.0) {
    let lacy = vnoise(world * 0.9 + vec2f(time * 0.6, -time * 0.4)).x * 0.65 + vnoise(world * 2.3 - vec2f(time * 0.9, time * 0.5)).x * 0.35;
    lace = smoothstep(0.1, 0.55, lacy * 0.5 + 0.5);
  }
  color = mix(color, params.foam * ground, foam * mix(0.55, 1.0, lace));
  let cap = crest * smoothstep(0.2, 0.6, lace) * mix(0.5, 1.2, gust);
  color = mix(color, params.foam * ground, cap * 0.75);
  // A wake churns white behind what made it: solid where it is fresh, and as it thins it breaks up
  // into lace and streaks, the thinnest of it surviving only in the froth noise's hollows.
  var froth = 0.5;
  if (churn.w > 0.0) {
    froth = vnoise(world * 3.1 + vec2f(time * 0.3, time * 0.2)).x * 0.5 + 0.5;
  }
  let wash = smoothstep(0.0, 0.25, churn.w - froth * 0.8) * mix(0.75, 1.0, lace);
  color = mix(color, params.foam * ground, wash * 0.9);
  // Running water: a rapid churned white, the more of it the faster it runs, and the foam it made,
  // carried on by the current to where the importer found it would lie (\`carried\`): trailing away
  // below the rapid and turning slowly in the slack water beside it.
  var rush = 0.0;
  let carried = s.uv.y * params.wave.y * params.wave.z;
  if ((white > 0.0 || carried > ${FOAM_COVERS[0].toFixed(2)}) && !stream) {
    let foamy = carriedFroth(world, time, flowing, fine, span) * 0.5 + 0.5;
    let amount = max(white, (0.45 + 0.35 * carried) * smoothstep(${FOAM_COVERS[0].toFixed(2)}, ${FOAM_COVERS[1].toFixed(2)}, carried));
    // Broken white over dark water between: thin foam is scattered flecks, and it closes up as it
    // thickens. A standing wave breaks on the face that meets the current (where the surface rises
    // downstream) and its back runs dark, so a rapid shows its waves as bars of white across it.
    let edge = mix(0.74, 0.42, max(white, carried));
    let face = dot(lie + stand.yz, flowing / max(speed, 1e-3));
    rush = smoothstep(edge, edge + 0.09, foamy + ${WAVE_FOAM.toFixed(1)} * face - ${WAVE_FOAM_LESS.toFixed(2)} * white) * amount;
    color = mix(color, params.foam * ground, rush * 0.9);
  }

  if (params.ice.x > 0.5) {
    // Wind-scoured ice under drifts of snow, with the odd long crack: still, and opaque.
    let drift = smoothstep(-0.25, 0.35, bands(world * 0.09, 0.0) + bands(world * 0.31, 0.0) * 0.4);
    let crack = smoothstep(0.9, 0.99, 1.0 - abs(sin(world.x * 0.13 + 2.0 * sin(world.y * 0.08))));
    let ice = mix(vec3f(0.74, 0.86, 0.93), vec3f(0.92, 0.95, 0.98), drift) * (1.0 - crack * 0.14 * (1.0 - drift));
    return vec4f(ice * ground * 1.15 + mirror * fresnel * 0.2 * (1.0 - drift), smoothstep(-${SHORE_FADE.toFixed(2)}, 0.0, depth));
  }
  var alpha = max(smoothstep(0.0, 0.1, depth) * mix(0.62, 0.9, deep), max(max(cap, wash), rush) * 0.9);
  alpha = clamp(alpha - clear * 0.12 + fresnel * 0.35 + spark * 0.4 * glints, 0.0, 1.0);
  // Past the shoreline, where the cell's far corner is dry ground, nothing: not even the mirror's sheen.
  alpha *= smoothstep(-${SHORE_FADE.toFixed(2)}, 0.0, depth);
  return vec4f(color, alpha);
}
`


/** What the water samples until the renderer's mirror is handed over: transparent, so it shows sky. */
const blank = new Texture({ data: new Uint8Array(4), width: 1, height: 1 }, { srgb: false, wrap: 'clamp', mipmaps: false })

/** A wake map with nothing on it: no height, no slope, no foam. */
const still = new Texture({ data: new Uint8Array([0, 0, 0, 0]), width: 1, height: 1 }, { srgb: false, wrap: 'clamp', mipmaps: false })

/** The chop's noise, read filtered and at the mip level the distance asks for (`chopPixels`). */
const chop = new Texture({ data: chopPixels(), width: CHOP_CELLS * CHOP_TEXELS, height: CHOP_CELLS * CHOP_TEXELS }, { srgb: false, wrap: 'repeat', mipmaps: true })

/** One shared material, so every tile draws with the same pipeline and a sky change reaches them all. */
let shared: ShaderMaterial | null = null
function material(): ShaderMaterial {
  shared ??= new ShaderMaterial({
    wgsl: WGSL,
    vertex: VERTEX,
    uniforms: {
      shallow: Color.hex(0x7fd8f5),
      deep: Color.hex(0x2f8fd8),
      foam: Color.hex(0xf4fcff),
      bed: Color.hex(0xd9cf9a),
      // A river's colour where it is deep (\`Manifest::river_tint\`, set by \`setWaterRiver\`).
      river: Color.hex((RIVER_TINT[0] << 16) | (RIVER_TINT[1] << 8) | RIVER_TINT[2]),
      sky: Color.hex(0xffffff),
      sun: Color.hex(0xffffff),
      cloud: Color.hex(0xffffff),
      // Height of the mirrored plane, and 1 while there is one.
      plane: [0, 0],
      // 1 while the water is frozen; and 1 on a world whose lakes and sea are the rivers' colour too.
      ice: [0, 0],
      // The waves' clock, seconds into their loop; 1 while they run (not on ice); how much of a
      // river's carried foam is drawn; and 1 while a river is drawn as running water and not as a
      // lake. Only the GPU benchmark turns the last two down, to time them.
      wave: [0, 1, 1, 1],
      // Where the wake map's corner is (x, z) and how wide it is, metres (`WaveField.area`).
      wakeArea: [0, 0, 1, 0],
      // The square the refined mesh covers (\`WaterRefine\`): its corner (x, z), its side, metres, and 1 while it is drawn.
      refineArea: [0, 0, 0, 0],
    },
    // Still water until the wave field is handed over.
    // The chop map first: the material's sampler is its first texture's, and the map must repeat.
    textures: { chopMap: chop, reflection: blank, wakeMap: still, skyPanorama: blank },
    blend: 'alpha',
    // Written, so the ink (which reads depth) outlines the surface and not the bed seen through it.
    depthWrite: true,
    // Seen from below too: a swimmer (or anyone on the bed) looks up at it.
    side: 'double',
    // Streams float centimetres over the terrain; a bias keeps their far end from sinking into it.
    depthBias: 12,
    depthBiasSlope: 2,
  })
  return shared
}

/** The lake and river surfaces drawn now, for the refined mesh to cut finer near the camera. */
export const waterSurfaces = new Set<Mesh>()

/** The water material, for the refined mesh to draw with. */
export function waterMaterial(): ShaderMaterial {
  return material()
}

/** Hands the water the square the refined mesh covers, or none. */
export function setWaterRefine(area: Readonly<[number, number, number]> | null) {
  const m = material()
  ;(m.uniforms.refineArea as [number, number, number, number]).splice(0, 4, ...(area ?? [0, 0, 0]), area ? 1 : 0)
  m.needsUpdate()
}

/** Sets the colour of the rivers' deep water (sRGB hex, `WorldData.riverTint`), and whether the lakes and the sea are that colour too. */
export function setWaterRiver(tint: number, all: boolean) {
  ;(material().uniforms.river as Color).setHex(tint)
  ;(material().uniforms.ice as [number, number])[1] = all ? 1 : 0
  material().needsUpdate()
}

/** Freezes every lake, river and stream. */
export function freezeWater() {
  ;(material().uniforms.ice as [number, number])[0] = 1
  ;(material().uniforms.wave as number[])[1] = 0
  material().needsUpdate()
}

/** Hands the water the wave field's map (`WaveField.texture`) and where it lies (`WaveField.area`). */
export function setWaterWakes(map: Texture, area: Readonly<[number, number, number, number]>) {
  const m = material()
  m.textures.wakeMap = map
  ;(m.uniforms.wakeArea as [number, number, number, number]).splice(0, 4, ...area)
  m.needsUpdate()
}

/** Sets the waves' clock: `time` seconds into their loop (`waveTime` of the server's tick). */
export function setWaterTime(time: number) {
  ;(material().uniforms.wave as number[])[0] = time
  material().needsUpdate()
}

/** Has the water sample `texture`, the renderer's mirror image of the scene, and `sky`, its sky panorama. */
export function setWaterReflection(texture: Texture, sky: Texture) {
  material().textures.reflection = texture
  material().textures.skyPanorama = sky
  material().needsUpdate()
}

/** The height the mirror was drawn at this frame, or null for none (the water then reflects the sky alone). */
export function setWaterPlane(height: number | null) {
  const plane = material().uniforms.plane as [number, number]
  const on = height === null ? 0 : 1
  if (plane[1] === on && (height === null || plane[0] === height)) return
  plane[0] = height ?? plane[0]
  plane[1] = on
  material().needsUpdate()
}

/** Points the reflection and glint at the scene's sky and sun colours (sRGB hex), and the mirrored clouds at `cloud` (linear). */
export function setWaterLight(sky: number, sun: number, cloud: Readonly<Color>) {
  const uniforms = material().uniforms
  ;(uniforms.sky as Color).setHex(sky)
  ;(uniforms.sun as Color).setHex(sun)
  ;(uniforms.cloud as Color).copy(cloud)
  material().needsUpdate()
}

/**
 * One tile's water, or null if it has none: from `prepared` when `prepareTile` built it already. It
 * is translucent and added apart from the tile's group so the game can place it in the world apart from the tile.
 */
export function buildWater(tile: Pick<TileData, 'water' | 'reach' | 'flow' | 'foam' | 'streams'>, ground: TileGround, around?: HeightSource, prepared?: WaterBuffers[]): Group | null {
  const meshes = (prepared ?? waterBuffers(tile, ground, around)).map((buffers) => {
    const mesh = new Mesh(new Geometry(buffers), material())
    mesh.renderOrder = 1
    if (buffers.surface) waterSurfaces.add(mesh)
    return mesh
  })
  if (meshes.length === 0) return null
  const group = new Group()
  group.add(...meshes)
  return group
}

/** Frees a tile's water geometry. The material is shared and stays. */
export function disposeWater(group: Group) {
  group.traverse((object) => {
    if (object instanceof Mesh) {
      waterSurfaces.delete(object)
      object.geometry.dispose()
    }
  })
  group.removeFromParent()
}
