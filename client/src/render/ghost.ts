import type { ParticleSpec } from './particles'

/** Somewhere particles can be spawned: the game's smoke and glow pools. */
interface Pool {
  spawn(spec: ParticleSpec): void
}

const rand = (low: number, high: number) => low + Math.random() * (high - low)
/** How many of something that comes `rate` times a second are due in `dt` seconds, rounded at random. */
const due = (rate: number, dt: number) => Math.floor(rate * dt + Math.random())

/** The smoke a ghost is made of: near black with a violet cast, so it reads as a shadow by day and a darker dark by night. */
const SHADOW = 0x0c0912
const SHADOW_END = 0x1d1430
/** The faint light at its rim and in its eyes. */
const RIM = 0x7a52ff
const EYES = 0xc7b0ff
/** Its eyes when the player looking out of them may not take shape here. */
const EYES_BLOCKED = 0xff5a5a
/** How high over the capsule centre a ghost's eyes are, metres, and how far apart. */
const EYE_HEIGHT = 0.55
const EYE_GAP = 0.13
/** How far ahead of the centre the eyes are, metres. */
const EYE_AHEAD = 0.35

/** A soldier's capsule centre is this far over their feet, metres: where a forming ghost's swirl sits. */
const FEET = 0.9

/**
 * Ghosts (GHOSTS.md), drawn as smoke: a churning shadow with a violet rim and two dim eyes, and a ghost
 * taking shape as a swirl closing in on the spot and rising into a column that the soldier stands up out
 * of. Nothing is kept between frames: each is a few short-lived particles laid where the ghost is now.
 */
export class GhostClouds {
  constructor(private readonly smoke: Pool, private readonly glow: Pool) {}

  /**
   * A ghost flying at `at` (its capsule centre) with `velocity` `[x, y, z]`, m/s, facing `yaw`, for the
   * `dt` seconds just passed. `own` is the local player's, which the camera sits behind: thinner, so it does
   * not hide the view. `blocked` shows red eyes: it may not take shape here.
   */
  flying(at: readonly [number, number, number], velocity: readonly [number, number, number], yaw: number, own: boolean, blocked: boolean, dt: number) {
    const [x, y, z] = at
    const [vx, vy, vz] = velocity
    for (let i = due(own ? 22 : 45, dt); i > 0; i--) {
      this.smoke.spawn({
        position: [x + rand(-0.35, 0.35), y + rand(-0.6, 0.6), z + rand(-0.35, 0.35)],
        // Dragged out behind as it goes, and curling up.
        velocity: [vx * 0.25 + rand(-0.4, 0.4), vy * 0.25 + rand(0, 0.5), vz * 0.25 + rand(-0.4, 0.4)],
        life: rand(0.6, 1.2), size: [rand(0.5, 0.8), rand(1.1, 1.6)], color: SHADOW, colorEnd: SHADOW_END,
        alpha: own ? 0.35 : 0.75, hold: true, gravity: -0.4, drag: 2.2, turbulence: 1.4, spin: [-1.2, 1.2], shape: 'puff',
      })
    }
    for (let i = due(own ? 4 : 10, dt); i > 0; i--) {
      this.glow.spawn({
        position: [x + rand(-0.5, 0.5), y + rand(-0.7, 0.7), z + rand(-0.5, 0.5)],
        velocity: [vx * 0.2 + rand(-0.3, 0.3), rand(0.1, 0.6), vz * 0.2 + rand(-0.3, 0.3)],
        life: rand(0.4, 0.8), size: [0.06, 0.02], color: RIM, glow: 1.6, alpha: 0.45, drag: 1.5, turbulence: 1.2,
      })
    }
    if (own) return
    const ahead = [-Math.sin(yaw) * EYE_AHEAD, -Math.cos(yaw) * EYE_AHEAD]
    const right = [Math.cos(yaw) * EYE_GAP, -Math.sin(yaw) * EYE_GAP]
    for (const side of [-1, 1]) {
      this.glow.spawn({
        position: [x + ahead[0]! + right[0]! * side, y + EYE_HEIGHT, z + ahead[1]! + right[1]! * side],
        velocity: [vx, vy, vz], life: Math.max(dt, 1 / 30) * 1.5, size: [0.07, 0.06], color: blocked ? EYES_BLOCKED : EYES, glow: 4, alpha: 0.9, shape: 'glare',
      })
    }
  }

  /**
   * A ghost taking shape at `at` (where the soldier's capsule centre will be), `done` of the way there,
   * 0 to 1, for the `dt` seconds just passed: smoke drawn in from a ring that tightens, spun round and
   * lifted into a column the height of a soldier.
   */
  forming(at: readonly [number, number, number], done: number, dt: number) {
    const [x, y, z] = at
    const feet = y - FEET
    const radius = 0.3 + 1.6 * (1 - done)
    for (let i = due(70 + 60 * done, dt); i > 0; i--) {
      const angle = Math.random() * Math.PI * 2
      const [c, s] = [Math.cos(angle), Math.sin(angle)]
      const swirl = 2.5
      const inward = 1.2
      this.smoke.spawn({
        position: [x + c * radius, feet + rand(0, 0.3 + 1.6 * done), z + s * radius],
        // Round the spot (the tangent), in towards it, and up.
        velocity: [-s * swirl - c * inward, rand(0.4, 1.4), c * swirl - s * inward],
        life: rand(0.5, 0.9), size: [rand(0.3, 0.5), rand(0.8, 1.2)], color: SHADOW, colorEnd: SHADOW_END,
        alpha: 0.85, hold: true, gravity: -0.6, drag: 1.2, turbulence: 0.8, spin: [-2, 2], shape: 'puff',
      })
    }
    for (let i = due(25 * done, dt); i > 0; i--) {
      this.glow.spawn({
        position: [x + rand(-0.3, 0.3), feet + rand(0, 1.8), z + rand(-0.3, 0.3)],
        velocity: [rand(-0.2, 0.2), rand(0.5, 1.5), rand(-0.2, 0.2)],
        life: rand(0.3, 0.6), size: [0.05, 0.02], color: RIM, glow: 2.5, alpha: 0.7, drag: 1,
      })
    }
  }
}
