import { Color, FlashEffects } from '@ragnarok/engine'
import { tinted } from './groundlight'
import { NIGHT_FLOOR } from './particles'

/** Anything with x, y and z: three's Vector3 or the engine's. */
type V = { x: number; y: number; z: number }
const sub = (a: V, b: V): V => ({ x: a.x - b.x, y: a.y - b.y, z: a.z - b.z })
const len = (a: V) => Math.hypot(a.x, a.y, a.z)
const add = (a: V, b: V, s = 1): V => ({ x: a.x + b.x * s, y: a.y + b.y * s, z: a.z + b.z * s })
const norm = (a: V): V => {
  const l = len(a) || 1
  return { x: a.x / l, y: a.y / l, z: a.z / l }
}
const spread = (): V => ({ x: Math.random() - 0.5, y: Math.random() - 0.5, z: Math.random() - 0.5 })

/**
 * Short-lived shot effects in world space: a muzzle flash, a tracer along the shell's path and a
 * ball of fire where it lands. Each piece is one record on the GPU (`FlashEffects`), a formula of its
 * age that the vertex shader evaluates, so nothing is allocated or touched after it is spawned. Sizes
 * are the radius a ball ends up with; `bright` pieces ignore fog, so tracers and sparks keep their
 * colour against a pale sky and at range.
 */
export class Effects {
  readonly root: FlashEffects
  /** The light on the ground against noon's (`Sky.tint`): smoke and dust are not lit, so they are tinted by it. */
  readonly light = new Color()

  constructor(capacity = 8192) {
    this.root = new FlashEffects(capacity)
  }

  /** A shell leaving the barrel: a flash and a short puff along it. The shell is drawn in flight. */
  muzzleFlash(muzzle: V, direction: V) {
    const f = this.root
    f.spawn({ at: muzzle, life: 0.12, size: 0.5, grow: 0.8, color: 0xffe9a0 })
    f.spawn({ at: add(muzzle, norm(direction), 1), life: 0.25, size: 0.3, grow: 0.9, color: 0xffc060 })
  }

  /** A rifle shot: a short streak flies from the muzzle to `to` and, when it ended on something, sparks there. */
  tracer(from: V, to: V, hit: boolean) {
    const length = len(sub(to, from))
    if (length < 0.01) return
    const direction = norm(sub(to, from))
    this.root.spawn({ at: add(from, direction, 0.6), life: 0.06, size: 0.1, grow: 0.1, color: 0xfff2b0, bright: true })
    // Thin, and a short segment on the move rather than a rod drawn the whole way: it reads as speed.
    const start = Math.min(1.5, length)
    const run = length - start
    if (run > 1) {
      const streak = Math.min(5, run)
      const life = Math.min(0.4, Math.max(0.05, run / 350))
      const at = add(from, direction, start + streak / 2)
      for (const [width, color] of [[0.2, 0xfff6d8], [0.55, 0xff9a2e]] as const) {
        this.root.spawn({ at, direction, along: run - streak, length: streak, size: 0.06 * width, life, color, bright: true, fade: 'late' })
      }
    }
    if (hit) this.sparks(to, 5, 0.9)
  }

  /** A flash and a spray of `count` bright specks thrown from `at`, drawn for `reach` of the usual reach. */
  private sparks(at: V, count: number, reach: number) {
    this.root.spawn({ at, life: 0.08, size: 0.08, grow: 0.14, color: 0xffffff, bright: true })
    for (let i = 0; i < count; i++) {
      const direction = norm({ x: Math.random() - 0.5, y: Math.random() * 0.9 + 0.1, z: Math.random() - 0.5 })
      const speed = (3 + Math.random() * 4) * reach
      const size = 0.015 + Math.random() * 0.02
      this.root.spawn({ at, direction, along: speed * 0.3, drop: 1.8, size, grow: -0.6 * size, life: 0.18 + Math.random() * 0.14, color: i % 2 ? 0xffd070 : 0xff8a1f, bright: true })
    }
  }

  /** A shotgun going off: a fat flash, flame thrown forward in a cone and a wisp of smoke. No rays. */
  blast(muzzle: V, direction: V) {
    const d = norm(direction)
    this.root.spawn({ at: add(muzzle, d, 0.5), life: 0.1, size: 0.35, grow: 0.35, color: 0xfff0b8, bright: true })
    for (let i = 0; i < 7; i++) {
      const jet = norm(add(d, spread(), 0.5))
      const size = 0.08 + Math.random() * 0.1
      this.root.spawn({ at: muzzle, direction: jet, sqrtAlong: 1.5 + Math.random() * 2, size, grow: -0.5 * size, life: 0.1 + Math.random() * 0.08, color: i % 2 ? 0xffb347 : 0xff7a1a, bright: true })
    }
    this.root.spawn({ at: add(muzzle, d, 0.8), direction: d, along: 1.2, lift: 0.4, size: 0.15, grow: 0.35, life: 0.5, color: 0x8a8a8a })
  }

  /** One shotgun pellet landing: a few sparks and a small puff of dust. */
  pelletStrike(at: V) {
    this.sparks(at, 3, 0.7)
    this.root.spawn({ at, lift: 0.3, size: 0.1, grow: 0.3, life: 0.4, color: 0xb8ad98 })
  }

  /** A round striking something: a spark and a puff. */
  strike(at: V) {
    this.root.spawn({ at, life: 0.22, size: 0.15, grow: 0.4, color: 0xffa21f, bright: true })
  }

  /**
   * Debris flung from where a round hit: red flecks off a body, sparks off a vehicle. A kill throws
   * more, harder.
   */
  impact(at: V, kind: 'body' | 'vehicle' | 'kill', away: V | null = null) {
    const count = kind === 'kill' ? 12 : 6
    const color = kind === 'vehicle' ? 0xffc040 : 0xff2a22
    for (let i = 0; i < count; i++) {
      let direction = norm({ x: Math.random() - 0.5, y: Math.random() * 0.8, z: Math.random() - 0.5 })
      if (away) direction = norm(add(direction, away, 0.8))
      const speed = (kind === 'kill' ? 4.5 : 3) * (0.5 + Math.random())
      const size = (kind === 'vehicle' ? 0.05 : 0.08) * (0.7 + Math.random() * 0.8) * (kind === 'kill' ? 1.4 : 1)
      this.root.spawn({ at, direction, along: speed * 0.5, drop: 2, size, grow: -0.5 * size, life: 0.28 + Math.random() * 0.2, color, bright: true })
    }
    this.root.spawn({ at, life: 0.09, size: 0.12, grow: 0.3, color: 0xffffff, bright: true })
  }

  /**
   * A round going through armour at `at`, on a plate facing `outward`: a white-hot flash, molten
   * fragments blown back out of the hole and a wisp of smoke from it. `big` for a shell, which throws
   * more and further, against a bullet through a slit or a thin plate.
   */
  pierce(at: V, outward: V, big: boolean) {
    const out = norm(outward)
    const scale = big ? 1 : 0.45
    this.root.spawn({ at, life: 0.1, size: 0.25 * scale, grow: 0.5 * scale, color: 0xffffff, bright: true })
    this.root.spawn({ at, life: 0.35, size: 0.18 * scale, grow: 0.35 * scale, color: 0xff8a2a, bright: true })
    const count = big ? 16 : 6
    for (let i = 0; i < count; i++) {
      const direction = norm(add(out, spread(), 1.4))
      const speed = (4 + Math.random() * 6) * scale
      const size = (0.02 + Math.random() * 0.03) * (big ? 1.3 : 1)
      this.root.spawn({ at, direction, along: speed * 0.45, drop: 3, size, grow: -0.6 * size, life: 0.3 + Math.random() * 0.35, color: i % 3 ? 0xffb347 : 0xfff0c0, bright: true })
    }
    this.root.spawn({ at: add(at, out, 0.15), lift: 0.8, size: 0.12 * scale, grow: 0.6 * scale, life: 1.6, color: tinted(0x2c2a28, this.light, NIGHT_FLOOR) })
  }

  /** Armour stopping a round at `at`, on a plate facing `outward`: sparks glancing off and a puff of grit. */
  deflect(at: V, outward: V) {
    const out = norm(outward)
    this.root.spawn({ at, life: 0.07, size: 0.08, grow: 0.12, color: 0xffffff, bright: true })
    for (let i = 0; i < 5; i++) {
      const direction = norm(add(out, spread(), 2))
      const speed = 3 + Math.random() * 4
      const size = 0.012 + Math.random() * 0.015
      this.root.spawn({ at, direction, along: speed * 0.3, drop: 1.5, size, grow: -0.6 * size, life: 0.15 + Math.random() * 0.12, color: i % 2 ? 0xffe3a0 : 0xffa040, bright: true })
    }
    this.root.spawn({ at, lift: 0.3, size: 0.06, grow: 0.25, life: 0.5, color: tinted(0x9a948a, this.light, NIGHT_FLOOR) })
  }

  /** The white-hot flash of a blast at `at`; the fire, smoke and debris around it come from `explosion`. */
  burst(at: V, radius: number) {
    this.root.spawn({ at, life: 0.25, size: radius * 0.12, grow: radius * 0.3, color: 0xff9a3c, bright: true })
    this.root.spawn({ at, life: 0.12, size: radius * 0.1, grow: radius * 0.2, color: 0xfff0c0, bright: true })
  }

  /** A ring of pale light racing out over the ground from a blast at `at` (already on the ground) to ~7 radii. */
  shockwave(at: V, radius: number) {
    this.root.spawn({ at, ring: true, life: 0.45 + radius * 0.04, size: radius * 0.3, grow: radius * 6, length: radius * 1.1, color: 0xffe9c0, opacity: 0.7, bright: true })
    // A fainter, slower echo of dust behind it.
    this.root.spawn({ at, ring: true, life: 0.8 + radius * 0.05, size: radius * 0.2, grow: radius * 4, length: radius * 1.6, color: 0xcbbf9f, opacity: 0.35 })
  }

  /** Rings spreading over the water from a splash of about `size` metres at `at` (on the surface). */
  ripple(at: V, size: number) {
    const rings = size > 1.5 ? 3 : size > 0.4 ? 2 : 1
    for (let i = 0; i < rings; i++) {
      this.root.spawn({ at, ring: true, life: 0.9 + size * 0.1 + i * 0.25, size: size * 0.3 + i * size * 0.15, grow: 1.5 + size * 1.6, length: 0.12 + size * 0.06, color: 0xf0faff, opacity: 0.55 - i * 0.12 })
    }
  }

  /** A puff of smoke from a damaged vehicle, drifting up. Black and bigger for a wreck. */
  smoke(at: V, wreck: boolean) {
    const size = wreck ? 1.1 : 0.7
    this.root.spawn({ at, lift: 3, size: size * 0.5, grow: size, life: wreck ? 1.4 : 1.0, color: tinted(wreck ? 0x222222 : 0x777777, this.light, NIGHT_FLOOR) })
  }

  /** A cloud of dust of about `size` metres across at `at`, swelling and rising as it thins. */
  dust(at: V, size: number) {
    this.root.spawn({ at, lift: size * 0.5, size: size * 0.35, grow: size * 0.65, life: 2.2, color: tinted(0xcbc2b0, this.light, NIGHT_FLOOR) })
  }

  /** Just the muzzle flash: the local player's own shot before the server confirms it, or a machine gun's round. */
  muzzle(at: V) {
    this.root.spawn({ at, life: 0.06, size: 0.12, grow: 0.1, color: 0xfff2b0 })
  }

  update(dt: number) {
    this.root.update(dt)
  }

  /** How many are alive, for tests. */
  get count() {
    return this.root.count
  }

  dispose() {
    this.root.removeFromParent()
  }
}
