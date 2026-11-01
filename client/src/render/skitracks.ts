import { Vector3 } from '@ragnarok/engine'
import { Skidmarks } from './skidmarks'
import type { Particles } from './particles'

/** The two grooves a skier leaves: how far apart, how wide and how dark, and how long they last, seconds. */
const STANCE = 0.3
const GROOVE = 0.09
const TINT = 0x9fb4cc
const ALPHA = 0.8
const LIFE = 300
const QUADS = 12000
/** Fastest there is no spray, m/s; puffs a second at speed, and how far from a skier anyone notices. */
const SPRAY_FROM = 8
const SPRAY_RATE = 26

const SIDE = new Vector3()
const AT = new Vector3()

/** What skiers leave in the snow: grooves that fade over minutes and snow flung up at speed. Seen, not simulated. */
export class SkiTracks {
  readonly marks = new Skidmarks(QUADS, LIFE)
  private readonly spray = new Map<number, number>()

  constructor(
    private readonly groundAt: (x: number, z: number) => number,
    private readonly smoke: Particles,
  ) {}

  /** Skier `key` is at `x, z` going `vx, vz`, this frame. A skier in the air or standing leaves nothing. */
  run(key: number, x: number, z: number, vx: number, vz: number, grounded: boolean, dt: number) {
    const speed = Math.hypot(vx, vz)
    if (!grounded || speed < 0.3) {
      this.lift(key)
      return
    }
    SIDE.set(vz / speed, 0, -vx / speed)
    for (const [track, offset] of [[`${key}L`, -STANCE / 2], [`${key}R`, STANCE / 2]] as const) {
      const [px, pz] = [x + SIDE.x * offset, z + SIDE.z * offset]
      AT.set(px, this.groundAt(px, pz), pz)
      this.marks.mark(track, AT, SIDE, GROOVE, TINT, ALPHA)
    }
    if (speed < SPRAY_FROM) return
    const owed = (this.spray.get(key) ?? 0) + SPRAY_RATE * dt
    this.spray.set(key, owed % 1)
    for (let n = 0; n < Math.floor(owed); n++) {
      const position = [x + (Math.random() - 0.5) * 0.4, this.groundAt(x, z) + 0.06, z + (Math.random() - 0.5) * 0.4] as const
      const velocity = [vx * 0.25 + (Math.random() - 0.5), 0.6 + Math.random() * 1.2, vz * 0.25 + (Math.random() - 0.5)] as const
      this.smoke.spawn({ position, velocity, life: 0.55, size: [0.12, 0.5], color: 0xffffff, alpha: 0.45, gravity: 3, drag: 1.5, shape: 'puff' })
    }
  }

  /** Skier `key` left the ground, or is gone: their next groove starts afresh. */
  lift(key: number) {
    this.marks.lift(`${key}L`)
    this.marks.lift(`${key}R`)
  }

  update(dt: number) {
    this.marks.update(dt)
  }

  dispose() {
    this.marks.dispose()
  }
}
