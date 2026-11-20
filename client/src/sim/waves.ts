import { TICK_RATE } from './consts'

/**
 * Waves on open water, mirroring `crates/shared/src/waves.rs`: the same swells as a function of place
 * and server tick, so what the water's surface draws is what the sim rides vehicles on.
 * `waves.spec.ts` holds this copy to the Rust one.
 */

/**
 * One swell: heading (radians from +X towards +Z), wavelength and height (metres, half crest to trough),
 * how far its water swings across against how high it rises (`steep`), cycles a loop, phase.
 */
interface Swell {
  heading: number
  length: number
  height: number
  steep: number
  cycles: number
  phase: number
}

/**
 * Gerstner swells: the water goes round as each passes, gathering under the narrow crests and spreading
 * over the broad troughs. Two long ones heave the sea, three short ones chop it.
 */
export const SWELLS: readonly Swell[] = [
  { heading: 0.3, length: 70.0, height: 1.1, steep: 2.0, cycles: 8, phase: 0.0 },
  { heading: -0.2, length: 45.0, height: 0.75, steep: 2.0, cycles: 10, phase: 1.7 },
  { heading: 0.7, length: 20.0, height: 0.16, steep: 1.8, cycles: 17, phase: 4.1 },
  { heading: -0.9, length: 10.0, height: 0.07, steep: 1.5, cycles: 24, phase: 2.6 },
  { heading: 1.6, length: 6.0, height: 0.035, steep: 1.2, cycles: 31, phase: 0.9 },
]
export const SHORE_DEPTHS = [0.3, 2.5] as const
/** How wide open water must be for a swell to rise on it, in its wavelengths: none below the first, all of it above the second. */
export const FETCH = [2.0, 8.0] as const
/** Open water this wide, metres, carries every swell (the sea). */
export const OPEN_SEA = 1020
/** The step `TileData.reach` counts in, metres (`REACH_STEP` in `crates/shared`). */
export const REACH_STEP = 4
export const LOOP_SECONDS = 64
export const LOOP_TICKS = LOOP_SECONDS * TICK_RATE
/** The step `TileData.flow` counts in, m/s (`FLOW_STEP` in `crates/shared`). */
export const FLOW_STEP = 0.05
/**
 * White water: the surge of a rapid's standing waves (the waves themselves are part of the river's level).
 * It is this high (metres) where the current runs at the second speed (m/s), and there is none below the first.
 */
export const CHURN_HEIGHT = 0.1
export const CHURN_SPEEDS = [1.2, 2.6] as const
/** The surge's four waves, a quarter of a half-turn apart: water takes each by how nearly it runs along its heading. */
const CHURN: readonly Swell[] = [
  { heading: 0.2, length: 4.6, height: 1.0, steep: 0, cycles: 41, phase: 0.0 },
  { heading: 0.2 + Math.PI / 4, length: 5.3, height: 1.0, steep: 0, cycles: 37, phase: 1.3 },
  { heading: 0.2 + Math.PI / 2, length: 4.9, height: 1.0, steep: 0, cycles: 43, phase: 2.9 },
  { heading: 0.2 + (3 * Math.PI) / 4, length: 5.6, height: 1.0, steep: 0, cycles: 35, phase: 4.4 },
]
/** What the four waves' shares add up to, whichever way the water runs. */
const CHURN_SHARES = 1.5

/**
 * The surface over a point: its height above the water's level (metres), its slope `[dh/dx, dh/dz]`,
 * how fast it rises (m/s), and how fast the waves swing the water there across (`orbitX`, `orbitZ`, m/s).
 */
export interface Wave {
  height: number
  slopeX: number
  slopeZ: number
  rise: number
  orbitX: number
  orbitZ: number
}

/**
 * Where the water at rest at a point has gone: across (`shiftX`, `shiftZ`, metres) and up (metres), and
 * how much the surface about it is squeezed (1 at rest, under 1 gathered into a crest).
 */
export interface Particle {
  shiftX: number
  shiftZ: number
  height: number
  squeeze: number
}

/** How many Newton steps find the water that has come to stand over a point. */
const SOLVE_STEPS = 3

/** The wave clock at server tick `tick` (fractional for a frame between ticks), seconds into the loop. */
export function waveTime(tick: number): number {
  return ((((tick % LOOP_TICKS) + LOOP_TICKS) % LOOP_TICKS) / TICK_RATE)
}

/** How much of the waves water `depth` metres deep has. */
export function shore(depth: number): number {
  const t = Math.min(1, Math.max(0, (depth - SHORE_DEPTHS[0]) / (SHORE_DEPTHS[1] - SHORE_DEPTHS[0])))
  return t * t * (3 - 2 * t)
}

/** How much of a swell `length` metres long rises on open water `reach` metres across. */
export function fetch(reach: number, length: number): number {
  const t = Math.min(1, Math.max(0, (reach - FETCH[0] * length) / ((FETCH[1] - FETCH[0]) * length)))
  return t * t * (3 - 2 * t)
}

const swellTerms = SWELLS.map((swell) => ({
  ...swell,
  k: (Math.PI * 2) / swell.length,
  omega: (Math.PI * 2 * swell.cycles) / LOOP_SECONDS,
  dx: Math.cos(swell.heading),
  dz: Math.sin(swell.heading),
}))

/** Everything the waves do to the water at rest at a point (`Sums` in the Rust), reused from call to call. */
const at = { shiftX: 0, shiftZ: 0, xx: 1, xz: 0, zz: 1, height: 0, gradX: 0, gradZ: 0, lift: 0, orbitX: 0, orbitZ: 0, level: 0 }

function sums(x0: number, z0: number, time: number, reach: number, share: number) {
  at.shiftX = at.shiftZ = at.xz = at.height = at.gradX = at.gradZ = at.lift = at.orbitX = at.orbitZ = at.level = 0
  at.xx = at.zz = 1
  for (const s of swellTerms) {
    const height = s.height * fetch(reach, s.length) * share
    if (height <= 0) continue
    const swing = s.steep * height
    const theta = s.k * (s.dx * x0 + s.dz * z0) - s.omega * time + s.phase
    const sin = Math.sin(theta)
    const cos = Math.cos(theta)
    at.shiftX -= swing * sin * s.dx
    at.shiftZ -= swing * sin * s.dz
    const gather = swing * s.k * cos
    at.xx -= gather * s.dx * s.dx
    at.xz -= gather * s.dx * s.dz
    at.zz -= gather * s.dz * s.dz
    at.height += height * cos
    at.gradX -= height * s.k * sin * s.dx
    at.gradZ -= height * s.k * sin * s.dz
    at.lift += height * s.omega * sin
    at.orbitX += swing * s.omega * cos * s.dx
    at.orbitZ += swing * s.omega * cos * s.dz
    at.level += 0.5 * swing * s.k * height
  }
}

/**
 * Where the water at rest at `(x0, z0)` has gone, `time` seconds into the loop, on open water `reach`
 * metres across with `share` of the waves (see `shore`): how the water shader moves the surface.
 */
export function particle(x0: number, z0: number, time: number, reach: number, share: number): Particle {
  sums(x0, z0, time, reach, share)
  return { shiftX: at.shiftX, shiftZ: at.shiftZ, height: at.height + at.level, squeeze: at.xx * at.zz - at.xz * at.xz }
}

/**
 * The waves over `(x, z)`, `time` seconds into the loop, on open water `reach` metres across (each swell
 * as much as `fetch` lets rise) with `share` of them (see `shore`). The water standing over the point has
 * come from elsewhere, so it is found first, by Newton's method.
 */
export function wave(x: number, z: number, time: number, reach: number, share: number): Wave {
  if (share <= 0) return { height: 0, slopeX: 0, slopeZ: 0, rise: 0, orbitX: 0, orbitZ: 0 }
  let x0 = x
  let z0 = z
  sums(x0, z0, time, reach, share)
  for (let i = 0; i < SOLVE_STEPS; i++) {
    const missX = x0 + at.shiftX - x
    const missZ = z0 + at.shiftZ - z
    const det = at.xx * at.zz - at.xz * at.xz
    x0 -= (at.zz * missX - at.xz * missZ) / det
    z0 -= (at.xx * missZ - at.xz * missX) / det
    sums(x0, z0, time, reach, share)
  }
  const det = at.xx * at.zz - at.xz * at.xz
  const slopeX = (at.zz * at.gradX - at.xz * at.gradZ) / det
  const slopeZ = (at.xx * at.gradZ - at.xz * at.gradX) / det
  return {
    height: at.height + at.level,
    slopeX,
    slopeZ,
    rise: at.lift - slopeX * at.orbitX - slopeZ * at.orbitZ,
    orbitX: at.orbitX,
    orbitZ: at.orbitZ,
  }
}

const churnTerms = CHURN.map((swell) => ({
  ...swell,
  k: (Math.PI * 2) / swell.length,
  omega: (Math.PI * 2 * swell.cycles) / LOOP_SECONDS,
  dx: Math.cos(swell.heading),
  dz: Math.sin(swell.heading),
}))

/** How big the surge of a rapid's standing waves is on water running at `speed` m/s, metres. */
export function churnSize(speed: number): number {
  const t = Math.min(1, Math.max(0, (speed - CHURN_SPEEDS[0]) / (CHURN_SPEEDS[1] - CHURN_SPEEDS[0])))
  return CHURN_HEIGHT * t * t * (3 - 2 * t)
}

/**
 * The surge of a rapid's standing waves at `(x, z)`, `time` seconds into the loop, on water running at
 * `(flowX, flowZ)` m/s: its crests lie across the current.
 */
export function churn(x: number, z: number, time: number, flowX: number, flowZ: number): Wave {
  const out = { height: 0, slopeX: 0, slopeZ: 0, rise: 0, orbitX: 0, orbitZ: 0 }
  const speed = Math.hypot(flowX, flowZ)
  const size = churnSize(speed) / CHURN_SHARES
  if (size <= 0) return out
  for (const s of churnTerms) {
    const along = (s.dx * flowX + s.dz * flowZ) / speed
    const height = size * along ** 4
    const a = s.k * (s.dx * x + s.dz * z) + s.phase
    const b = s.omega * time
    out.height += height * Math.sin(a) * Math.cos(b)
    out.slopeX += height * s.k * s.dx * Math.cos(a) * Math.cos(b)
    out.slopeZ += height * s.k * s.dz * Math.cos(a) * Math.cos(b)
    out.rise -= height * s.omega * Math.sin(a) * Math.sin(b)
  }
  return out
}

const f = (value: number) => value.toFixed(6)

/**
 * The same waves in WGSL: `fn gerstner(p0: vec2f, t: f32, reach: f32, share: f32) -> Gerstner` gives where
 * the water at rest at `p0` has gone (`shift` across and `height` up, metres), the slope of the surface
 * there `(dh/dx, dh/dz)` and how squeezed it is (`squeeze`, under 1 gathered into a crest), on open water
 * `reach` metres across with `share` of the waves; `fn waveShore(depth: f32) -> f32` the share of them
 * water that deep has, and `fn churnWaves(p: vec2f, t: f32, flow: vec2f) -> vec3f` the surge of a rapid's
 * standing waves on water running at `flow` m/s. The shader moves each vertex as its water moves, so it
 * needs no solve: the solve is only for the sim's question of what stands over a fixed point.
 */
export const WAVES_WGSL = /* wgsl */ `
struct Gerstner {
  shift: vec2f,
  height: f32,
  slope: vec2f,
  squeeze: f32,
}

fn gerstner(p0: vec2f, t: f32, reach: f32, share: f32) -> Gerstner {
  var shift = vec2f(0.0);
  var height = 0.0;
  var grad = vec2f(0.0);
  // The stretch of the surface, symmetric: xx, xz, zz.
  var stretch = vec3f(1.0, 0.0, 1.0);
${swellTerms
  .map(
    (s) => `  {
    let h = ${f(s.height)} * smoothstep(${f(FETCH[0] * s.length)}, ${f(FETCH[1] * s.length)}, reach) * share;
    let d = vec2f(${f(s.dx)}, ${f(s.dz)});
    let theta = ${f(s.k)} * dot(d, p0) - ${f(s.omega)} * t + ${f(s.phase)};
    let sn = sin(theta);
    let cs = cos(theta);
    shift -= ${f(s.steep)} * h * sn * d;
    let gather = ${f(s.steep * s.k)} * h * cs;
    stretch -= gather * vec3f(d.x * d.x, d.x * d.y, d.y * d.y);
    height += h * (cs + ${f(0.5 * s.steep * s.k)} * h);
    grad -= h * ${f(s.k)} * sn * d;
  }`,
  )
  .join('\n')}
  let squeeze = stretch.x * stretch.z - stretch.y * stretch.y;
  let slope = vec2f(stretch.z * grad.x - stretch.y * grad.y, stretch.x * grad.y - stretch.y * grad.x) / squeeze;
  return Gerstner(shift, height, slope, squeeze);
}

fn waveShore(depth: f32) -> f32 {
  return smoothstep(${f(SHORE_DEPTHS[0])}, ${f(SHORE_DEPTHS[1])}, depth);
}

fn churnWaves(p: vec2f, t: f32, flow: vec2f) -> vec3f {
  let speed = length(flow);
  let size = ${f(CHURN_HEIGHT / CHURN_SHARES)} * smoothstep(${f(CHURN_SPEEDS[0])}, ${f(CHURN_SPEEDS[1])}, speed);
  var out = vec3f(0.0);
  if (size <= 0.0) {
    return out;
  }
  let dir = flow / speed;
${churnTerms
  .map(
    (s) => `  {
    let d = vec2f(${f(s.dx)}, ${f(s.dz)});
    let along = dot(d, dir);
    let a = ${f(s.k)} * dot(d, p) + ${f(s.phase)};
    let height = size * along * along * along * along * cos(${f(s.omega)} * t);
    out.x += height * sin(a);
    out.y += height * ${f(s.k)} * d.x * cos(a);
    out.z += height * ${f(s.k)} * d.y * cos(a);
  }`,
  )
  .join('\n')}
  return out;
}
`
