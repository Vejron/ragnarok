import { TICK_RATE } from './consts'

/**
 * Waves on open water, mirroring `crates/shared/src/waves.rs`: the same swells as a function of place
 * and server tick, so what the water's surface draws is what the sim rides vehicles on.
 * `waves.spec.ts` holds this copy to the Rust one.
 */

/** One swell: heading (radians from +X towards +Z), wavelength and height (metres), cycles a loop, phase. */
interface Swell {
  heading: number
  length: number
  height: number
  cycles: number
  phase: number
}

export const SWELLS: readonly Swell[] = [
  { heading: 0.3, length: 90.0, height: 0.48, cycles: 7, phase: 0.0 },
  { heading: -0.2, length: 60.0, height: 0.3, cycles: 8, phase: 1.7 },
  { heading: 0.7, length: 25.0, height: 0.08, cycles: 15, phase: 4.1 },
  { heading: -0.9, length: 13.0, height: 0.035, cycles: 21, phase: 2.6 },
  { heading: 1.6, length: 7.5, height: 0.018, cycles: 29, phase: 0.9 },
]
/** Crests are `((1 + cos θ) / 2)^PEAK`, less that shape's mean so the water keeps its level. */
export const PEAK = 8
const PEAK_MEAN = 12870 / 65536
export const SHORE_DEPTHS = [0.3, 2.5] as const
/** How wide open water must be for a swell to rise on it, in its wavelengths: none below the first, all of it above the second. */
export const FETCH = [1.0, 4.0] as const
/** Open water this wide, metres, carries every swell (the sea). */
export const OPEN_SEA = 1020
/** The step `TileData.reach` counts in, metres (`REACH_STEP` in `crates/shared`). */
export const REACH_STEP = 4
export const LOOP_SECONDS = 64
export const LOOP_TICKS = LOOP_SECONDS * TICK_RATE
/** The step `TileData.flow` counts in, m/s (`FLOW_STEP` in `crates/shared`). */
export const FLOW_STEP = 0.05
/** White water's standing waves: this high (metres) where the current runs at the second speed (m/s), none below the first. */
export const CHURN_HEIGHT = 0.22
export const CHURN_SPEEDS = [1.5, 3.5] as const
/** The two standing waves, each a share of `CHURN_HEIGHT`. */
const CHURN: readonly Swell[] = [
  { heading: 0.4, length: 5.0, height: 0.65, cycles: 41, phase: 0.0 },
  { heading: 2.1, length: 3.3, height: 0.35, cycles: 53, phase: 1.3 },
]

/** The surface at a point above the water's level (metres), its slope `[dh/dx, dh/dz]` and how fast it rises (m/s). */
export interface Wave {
  height: number
  slopeX: number
  slopeZ: number
  rise: number
}

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

/**
 * The waves at `(x, z)`, `time` seconds into the loop, on open water `reach` metres across (each swell
 * as much as `fetch` lets rise), at full height for the depth (scale by `shore`).
 */
export function wave(x: number, z: number, time: number, reach: number): Wave {
  const out = { height: 0, slopeX: 0, slopeZ: 0, rise: 0 }
  for (const s of swellTerms) {
    const share = fetch(reach, s.length)
    if (share <= 0) continue
    const height = s.height * share
    const theta = s.k * (s.dx * x + s.dz * z) - s.omega * time + s.phase
    const lift = Math.max(0, 0.5 * (1 + Math.cos(theta)))
    const liftLess = lift ** (PEAK - 1)
    out.height += 2 * height * (liftLess * lift - PEAK_MEAN)
    const turn = -height * PEAK * liftLess * Math.sin(theta)
    out.slopeX += turn * s.k * s.dx
    out.slopeZ += turn * s.k * s.dz
    out.rise -= turn * s.omega
  }
  return out
}

const churnTerms = CHURN.map((swell) => ({
  ...swell,
  k: (Math.PI * 2) / swell.length,
  omega: (Math.PI * 2 * swell.cycles) / LOOP_SECONDS,
  dx: Math.cos(swell.heading),
  dz: Math.sin(swell.heading),
}))

/** How big a rapid's standing waves are on water running at `speed` m/s, metres. */
export function churnSize(speed: number): number {
  const t = Math.min(1, Math.max(0, (speed - CHURN_SPEEDS[0]) / (CHURN_SPEEDS[1] - CHURN_SPEEDS[0])))
  return CHURN_HEIGHT * t * t * (3 - 2 * t)
}

/** The standing waves of white water at `(x, z)`, `time` seconds into the loop, on water running at `speed` m/s. */
export function churn(x: number, z: number, time: number, speed: number): Wave {
  const out = { height: 0, slopeX: 0, slopeZ: 0, rise: 0 }
  const size = churnSize(speed)
  if (size <= 0) return out
  for (const s of churnTerms) {
    const height = size * s.height
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
 * The same waves in WGSL: `fn waves(p: vec2f, t: f32, reach: f32) -> vec3f` gives the height and the slope
 * `(dh/dx, dh/dz)` on open water `reach` metres across at full height for the depth,
 * `fn waveShore(depth: f32) -> f32` the share of them water that deep has, and
 * `fn churnWaves(p: vec2f, t: f32, speed: f32) -> vec3f` a rapid's standing waves on water running at `speed` m/s.
 */
export const WAVES_WGSL = /* wgsl */ `
fn waves(p: vec2f, t: f32, reach: f32) -> vec3f {
  var out = vec3f(0.0);
${swellTerms
  .map(
    (s) => `  {
    let share = smoothstep(${f(FETCH[0] * s.length)}, ${f(FETCH[1] * s.length)}, reach);
    let d = vec2f(${f(s.dx)}, ${f(s.dz)});
    let theta = ${f(s.k)} * dot(d, p) - ${f(s.omega)} * t + ${f(s.phase)};
    let lift = max(0.0, 0.5 * (1.0 + cos(theta)));
    let liftLess = pow(lift, ${f(PEAK - 1)});
    out.x += ${f(2 * s.height)} * share * (liftLess * lift - ${f(PEAK_MEAN)});
    let turn = -${f(s.height * PEAK)} * share * liftLess * sin(theta);
    out.y += turn * ${f(s.k)} * d.x;
    out.z += turn * ${f(s.k)} * d.y;
  }`,
  )
  .join('\n')}
  return out;
}

fn waveShore(depth: f32) -> f32 {
  return smoothstep(${f(SHORE_DEPTHS[0])}, ${f(SHORE_DEPTHS[1])}, depth);
}

fn churnWaves(p: vec2f, t: f32, speed: f32) -> vec3f {
  let size = ${f(CHURN_HEIGHT)} * smoothstep(${f(CHURN_SPEEDS[0])}, ${f(CHURN_SPEEDS[1])}, speed);
  var out = vec3f(0.0);
  if (size <= 0.0) {
    return out;
  }
${churnTerms
  .map(
    (s) => `  {
    let d = vec2f(${f(s.dx)}, ${f(s.dz)});
    let a = ${f(s.k)} * dot(d, p) + ${f(s.phase)};
    let b = cos(${f(s.omega)} * t);
    let height = size * ${f(s.height)};
    out.x += height * sin(a) * b;
    out.y += height * ${f(s.k)} * d.x * cos(a) * b;
    out.z += height * ${f(s.k)} * d.y * cos(a) * b;
  }`,
  )
  .join('\n')}
  return out;
}
`
