import { Color, Ramp } from '@ragnarok/engine'

/**
 * The terrain's toon ramp: eight steps over the half-Lambert coordinate (`N·L * 0.5 + 0.5`), dark and
 * coarse in shade and fine near the light. Flat ground under the sun (about 0.83) sits in the middle
 * of step 6, so a few degrees of slope tip it one step either way instead of flickering.
 */
export const TERRAIN_STEPS = [90, 100, 115, 140, 180, 208, 235, 255] as const
export const TERRAIN_RAMP = new Ramp(new Uint8Array(TERRAIN_STEPS))

/** The terrain ramp's level at `nDotL`, 0 to 1, as the shader reads it. */
export function terrainRamp(nDotL: number): number {
  const step = Math.floor(Math.min(0.999, Math.max(0, nDotL * 0.5 + 0.5)) * TERRAIN_STEPS.length)
  return TERRAIN_STEPS[step]! / 255
}

/**
 * WGSL for the light that falls on the terrain, for custom shaders that draw on or over it (grass,
 * water, skid marks) so they darken and colour with the time of day just as the ground does:
 * the sun (or moon) through the terrain's ramp, scaled by `shadow` (1 for none), the hemisphere fill
 * and nearby point lights. Multiply an unlit colour by `groundLight(...) / PI`, as the standard shader does.
 */
export const GROUND_LIGHT_WGSL = /* wgsl */ `
const TERRAIN_STEPS = array<f32, 8>(${TERRAIN_STEPS.map((s) => (s / 255).toFixed(4)).join(', ')});

fn groundLight(world: vec3f, n: vec3f, shadow: f32) -> vec3f {
  let step = u32(clamp(dot(n, frame.sunDir.xyz) * 0.5 + 0.5, 0.0, 0.999) * 8.0);
  // Copied to a variable first: WGSL only indexes a constant array with a constant.
  var steps = TERRAIN_STEPS;
  let hemi = mix(frame.hemiGround.rgb, frame.hemiSky.rgb, 0.5 * n.y + 0.5);
  return frame.sunColor.rgb * steps[step] * shadow + hemi + pointLight(world, n);
}
`

/** The light on flat ground from `sun` (already scaled by its strength) at height `up` and the hemisphere's sky fill, into `out`. */
export function flatGroundLight(sun: Color, up: number, hemiSky: Color, out: Color): Color {
  const ramp = terrainRamp(up)
  return out.set(sun.r * ramp + hemiSky.r, sun.g * ramp + hemiSky.g, sun.b * ramp + hemiSky.b)
}

/**
 * `color` (sRGB hex) as it looks under `tint`, the light on the ground against noon's per channel (see
 * `Sky.tint`), but never darker than `floor` of itself, so smoke and dust stay readable at night.
 */
export function tinted(color: number, tint: Color, floor = 0): number {
  if (tint.r >= 1 && tint.g >= 1 && tint.b >= 1) return color
  scratch.setHex(color)
  return scratch.set(scratch.r * Math.max(floor, tint.r), scratch.g * Math.max(floor, tint.g), scratch.b * Math.max(floor, tint.b)).getHex()
}

const scratch = new Color()
