import type { Color, Vec3 } from '@ragnarok/engine'
import { Particles } from './particles'

/** How many particles the roost can hold: a few sleds pulling away at once, or a pair of tanks. */
const POOL = 6144

/** How a vehicle throws its snow (`KindFx.roost`), past where on its track it leaves. */
export interface RoostStyle {
  /** How wide the plume starts, metres. */
  width: number
  /** How fast its snow goes up at full strength, m/s: a sled's track throws it high, a tank's lays it out low. */
  up: number
  /** How much of it there is. */
  power: number
}

/** A whole number of particles for `rate` a second over `dt`, carrying the fraction by chance. */
const count = (rate: number, dt: number) => Math.floor(rate * dt + Math.random())

/**
 * The snow a track throws up when it digs in, in a winter world: a rooster tail of clumps arcing up
 * and back off the track, a powder haze that hangs in the air behind, clods that land and skitter,
 * and ice crystals glittering in the sun. Its own pool, as the falling snow has, so that a few sleds
 * pulling away cannot crowd out smoke and dust; the glitter is light, so it goes in the additive pool.
 */
export class SnowRoost {
  private readonly pool = new Particles(POOL, false, true)

  constructor(
    private readonly glow: Particles,
    groundAt: (x: number, z: number) => number,
  ) {
    this.pool.groundAt = groundAt
  }

  get root() {
    return this.pool.root
  }

  /** How many particles of snow are in the air. */
  get alive() {
    return this.pool.alive
  }

  /** The light on the ground (`Particles.light`): it dims the snow at dusk, and the glitter needs sun. */
  get light(): Color {
    return this.pool.light
  }

  /**
   * Snow off the track at `at` (on the ground) for `dt` seconds, at `strength` 0..1. `back` is the
   * unit direction behind the vehicle, `carried` its velocity, which the snow leaves with.
   */
  throw(at: Vec3, back: Vec3, carried: Vec3, speed: number, strength: number, style: RoostStyle, dt: number) {
    if (strength <= 0) return
    const amount = strength * style.power
    const w = style.width
    // Thrown back faster the faster the track turns, and up as hard as the track can fling it.
    const fling = 4 + Math.abs(speed) * 0.25
    const up = style.up * (0.6 + strength * 0.4)
    const out = (along: number, rise: number, keep: number) =>
      [back.x * along + carried.x * keep, rise + carried.y * keep, back.z * along + carried.z * keep] as const
    const from = (lift: number) => [at.x + back.x * 0.2, at.y + lift, at.z + back.z * 0.2] as const

    // The plume: dense clumps that arc up and back and break up as they fall.
    const plume = count(170 * amount, dt)
    if (plume > 0) {
      this.pool.emit({
        count: plume,
        position: from(0.2),
        spread: w * 0.4,
        velocity: out(fling, up, 0.6),
        jitter: [w * 2.2, up * 0.35, w * 2.2],
        life: [0.9, 1.7],
        size: [0.25, 1.3],
        scale: [0.7, 1.4],
        color: 0xffffff,
        color2: 0xcfdcea,
        alpha: 0.85,
        gravity: 9,
        drag: 0.7,
        spin: [-1.5, 1.5],
        shape: 'puff',
      })
    }
    // The powder: a fine haze that hangs and drifts in the air the vehicle leaves behind.
    const haze = count(26 * amount, dt)
    if (haze > 0) {
      this.pool.emit({
        count: haze,
        position: from(0.6),
        spread: w,
        velocity: out(fling * 0.4, up * 0.25, 0.3),
        jitter: [1.4, 0.6, 1.4],
        life: [2, 3.4],
        size: [1, 4.5],
        scale: [0.8, 1.3],
        color: 0xf4f8ff,
        color2: 0xdde6f0,
        alpha: 0.28,
        gravity: 0.15,
        drag: 1.6,
        spin: [-0.4, 0.4],
        shape: 'puff',
      })
    }
    // Clods: packed lumps that fly further, land and skitter.
    const clods = count(22 * amount, dt)
    if (clods > 0) {
      this.pool.emit({
        count: clods,
        position: from(0.15),
        spread: w * 0.3,
        velocity: out(fling * 0.9, up * 0.75, 0.6),
        jitter: [w * 1.6, up * 0.3, w * 1.6],
        life: [0.8, 1.5],
        size: [0.12, 0.08],
        scale: [0.6, 1.6],
        color: 0xeef3f8,
        alpha: 1,
        gravity: 9.8,
        bounce: true,
        shape: 'round',
      })
    }
    // Ice crystals catching the sun: only by day, and brighter the brighter it is.
    const light = this.pool.light
    const sun = Math.max(0, Math.min(1, ((light.r + light.g + light.b) / 3 - 0.3) / 0.7))
    const glitter = count(70 * amount * sun, dt)
    if (glitter > 0) {
      this.glow.emit({
        count: glitter,
        position: from(0.3),
        spread: w * 0.6,
        velocity: out(fling, up * 0.9, 0.6),
        jitter: [w * 3, up * 0.5, w * 3],
        life: [0.4, 0.9],
        size: [0.07, 0.02],
        color: 0xffffff,
        color2: 0xcfe6ff,
        alpha: 1,
        gravity: 5,
        drag: 0.8,
        stretch: 0.015,
        glow: 1 + 1.5 * sun,
        shape: 'glare',
      })
    }
  }

  /** Steps the snow in the air, keeping the ground the clods land on round `eye`. */
  update(eye: { x: number; z: number }, dt: number) {
    this.pool.follow(eye.x, eye.z)
    this.pool.update(dt)
  }

  dispose() {
    this.pool.dispose()
  }
}
