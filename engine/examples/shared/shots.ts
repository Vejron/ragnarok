import { FlashEffects } from '../../src'

type V = { x: number; y: number; z: number }
const sub = (a: V, b: V) => ({ x: a.x - b.x, y: a.y - b.y, z: a.z - b.z })
const len = (a: V) => Math.hypot(a.x, a.y, a.z)
const scale = (a: V, s: number) => ({ x: a.x * s, y: a.y * s, z: a.z * s })
const add = (a: V, b: V, s = 1) => ({ x: a.x + b.x * s, y: a.y + b.y * s, z: a.z + b.z * s })
const norm = (a: V) => scale(a, 1 / (len(a) || 1))

/**
 * The game's shot effects (client/src/render/effects.ts) on FlashEffects: the same methods, colours and
 * motion, each piece one GPU record instead of a mesh with its own material. The three version scales a
 * unit sphere and a 0.06-radius cylinder; here sizes are the radius they end up as.
 */
export class Shots {
  constructor(readonly flashes = new FlashEffects()) {}

  muzzleFlash(muzzle: V, direction: V) {
    this.flashes.spawn({ at: muzzle, life: 0.12, size: 0.5, grow: 0.8, color: 0xffe9a0 })
    this.flashes.spawn({ at: add(muzzle, norm(direction), 1), life: 0.25, size: 0.3, grow: 0.9, color: 0xffc060 })
  }

  tracer(from: V, to: V, hit: boolean) {
    const length = len(sub(to, from))
    if (length < 0.01) return
    const direction = norm(sub(to, from))
    this.flashes.spawn({ at: add(from, direction, 0.6), life: 0.06, size: 0.1, grow: 0.1, color: 0xfff2b0, bright: true })
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
    this.flashes.spawn({ at, life: 0.08, size: 0.08, grow: 0.14, color: 0xffffff, bright: true })
    for (let i = 0; i < count; i++) {
      const direction = { x: Math.random() - 0.5, y: Math.random() * 0.9 + 0.1, z: Math.random() - 0.5 }
      const speed = (3 + Math.random() * 4) * reach
      const size = 0.015 + Math.random() * 0.02
      this.flashes.spawn({ at, direction, along: speed * 0.3, drop: 1.8, size, grow: -0.6 * size, life: 0.18 + Math.random() * 0.14, color: i % 2 ? 0xffd070 : 0xff8a1f, bright: true })
    }
  }

  blast(muzzle: V, direction: V) {
    const d = norm(direction)
    this.flashes.spawn({ at: add(muzzle, d, 0.5), life: 0.1, size: 0.35, grow: 0.35, color: 0xfff0b8, bright: true })
    for (let i = 0; i < 7; i++) {
      const jet = add(d, { x: Math.random() - 0.5, y: Math.random() - 0.5, z: Math.random() - 0.5 }, 0.5)
      const size = 0.08 + Math.random() * 0.1
      this.flashes.spawn({ at: muzzle, direction: jet, sqrtAlong: 1.5 + Math.random() * 2, size, grow: -0.5 * size, life: 0.1 + Math.random() * 0.08, color: i % 2 ? 0xffb347 : 0xff7a1a, bright: true })
    }
    this.flashes.spawn({ at: add(muzzle, d, 0.8), direction: d, along: 1.2, lift: 0.4, size: 0.15, grow: 0.35, life: 0.5, color: 0x8a8a8a })
  }

  pelletStrike(at: V) {
    this.sparks(at, 3, 0.7)
    this.flashes.spawn({ at, lift: 0.3, size: 0.1, grow: 0.3, life: 0.4, color: 0xb8ad98 })
  }

  round(from: V, to: V, speed = 700) {
    const length = len(sub(to, from))
    if (length < 0.01) return
    const direction = norm(sub(to, from))
    this.flashes.spawn({ at: from, life: 0.06, size: 0.22, grow: 0.15, color: 0xffe9a0, bright: true })
    const streak = Math.min(7, length)
    const at = add(from, direction, streak / 2)
    for (const [width, color] of [[1.2, 0xfff4cc], [3, 0xff7a1a]] as const) {
      this.flashes.spawn({ at, direction, along: length - streak, length: streak, size: 0.06 * width, life: length / speed, color, bright: true })
    }
  }

  burst(at: V, radius: number) {
    this.flashes.spawn({ at, life: 0.25, size: radius * 0.12, grow: radius * 0.3, color: 0xff9a3c, bright: true })
    this.flashes.spawn({ at, life: 0.12, size: radius * 0.1, grow: radius * 0.2, color: 0xfff0c0, bright: true })
  }

  dust(at: V, size: number) {
    this.flashes.spawn({ at, lift: size * 0.5, size: size * 0.35, grow: size * 0.65, life: 2.2, color: 0xcbc2b0 })
  }
}
