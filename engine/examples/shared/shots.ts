import { FlashEffects, type ParticleSystem } from '../../src'

type V = { x: number; y: number; z: number }
const sub = (a: V, b: V) => ({ x: a.x - b.x, y: a.y - b.y, z: a.z - b.z })
const len = (a: V) => Math.hypot(a.x, a.y, a.z)
const scale = (a: V, s: number) => ({ x: a.x * s, y: a.y * s, z: a.z * s })
const add = (a: V, b: V, s = 1) => ({ x: a.x + b.x * s, y: a.y + b.y * s, z: a.z + b.z * s })
const norm = (a: V) => scale(a, 1 / (len(a) || 1))

/**
 * The game's shot effects (client/src/render/effects.ts): flashes, sparks and smoke as particles, and
 * tracers as `FlashEffects` streaks. The `glow` particles are additive.
 */
export class Shots {
  constructor(readonly particles: ParticleSystem, readonly flashes = new FlashEffects()) {}

  private flash(at: V, size: number, life: number, color: number) {
    this.particles.spawn({ position: at, velocity: [0, 0, 0], life, size: [size * 0.7, size], color, glow: 8, alpha: 1, blend: 'additive', shape: 'glare' })
  }

  private puff(at: V, life: number, size: number, color: number) {
    this.particles.spawn({ position: at, velocity: [0, 0.8, 0], life, size: [size * 0.3, size], color, alpha: 0.4, gravity: -0.6, drag: 1.2, spin: [-0.6, 0.6], shape: 'puff' })
  }

  muzzleFlash(muzzle: V) {
    this.flash(muzzle, 2.4, 0.12, 0xffe9a0)
  }

  tracer(from: V, to: V, hit: boolean) {
    const length = len(sub(to, from))
    if (length < 0.01) return
    const direction = norm(sub(to, from))
    this.flash(add(from, direction, 0.6), 0.45, 0.06, 0xfff2b0)
    const start = Math.min(1.5, length)
    const run = length - start
    if (run > 1) {
      const streak = Math.min(5, run)
      const life = Math.min(0.4, Math.max(0.05, run / 350))
      const at = add(from, direction, start + streak / 2)
      for (const [width, color] of [[0.2, 0xfff6d8], [0.55, 0xff9a2e]] as const) {
        this.flashes.spawn({ at, direction, along: run - streak, length: streak, size: 0.06 * width, life, color, bright: true, fade: 'late' })
      }
    }
    if (hit) this.sparks(to, 5, 0.9)
  }

  sparks(at: V, count: number, reach: number) {
    this.flash(at, 0.7, 0.08, 0xffffff)
    for (let i = 0; i < count; i++) {
      const direction = norm({ x: Math.random() - 0.5, y: Math.random() * 0.9 + 0.1, z: Math.random() - 0.5 })
      const speed = (4 + Math.random() * 4) * reach
      this.particles.spawn({ position: at, velocity: scale(direction, speed), life: 0.18 + Math.random() * 0.14, size: [0.06, 0.02], color: i % 2 ? 0xffd070 : 0xff8a1f, glow: 6, stretch: 0.03, alpha: 1, gravity: 9.8, drag: 0.5, blend: 'additive' })
    }
  }

  blast(muzzle: V, direction: V) {
    const d = norm(direction)
    this.flash(add(muzzle, d, 0.5), 1.6, 0.1, 0xfff0b8)
    for (let i = 0; i < 6; i++) {
      const jet = norm(add(d, { x: Math.random() - 0.5, y: Math.random() - 0.5, z: Math.random() - 0.5 }, 0.5))
      this.particles.spawn({ position: muzzle, velocity: scale(jet, 7 + Math.random() * 9), life: 0.1 + Math.random() * 0.08, size: [0.35, 0.1], color: i % 2 ? 0xffb347 : 0xff7a1a, glow: 4, stretch: 0.02, alpha: 1, drag: 6, blend: 'additive' })
    }
    this.puff(add(muzzle, d, 0.8), 0.9, 1.2, 0x8a8a8a)
  }

  pelletStrike(at: V) {
    this.sparks(at, 3, 0.7)
    this.puff(at, 0.5, 0.7, 0xb8ad98)
  }

  round(from: V, to: V, speed = 700) {
    const length = len(sub(to, from))
    if (length < 0.01) return
    const direction = norm(sub(to, from))
    this.flash(from, 1, 0.06, 0xffe9a0)
    const streak = Math.min(7, length)
    const at = add(from, direction, streak / 2)
    for (const [width, color] of [[1.2, 0xfff4cc], [3, 0xff7a1a]] as const) {
      this.flashes.spawn({ at, direction, along: length - streak, length: streak, size: 0.06 * width, life: length / speed, color, bright: true })
    }
  }

  burst(at: V, radius: number) {
    this.flash(at, radius * 1.8, 0.25, 0xff9a3c)
    this.flash(at, radius * 1.2, 0.12, 0xfff0c0)
  }

  dust(at: V, size: number) {
    this.puff(at, 2.2, size, 0xcbc2b0)
  }
}
