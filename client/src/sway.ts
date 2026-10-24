import { STAMINA_FULL } from './sim/consts'

/** How far a gun's aim wanders out of breath, radians at most; twice that while winded. */
export const SWAY_MAX = 0.035
/** How fast the wander's size follows stamina, per second, so it never jerks the view. */
const SWAY_EASE = 3

/** How far the aim wanders with `spent` stamina points used: nothing rested, growing by the square as it runs out. */
export function swayAmplitude(spent: number, winded: boolean): number {
  const fatigue = Math.min(1, Math.max(0, spent / STAMINA_FULL))
  return SWAY_MAX * fatigue * fatigue * (winded ? 2 : 1)
}

/** `current` eased `dt` seconds towards `target`. */
export function easeSway(current: number, target: number, dt: number): number {
  return current + (target - current) * Math.min(1, dt * SWAY_EASE)
}

const TAU = Math.PI * 2

/**
 * The `[yaw, pitch]` offset a wander of `amplitude` puts on the aim at `t` seconds: two sines per axis at
 * unrelated rates, so it drifts unsteadily and never visibly repeats. Each axis stays within `amplitude`.
 */
export function sway(t: number, amplitude: number): [number, number] {
  const yaw = 0.6 * Math.sin(TAU * 0.45 * t) + 0.4 * Math.sin(TAU * 0.73 * t + 1.3)
  const pitch = 0.6 * Math.sin(TAU * 0.61 * t + 0.7) + 0.4 * Math.sin(TAU * 1.07 * t + 2.1)
  return [amplitude * yaw, amplitude * 0.8 * pitch]
}
