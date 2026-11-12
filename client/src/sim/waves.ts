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
export const LOOP_SECONDS = 64
export const LOOP_TICKS = LOOP_SECONDS * TICK_RATE

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

const swellTerms = SWELLS.map((swell) => ({
  ...swell,
  k: (Math.PI * 2) / swell.length,
  omega: (Math.PI * 2 * swell.cycles) / LOOP_SECONDS,
  dx: Math.cos(swell.heading),
  dz: Math.sin(swell.heading),
}))

/** The waves at `(x, z)`, `time` seconds into the loop, at full height (scale by `shore`). */
export function wave(x: number, z: number, time: number): Wave {
  const out = { height: 0, slopeX: 0, slopeZ: 0, rise: 0 }
  for (const s of swellTerms) {
    const theta = s.k * (s.dx * x + s.dz * z) - s.omega * time + s.phase
    const lift = Math.max(0, 0.5 * (1 + Math.cos(theta)))
    const liftLess = lift ** (PEAK - 1)
    out.height += 2 * s.height * (liftLess * lift - PEAK_MEAN)
    const turn = -s.height * PEAK * liftLess * Math.sin(theta)
    out.slopeX += turn * s.k * s.dx
    out.slopeZ += turn * s.k * s.dz
    out.rise -= turn * s.omega
  }
  return out
}

const f = (value: number) => value.toFixed(6)

/**
 * The same waves in WGSL: `fn waves(p: vec2f, t: f32) -> vec3f` gives the height and the slope `(dh/dx, dh/dz)`
 * at full height, and `fn waveShore(depth: f32) -> f32` the share of them water that deep has.
 */
export const WAVES_WGSL = /* wgsl */ `
fn waves(p: vec2f, t: f32) -> vec3f {
  var out = vec3f(0.0);
${swellTerms
  .map(
    (s) => `  {
    let d = vec2f(${f(s.dx)}, ${f(s.dz)});
    let theta = ${f(s.k)} * dot(d, p) - ${f(s.omega)} * t + ${f(s.phase)};
    let lift = max(0.0, 0.5 * (1.0 + cos(theta)));
    let liftLess = pow(lift, ${f(PEAK - 1)});
    out.x += ${f(2 * s.height)} * (liftLess * lift - ${f(PEAK_MEAN)});
    let turn = -${f(s.height * PEAK)} * liftLess * sin(theta);
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
`
