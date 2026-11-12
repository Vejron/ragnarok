import { Particles } from './particles'

/** Half the side of the box of air round the camera that drops start in, and how high above the eye it reaches, metres. */
const REACH = 16
const HEIGHT = 14
/** Drops a second in the hardest rain, how fast they fall (m/s), and how long one lives: long enough to fall past the eye. */
const RATE = 2400
const SPEED = 10
const LIFE = 1.5
const POOL = Math.ceil(RATE * LIFE * 1.1)

/** A drop: a thin streak along its fall, its length from its speed (metres per m/s). */
const DROP = { life: LIFE, size: [0.016, 0.016] as [number, number], color: 0xc9d3de, alpha: 0.42, stretch: 0.07, shape: 'round' as const }

/** Rain falling round the camera, as many drops as `amount` (0 to 1) says, in a pool of its own. */
export class Rainfall {
  private readonly pool = new Particles(POOL, false)
  private owed = 0

  get root() {
    return this.pool.root
  }

  /** The light on the ground against noon's (`Sky.tint`), so the drops darken with the evening. */
  get light() {
    return this.pool.light
  }

  /** `wind` is the drift of the drops, m/s along x and z. */
  update(eye: { x: number; y: number; z: number }, amount: number, wind: readonly [number, number], dt: number) {
    this.owed = Math.min(this.owed + RATE * amount * dt, RATE * 0.1)
    for (; this.owed >= 1; this.owed--) {
      const speed = SPEED * (0.85 + Math.random() * 0.3)
      // Started upwind, so they drift across where the eye is as they fall.
      const back = (Math.random() * HEIGHT) / SPEED
      const position = [
        eye.x + (Math.random() * 2 - 1) * REACH - wind[0] * back,
        eye.y + 2 + Math.random() * HEIGHT,
        eye.z + (Math.random() * 2 - 1) * REACH - wind[1] * back,
      ] as const
      this.pool.spawn({ ...DROP, position, velocity: [wind[0], -speed, wind[1]] })
    }
    this.pool.follow(eye.x, eye.z)
    this.pool.update(dt)
  }

  dispose() {
    this.pool.dispose()
  }
}
