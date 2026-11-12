import type { Color } from '../math/vec'

/**
 * The Earth's atmosphere as the sky is built from it (Hillaire 2020, "A Scalable and Production Ready Sky
 * and Atmosphere Rendering Technique"): Rayleigh scattering off air, Mie scattering off haze, and ozone
 * absorbing. Distances are kilometres, coefficients per kilometre. The GPU's lookup tables
 * (`passes/sky.ts`) and the small CPU model below read the same numbers.
 */
export const ATMOSPHERE = {
  /** The ground's radius and the atmosphere's top, km. */
  bottom: 6360,
  top: 6460,
  /** Air's scattering per km, red, green and blue, and the height over which its density falls by e. */
  rayleigh: [5.802e-3, 13.558e-3, 33.1e-3] as const,
  rayleighHeight: 8,
  /** Haze's scattering and extinction per km (the same for every colour), and its height. */
  mieScatter: 3.996e-3,
  mieExtinction: 4.44e-3,
  mieHeight: 1.2,
  /** How strongly haze scatters forward: the glow around the sun. */
  mieG: 0.8,
  /** Ozone's absorption per km at its peak, in a layer `ozoneWidth` either side of `ozoneCentre`. */
  ozone: [0.65e-3, 1.881e-3, 0.085e-3] as const,
  ozoneCentre: 25,
  ozoneWidth: 15,
  /** How much light the ground throws back up. */
  groundAlbedo: 0.3,
  /** The height the sky is seen from, km: the game is near the ground and the sky does not move with the camera. */
  eye: 0.2,
}

/** WGSL constants matching `ATMOSPHERE`, for the shaders that build and read the sky. */
export const ATMOSPHERE_WGSL = /* wgsl */ `
const ATMO_BOTTOM = ${ATMOSPHERE.bottom.toFixed(1)};
const ATMO_TOP = ${ATMOSPHERE.top.toFixed(1)};
const ATMO_RAYLEIGH = vec3f(${ATMOSPHERE.rayleigh.map((v) => v.toExponential(4)).join(', ')});
const ATMO_RAYLEIGH_HEIGHT = ${ATMOSPHERE.rayleighHeight.toFixed(2)};
const ATMO_MIE_SCATTER = ${ATMOSPHERE.mieScatter.toExponential(4)};
const ATMO_MIE_EXTINCTION = ${ATMOSPHERE.mieExtinction.toExponential(4)};
const ATMO_MIE_HEIGHT = ${ATMOSPHERE.mieHeight.toFixed(2)};
const ATMO_MIE_G = ${ATMOSPHERE.mieG.toFixed(3)};
const ATMO_OZONE = vec3f(${ATMOSPHERE.ozone.map((v) => v.toExponential(4)).join(', ')});
const ATMO_OZONE_CENTRE = ${ATMOSPHERE.ozoneCentre.toFixed(1)};
const ATMO_OZONE_WIDTH = ${ATMOSPHERE.ozoneWidth.toFixed(1)};
const ATMO_GROUND_ALBEDO = ${ATMOSPHERE.groundAlbedo.toFixed(3)};
const ATMO_EYE = ${ATMOSPHERE.eye.toFixed(3)};
`

const A = ATMOSPHERE

/** Extinction per km at `h` km up, into `out` (r, g, b). */
function extinction(h: number, out: number[]) {
  const r = Math.exp(-h / A.rayleighHeight)
  const m = Math.exp(-h / A.mieHeight)
  const o = Math.max(0, 1 - Math.abs(h - A.ozoneCentre) / A.ozoneWidth)
  for (let i = 0; i < 3; i++) out[i] = A.rayleigh[i]! * r + A.mieExtinction * m + A.ozone[i]! * o
}

/** How far a ray from radius `r` with cosine `mu` to the vertical travels to the sphere of radius `R`, km, or -1. */
function toSphere(r: number, mu: number, R: number): number {
  const disc = r * r * (mu * mu - 1) + R * R
  if (disc < 0) return -1
  const s = Math.sqrt(disc)
  const near = -r * mu - s
  if (near > 0) return near
  const far = -r * mu + s
  return far > 0 ? far : -1
}

const ext = [0, 0, 0]

/**
 * What reaches a point `height` km up of light coming down at `cosZenith` from the top of the atmosphere,
 * per channel, into `out`: white overhead, red near the horizon, black when the ground is in the way.
 */
export function transmittance(cosZenith: number, out: number[], height: number = A.eye, steps = 24): number[] {
  const r = A.bottom + height
  if (toSphere(r, cosZenith, A.bottom) > 0) {
    out[0] = out[1] = out[2] = 0
    return out
  }
  const length = toSphere(r, cosZenith, A.top)
  const dt = length / steps
  let d0 = 0, d1 = 0, d2 = 0
  for (let i = 0; i < steps; i++) {
    const t = (i + 0.5) * dt
    const h = Math.sqrt(r * r + t * t + 2 * r * t * cosZenith) - A.bottom
    extinction(h, ext)
    d0 += ext[0]! * dt
    d1 += ext[1]! * dt
    d2 += ext[2]! * dt
  }
  out[0] = Math.exp(-d0)
  out[1] = Math.exp(-d1)
  out[2] = Math.exp(-d2)
  return out
}

const sunT = [0, 0, 0]

/**
 * The sky's radiance along `dir` (a unit vector, y up) with the sun toward `sun`, for a sun of
 * illuminance 1, single scattering only, into `out`. The GPU's tables add the light scattered many
 * times over, which this leaves out: it serves for colours and their changes through the day, not
 * for exact brightness.
 */
export function skyRadiance(dir: readonly number[], sun: readonly number[], out: number[], steps = 16): number[] {
  const r = A.bottom + A.eye
  const mu = dir[1]!
  const ground = toSphere(r, mu, A.bottom)
  const length = ground > 0 ? ground : toSphere(r, mu, A.top)
  const nu = dir[0]! * sun[0]! + dir[1]! * sun[1]! + dir[2]! * sun[2]!
  const rayleighPhase = (3 / (16 * Math.PI)) * (1 + nu * nu)
  const miePhase = cornetteShanks(nu, A.mieG)
  let t0 = 0, t1 = 0, t2 = 0
  let o0 = 0, o1 = 0, o2 = 0
  for (let i = 0; i < steps; i++) {
    // Samples crowd towards the eye, where the air is thick.
    const t = length * ((i + 0.5) / steps) ** 2
    const dt = length * (((i + 1) / steps) ** 2 - (i / steps) ** 2)
    const x = dir[0]! * t, y = r + dir[1]! * t, z = dir[2]! * t
    const rr = Math.hypot(x, y, z)
    const h = rr - A.bottom
    extinction(h, ext)
    const step0 = ext[0]! * dt, step1 = ext[1]! * dt, step2 = ext[2]! * dt
    // The sun's light at the sample, through what lies between it and the top.
    const cosSun = (x * sun[0]! + y * sun[1]! + z * sun[2]!) / rr
    transmittance(cosSun, sunT, h, 8)
    const rd = Math.exp(-h / A.rayleighHeight)
    const md = Math.exp(-h / A.mieHeight)
    const mie = A.mieScatter * md * miePhase
    const along0 = Math.exp(-(o0 + step0 / 2)), along1 = Math.exp(-(o1 + step1 / 2)), along2 = Math.exp(-(o2 + step2 / 2))
    t0 += along0 * sunT[0]! * (A.rayleigh[0] * rd * rayleighPhase + mie) * dt
    t1 += along1 * sunT[1]! * (A.rayleigh[1] * rd * rayleighPhase + mie) * dt
    t2 += along2 * sunT[2]! * (A.rayleigh[2] * rd * rayleighPhase + mie) * dt
    o0 += step0
    o1 += step1
    o2 += step2
  }
  out[0] = t0
  out[1] = t1
  out[2] = t2
  return out
}

/** Mie scattering's phase function, the Cornette-Shanks form of Henyey-Greenstein, for the cosine `nu` between the light and the view. */
export function cornetteShanks(nu: number, g: number): number {
  const k = (3 / (8 * Math.PI)) * ((1 - g * g) / (2 + g * g))
  return (k * (1 + nu * nu)) / (1 + g * g - 2 * g * nu) ** 1.5
}

/** `rgb` scaled so its brightest channel is 1: its hue and saturation without its strength. */
export function hueOf(rgb: readonly number[], out: Color): Color {
  const peak = Math.max(rgb[0]!, rgb[1]!, rgb[2]!, 1e-9)
  return out.set(rgb[0]! / peak, rgb[1]! / peak, rgb[2]! / peak)
}
