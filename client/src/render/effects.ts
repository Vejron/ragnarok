import { Color, FlashEffects } from '@ragnarok/engine'
import { dustCloud, type ParticleSpec, type Particles } from './particles'

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
const rand = (low: number, high: number) => low + Math.random() * (high - low)
const pick = <T>(items: readonly T[]) => items[Math.floor(Math.random() * items.length)]!

/** The drift the smoke leans with, m/s: the vehicles' smoke's wind (`vehiclesmoke.ts`). */
const WIND = [1.1, 0.5] as const
const SOOT = [0x161514, 0x0f0e0e, 0x1f1c1a] as const
const GRIT = [0xb8ad98, 0x9a948a, 0xa89f8c] as const
const SPARK = [0xffffff, 0xffe08a, 0xffa23a, 0xff6a1a] as const
/** A laser's beam, HDR so that it blooms: the core white-hot, and the sheath round it a hard saturated red. */
const BEAM_CORE = new Color().set(7, 3.5, 2.6)
const BEAM_HOT = new Color().set(4, 0.16, 0.05)
const BEAM_HAZE = new Color().set(1.2, 0.02, 0.01)
const FLARE = new Color().set(2.5, 0.5, 0.3)

type Pool = Pick<Particles, 'spawn'>

/**
 * Short-lived shot effects in world space. Flashes, sparks, fire and smoke are particles, the same GPU
 * pools a vehicle's plume and a blast use: `glow` for light, `smoke` for what blocks it and `blood` for
 * what comes out of a body. Spawning one writes a record and the GPU does the rest. Tracers, a laser's
 * beam, its lens flares and rings on the ground are records of `FlashEffects`, a formula of their age,
 * as a streak has no use for a particle's motion.
 */
export class Effects {
  readonly root: FlashEffects
  private readonly smoke: Pool
  private readonly glow: Pool
  private readonly blood: Pool

  constructor(pools: { smoke: Pool; glow: Pool; blood: Pool }, capacity = 4096) {
    this.root = new FlashEffects(capacity)
    this.smoke = pools.smoke
    this.glow = pools.glow
    this.blood = pools.blood
  }

  /** A shell leaving the barrel: a flash, a tongue of flame along it and a roll of smoke. The shell is drawn in flight. */
  muzzleFlash(muzzle: V, direction: V) {
    const d = norm(direction)
    this.flash(muzzle, 2.4, 0.12, 0xffe9a0, 8)
    this.flame(add(muzzle, d, 1), [d.x * 7, d.y * 7, d.z * 7], 0.25, [0.5, 1.6], 0xffc060)
    for (let i = 0; i < 4; i++) {
      const v = add(d, spread(), 0.7)
      this.drift(add(muzzle, d, rand(0.5, 1.5)), [v.x * 4, v.y * 4 + 0.6, v.z * 4], rand(1, 1.8), [0.5, rand(1.8, 2.6)], 0x8a8885, 0.35)
    }
  }

  /** A rifle shot: a short streak flies from the muzzle to `to` and, when it ended on something, sparks there. */
  tracer(from: V, to: V, hit: boolean) {
    const length = len(sub(to, from))
    if (length < 0.01) return
    const direction = norm(sub(to, from))
    this.flash(add(from, direction, 0.6), 0.45, 0.06, 0xfff2b0, 6)
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

  /** One frame of laser eyes charging at `eye`, `share` of the way: motes of light drawn in, a swelling glare, and arcs near the end. */
  laserCharge(eye: V, share: number, dt: number) {
    this.flash(eye, 0.6 + share * share * 3, 0.06, 0xff2a10, 3 + share * 2)
    this.flash(eye, 0.25 + share * share, 0.06, 0xfff0e0, 5)
    const motes = Math.round((30 + share * 150) * dt) + (Math.random() < 0.5 ? 1 : 0)
    for (let i = 0; i < motes; i++) {
      const direction = norm(spread())
      const reach = 1 + Math.random() * 2.5
      const life = 0.22 + Math.random() * 0.12
      this.root.spawn({ at: add(eye, direction, reach), direction: { x: -direction.x, y: -direction.y, z: -direction.z }, along: reach, length: 0.25 + share * 0.5, size: 0.025, life, color: i % 2 ? 0xffb060 : 0xff3a18, bright: true })
    }
    if (share > 0.6 && Math.random() < share * dt * 30) {
      const direction = norm(spread())
      this.root.spawn({ at: add(eye, direction, 0.45), direction, length: 0.9, size: 0.02, life: 0.05, color: 0xffe8d8, bright: true })
    }
  }

  /** The instant laser eyes light: a white-hot burst, a lens flare across the sky and a ball of sparks and smoke thrown off. */
  laserIgnite(eye: V) {
    this.flash(eye, 6, 0.35, 0xff3a18, 3)
    this.flash(eye, 2.4, 0.2, 0xffffff, 5)
    this.lensFlare(eye, 9, 0.3)
    for (let i = 0; i < 40; i++) {
      const direction = norm(spread())
      const speed = rand(8, 24)
      this.spark(eye, [direction.x * speed, direction.y * speed, direction.z * speed], rand(0.3, 0.65), rand(0.05, 0.09), i)
    }
    for (let i = 0; i < 5; i++) {
      const direction = norm(spread())
      this.drift(eye, [direction.x * 3, direction.y * 3 + 1, direction.z * 3], rand(1, 1.8), [0.3, rand(1.6, 2.4)], pick(SOOT), 0.4)
    }
  }

  /** One frame of a laser's beam from `from` to `to`: a white core in a red-hot sheath that throbs, with a flare in the eye and sparks along it. */
  laserBeam(from: V, to: V, dt: number) {
    const length = len(sub(to, from))
    if (length < 0.01) return
    const direction = norm(sub(to, from))
    const at = add(from, direction, length / 2)
    const throb = 1 + Math.sin(performance.now() / 22) * 0.18 + (Math.random() - 0.5) * 0.25
    const life = Math.max(0.04, dt * 1.6)
    this.root.spawn({ at, direction, length, size: 1.1 * throb, life, color: BEAM_HAZE, opacity: 0.3, bright: true, soft: true, fade: 'late' })
    this.root.spawn({ at, direction, length, size: 0.6 * throb, life, color: BEAM_HOT, opacity: 0.9, bright: true, soft: true, fade: 'late' })
    this.root.spawn({ at, direction, length, size: 0.22 * throb, life, color: BEAM_CORE, bright: true, soft: true, fade: 'late' })
    this.flash(from, 2.4 * throb, life, 0xff3010, 3)
    this.flash(from, 0.9 * throb, life, 0xffffff, 5)
    this.lensFlare(from, 4.5 * throb, life)
    if (Math.random() < dt * 40) {
      const spot = add(from, direction, Math.random() * Math.min(length, 80))
      this.spark(spot, [rand(-2, 2), rand(0, 3), rand(-2, 2)], rand(0.3, 0.5), 0.07, 2)
    }
  }

  /** Where a laser burns, every few hundredths of a second: a white-hot glare, a spray of molten metal, fire, and rising black smoke. */
  laserBurn(at: V) {
    const above = add(at, { x: 0, y: 1, z: 0 }, 0.25)
    this.flash(above, 3.4, 0.16, 0xff4a18, 3)
    this.flash(above, 1.2, 0.1, 0xffffff, 5)
    this.lensFlare(above, 3.5, 0.1)
    this.sparks(above, 14, 2.2)
    for (let i = 0; i < 6; i++) {
      const heading = rand(0, Math.PI * 2)
      const out = rand(2, 7)
      this.glow.spawn({ position: above, velocity: [Math.cos(heading) * out, rand(4, 10), Math.sin(heading) * out], life: rand(0.8, 1.5), size: [rand(0.1, 0.18), 0.03], color: 0xffc060, colorEnd: 0xff3a05, glow: 6, stretch: 0.02, alpha: 1, gravity: 9.8, drag: 0.4, bounce: true })
    }
    for (let i = 0; i < 3; i++) this.flame(add(above, spread(), 0.8), [rand(-0.6, 0.6), rand(1.5, 3.5), rand(-0.6, 0.6)], rand(0.35, 0.6), [0.5, rand(1.4, 2)], i ? 0xff8a2a : 0xffc060)
    for (let i = 0; i < 6; i++) {
      this.smoke.spawn({
        position: add(above, spread(), 1.2),
        velocity: [WIND[0] * rand(0.4, 1) + rand(-0.6, 0.6), rand(2.2, 4.2), WIND[1] * rand(0.4, 1) + rand(-0.6, 0.6)],
        life: rand(3.2, 5.2),
        size: [rand(0.5, 0.8), rand(3.4, 5.4)],
        color: pick(SOOT),
        colorEnd: 0x55504a,
        alpha: 0.85,
        gravity: -0.9,
        drag: 0.55,
        spin: [-0.6, 0.6],
        shape: 'puff',
      })
    }
  }

  /** Heat coming off laser eyes that have just burnt. */
  laserSmoke(eye: V) {
    this.drift(add(eye, spread(), 0.2), [WIND[0] * 0.4, rand(1, 2), WIND[1] * 0.4], rand(1, 1.6), [0.2, rand(1, 1.5)], 0x5a5656, 0.4)
  }

  /** A lens flare at `at`: a long level streak and a short upright one, `reach` metres out either way. */
  private lensFlare(at: V, reach: number, life: number) {
    for (const [direction, length, size] of [[{ x: 1, y: 0, z: 0 }, reach * 2, 0.09], [{ x: 0, y: 0, z: 1 }, reach * 2, 0.09], [{ x: 0, y: 1, z: 0 }, reach, 0.07]] as const) {
      this.root.spawn({ at, direction, length, size, life, color: FLARE, bright: true, soft: true })
    }
  }

  /** Light `size` metres across at `at`, gone in `life` seconds and `glow` times over bright enough to bloom. */
  private flash(at: V, size: number, life: number, color: number, glow: number) {
    this.glow.spawn({ position: at, velocity: [0, 0, 0], life, size: [size * 0.7, size], color, glow, alpha: 1, shape: 'glare' })
  }

  /** A lick of fire: a hot puff that cools to a dark ember red. */
  private flame(at: V, velocity: readonly [number, number, number], life: number, size: [number, number], color: number) {
    this.smoke.spawn({ position: at, velocity, life, size, color, colorEnd: 0x4a1405, glow: 3, alpha: 0.9, gravity: -2.5, drag: 3, spin: [-2, 2], shape: 'puff' })
  }

  /** A puff that rises, swells and thins out. */
  private drift(at: V, velocity: readonly [number, number, number], life: number, size: [number, number], color: number, alpha: number, more?: Partial<ParticleSpec>) {
    this.smoke.spawn({ position: at, velocity, life, size, color, alpha, gravity: -0.6, drag: 1.2, spin: [-0.6, 0.6], shape: 'puff', ...more })
  }

  /** One hot speck on an arc, stretched along its flight; `i` picks its colour. */
  private spark(at: V, velocity: readonly [number, number, number], life: number, size: number, i: number) {
    this.glow.spawn({ position: at, velocity, life, size: [size, size * 0.3], color: SPARK[i % SPARK.length]!, colorEnd: 0xff3a05, glow: 6, stretch: 0.03, alpha: 1, gravity: 9.8, drag: 0.5 })
  }

  /** A flash and a spray of `count` bright specks thrown from `at`, drawn for `reach` of the usual reach. */
  private sparks(at: V, count: number, reach: number) {
    this.flash(at, 0.7, 0.08, 0xffffff, 8)
    for (let i = 0; i < count; i++) {
      const direction = norm({ x: Math.random() - 0.5, y: Math.random() * 0.9 + 0.1, z: Math.random() - 0.5 })
      const speed = rand(4, 8) * reach
      this.spark(at, [direction.x * speed, direction.y * speed, direction.z * speed], rand(0.18, 0.32), rand(0.04, 0.08), i)
    }
  }

  /** A shotgun going off: a fat flash, flame thrown forward in a cone and a wisp of smoke. */
  blast(muzzle: V, direction: V) {
    const d = norm(direction)
    this.flash(add(muzzle, d, 0.5), 1.6, 0.1, 0xfff0b8, 8)
    for (let i = 0; i < 6; i++) {
      const jet = norm(add(d, spread(), 0.5))
      const speed = rand(7, 16)
      this.glow.spawn({ position: muzzle, velocity: [jet.x * speed, jet.y * speed, jet.z * speed], life: rand(0.1, 0.18), size: [rand(0.25, 0.45), 0.1], color: i % 2 ? 0xffb347 : 0xff7a1a, colorEnd: 0xff3a05, glow: 4, stretch: 0.02, alpha: 1, drag: 6 })
    }
    this.drift(add(muzzle, d, 0.8), [d.x * 2.4, d.y * 2.4 + 0.8, d.z * 2.4], rand(0.7, 1.1), [0.3, rand(1, 1.5)], 0x8a8a8a, 0.35)
  }

  /** One shotgun pellet landing: a few sparks and a small puff of grit. */
  pelletStrike(at: V) {
    this.sparks(at, 3, 0.7)
    this.drift(at, [rand(-0.4, 0.4), rand(0.4, 1), rand(-0.4, 0.4)], rand(0.4, 0.7), [0.15, rand(0.5, 0.8)], pick(GRIT), 0.4)
  }

  /** A round striking something: a spark and a puff. */
  strike(at: V) {
    this.flash(at, 1.2, 0.2, 0xffa21f, 6)
    this.drift(at, [rand(-0.5, 0.5), rand(0.4, 1.2), rand(-0.5, 0.5)], rand(0.5, 0.8), [0.2, rand(0.7, 1.1)], pick(GRIT), 0.4)
  }

  /**
   * Debris flung from where a round hit: red flecks off a body, sparks off a vehicle. A kill throws
   * more, harder.
   */
  impact(at: V, kind: 'body' | 'vehicle' | 'kill', away: V | null = null) {
    const count = kind === 'kill' ? 12 : 6
    for (let i = 0; i < count; i++) {
      let direction = norm({ x: Math.random() - 0.5, y: Math.random() * 0.8, z: Math.random() - 0.5 })
      if (away) direction = norm(add(direction, away, 0.8))
      const speed = (kind === 'kill' ? 7 : 4.5) * (0.5 + Math.random())
      const velocity = [direction.x * speed, direction.y * speed, direction.z * speed] as const
      if (kind === 'vehicle') {
        this.spark(at, velocity, rand(0.28, 0.48), rand(0.06, 0.1), i)
      } else {
        const size = 0.09 * rand(0.7, 1.5) * (kind === 'kill' ? 1.4 : 1)
        this.blood.spawn({ position: at, velocity, life: rand(0.3, 0.5), size: [size, size * 0.8], color: 0x7a0606, alpha: 1, gravity: 9.8, drag: 0.5, stretch: 0.015, shape: 'drop' })
      }
    }
    this.flash(at, 0.6, 0.09, kind === 'vehicle' ? 0xffffff : 0xff6a50, 5)
  }

  /** A round striking a soldier at `at`, for the one who fired it: a pinpoint flash on the spot. */
  struck(at: V) {
    this.flash(at, 0.4, 0.08, 0xffffff, 8)
  }

  /**
   * A round going through armour at `at`, on a plate facing `outward`: a white-hot flash, molten
   * fragments blown back out of the hole and a wisp of smoke from it. `big` for a shell, which throws
   * more and further, against a bullet through a slit or a thin plate.
   */
  pierce(at: V, outward: V, big: boolean) {
    const out = norm(outward)
    const scale = big ? 1 : 0.45
    this.flash(at, 1.8 * scale, 0.1, 0xffffff, 10)
    this.flash(at, 1.4 * scale, 0.35, 0xff8a2a, 6)
    const count = big ? 16 : 6
    for (let i = 0; i < count; i++) {
      const direction = norm(add(out, spread(), 1.4))
      const speed = rand(6, 15) * scale
      this.spark(at, [direction.x * speed, direction.y * speed, direction.z * speed], rand(0.3, 0.65), rand(0.05, 0.1) * (big ? 1.3 : 1), i)
    }
    this.drift(add(at, out, 0.15), [out.x + WIND[0] * 0.4, out.y + rand(0.6, 1.2), out.z + WIND[1] * 0.4], rand(1.3, 2), [0.3 * scale, rand(1.2, 1.8) * scale], 0x2c2a28, 0.5)
  }

  /** Armour stopping a round at `at`, on a plate facing `outward`: sparks glancing off and a puff of grit. */
  deflect(at: V, outward: V) {
    const out = norm(outward)
    this.flash(at, 0.5, 0.07, 0xffffff, 8)
    for (let i = 0; i < 5; i++) {
      const direction = norm(add(out, spread(), 2))
      const speed = rand(5, 11)
      this.spark(at, [direction.x * speed, direction.y * speed, direction.z * speed], rand(0.15, 0.27), rand(0.03, 0.06), i)
    }
    this.drift(at, [out.x * 0.6, out.y * 0.6 + 0.5, out.z * 0.6], rand(0.4, 0.6), [0.12, 0.7], 0x9a948a, 0.4)
  }

  /** The white-hot flash of a blast at `at`; the fire, smoke and debris around it come from `explosion`. */
  burst(at: V, radius: number) {
    this.flash(at, radius * 1.8, 0.25, 0xff9a3c, 8)
    this.flash(at, radius * 1.2, 0.12, 0xfff0c0, 12)
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

  /** A cloud of dust of about `size` metres across at `at`, swelling and rising as it thins. */
  dust(at: V, size: number) {
    dustCloud(this.smoke, at, size)
  }

  /** Just the muzzle flash: the local player's own shot before the server confirms it, or a machine gun's round. */
  muzzle(at: V) {
    this.flash(at, 0.5, 0.06, 0xfff2b0, 6)
  }

  update(dt: number) {
    this.root.update(dt)
  }

  dispose() {
    this.root.removeFromParent()
  }
}
