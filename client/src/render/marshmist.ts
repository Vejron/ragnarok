import type { Color } from '@ragnarok/engine'
import { surfaceWetness } from '@/sim/surface'
import { Particles } from './particles'

/** Metres round the camera mist rises in. */
const REACH = 55
/** Places tried a second at full strength: only those on a marsh grow mist, so a bigger marsh is mistier. */
const TRIES = 140
/** Seconds a bank of mist hangs. */
const LIFE = 14
const POOL = 192
/** Mist's colour, before the light of the hour tints it. */
const MIST = 0xe4eaec

/**
 * How thick the mist over a marsh is at `phase` of the day (`dayPhase`), 0 to 1: thickest about sunrise
 * and sunset, middling through the night, a wisp at noon.
 */
export function mistAt(phase: number): number {
  const sun = Math.sin(2 * Math.PI * (phase - 0.25))
  return Math.max(sun < 0 ? 0.55 : 0.08, Math.exp(-((sun / 0.22) ** 2)))
}

/**
 * Low mist hanging over the marshes near the camera: big, faint, slow puffs a metre off the ground, in
 * a pool of its own, drawn at half resolution as smoke is. Nothing spawns away from a marsh.
 */
export class MarshMist {
  private readonly pool = new Particles(POOL, false, true)
  private owed = 0

  get root() {
    return this.pool.root
  }

  /**
   * `strength` is `mistAt` now, `light` the ground's light (`Scene.groundLight`), and `surfaceAt` and
   * `groundAt` the world's.
   */
  update(eye: { x: number; z: number }, dt: number, strength: number, light: Color, surfaceAt: (x: number, z: number) => number, groundAt: (x: number, z: number) => number) {
    // Mist scatters the sky's light as well as the sun's: paler than the ground in the low light it hangs in.
    this.pool.light.set(Math.min(1, light.r * 1.6 + 0.08), Math.min(1, light.g * 1.6 + 0.08), Math.min(1, light.b * 1.6 + 0.1))
    this.owed = Math.min(this.owed + TRIES * strength * dt, TRIES)
    for (; this.owed >= 1; this.owed--) {
      const angle = Math.random() * Math.PI * 2
      const out = Math.sqrt(Math.random()) * REACH
      const x = eye.x + Math.cos(angle) * out
      const z = eye.z + Math.sin(angle) * out
      const wet = surfaceWetness(surfaceAt(x, z))
      if (wet === 0) continue
      const size = 4 + Math.random() * 3
      this.pool.spawn({
        position: [x, groundAt(x, z) + 0.3 + Math.random() * 0.9, z],
        // A breath of air carries it, and it spreads as it goes.
        velocity: [0.25 + (Math.random() - 0.5) * 0.3, 0.02, 0.15 + (Math.random() - 0.5) * 0.3],
        life: LIFE * (0.7 + Math.random() * 0.5),
        size: [size, size * 1.6],
        color: MIST,
        alpha: 0.75 * (0.5 + 0.5 * wet) * Math.min(1, 0.4 + strength),
        shape: 'puff',
        spin: [-0.05, 0.05],
      })
    }
    this.pool.follow(eye.x, eye.z)
    this.pool.update(dt)
  }

  dispose() {
    this.pool.dispose()
  }
}
