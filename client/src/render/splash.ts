import type { FxWorld } from './tyres'

/** Anything with x, y and z. */
type V = { x: number; y: number; z: number }

const rand = (low: number, high: number) => low + Math.random() * (high - low)

/** Splashes further from the camera than this, metres, are skipped. */
const RANGE = 120
/** Most droplets one splash throws. */
const MAX_DROPS = 40
const SPRAY = 0xeaf5ff
const MIST = 0xf2fbff

/**
 * Water thrown up where something lands in it: a crown of droplets, a little mist for the big ones,
 * and rings running out over the surface. `at` is on the water surface and `size` is about the radius
 * of what hit it, metres (0.3 for a bullet, 0.5 for a boot, 5 for a shell). `speed` is how hard it
 * came down, metres per second, and throws the crown higher. Particles only, so nothing to clean up.
 */
export function splash(world: FxWorld, at: V, size: number, speed = 0) {
  const eye = world.camera
  if (Math.hypot(eye.x - at.x, eye.y - at.y, eye.z - at.z) > RANGE) return
  // A crowded room thins its splashes before the pools start evicting each other.
  const alive = world.smoke.alive
  const budget = alive > 1500 ? 0.4 : alive > 800 ? 0.65 : 1
  const rise = 2.5 + size * 2.5 + Math.min(speed, 20) * 0.3

  const drops = Math.min(MAX_DROPS, Math.max(2, Math.round((5 + size * 9) * budget)))
  for (let i = 0; i < drops; i++) {
    const heading = rand(0, Math.PI * 2)
    // Most of the crown goes up and a little out, the rest is a low skirt.
    const out = rand(0.2, 1) * (0.6 + size * 0.9)
    world.smoke.spawn({
      position: [at.x + Math.cos(heading) * size * 0.3, at.y + 0.05, at.z + Math.sin(heading) * size * 0.3],
      velocity: [Math.cos(heading) * out, rise * rand(0.45, 1), Math.sin(heading) * out],
      life: rand(0.5, 0.5 + rise * 0.1),
      size: [0.07 + size * 0.03, 0.04],
      color: SPRAY,
      alpha: 0.85,
      gravity: 9.8,
    })
  }

  // Mist and spume that hangs after the droplets fall.
  const mist = size < 0.6 ? (Math.random() < 0.5 ? 1 : 0) : Math.round((2 + size) * budget)
  for (let i = 0; i < mist; i++) {
    const heading = rand(0, Math.PI * 2)
    const out = rand(0, size * 0.6)
    world.smoke.spawn({
      position: [at.x + Math.cos(heading) * out * 0.4, at.y + 0.1, at.z + Math.sin(heading) * out * 0.4],
      velocity: [Math.cos(heading) * out, rise * rand(0.2, 0.5), Math.sin(heading) * out],
      life: rand(0.7, 1.2),
      size: [size * 0.3 + 0.2, size * rand(0.8, 1.3) + 0.4],
      color: MIST,
      alpha: 0.35,
      shape: 'puff',
      spin: [-1, 1],
      gravity: -0.4,
      drag: 2,
    })
  }

  world.ripple?.(at, size)
}
