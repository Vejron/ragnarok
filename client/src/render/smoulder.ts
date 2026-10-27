import type { Particles } from './particles'

/** Seconds a fresh crater smokes for, thinning to nothing. */
export const SMOULDER_SECONDS = 30
/** Craters smoking at once; a new one past this puts out the oldest. Bounds the particles in a barrage. */
export const SMOULDER_MAX = 16
/** Craters smaller than this (metres of radius: an autocannon round's) do not smoke, so a burst of them does not put out a shell's. */
export const SMOULDER_MIN_RADIUS = 3
/** Seconds between puffs of a crater just dug, and of one nearly cold. */
const WISP_EVERY = [0.08, 0.45] as const
/** The drift the smoke leans with, m/s: the vehicles' smoke's wind (`vehiclesmoke.ts`). */
const WIND = [1.1, 0.5] as const
/** Soot dark to grey, picked per puff so the column is not one flat colour. */
const SMOKE = [0x2e2a26, 0x3d3833, 0x55504a] as const

interface Smoulder {
  x: number
  y: number
  z: number
  radius: number
  born: number
  next: number
}

const rand = (min: number, max: number) => min + Math.random() * (max - min)

/**
 * Smoke rising from fresh craters (SCORCH.md): a column of puffs leaning downwind, thick and quick at
 * first, thin and seldom as the crater cools, gone after `SMOULDER_SECONDS`. The embers glowing with it
 * are drawn by the ground's shader from the crater's birth time.
 */
export class Smoulders {
  private readonly live: Smoulder[] = []

  /** A crater of `radius` metres dug at `(x, y, z)` (on the ground in its middle) at `now` seconds. */
  add(x: number, y: number, z: number, radius: number, now: number) {
    if (radius < SMOULDER_MIN_RADIUS) return
    if (this.live.length >= SMOULDER_MAX) this.live.shift()
    this.live.push({ x, y, z, radius, born: now, next: now })
  }

  /** Smoking craters, for tests. */
  get count() {
    return this.live.length
  }

  clear() {
    this.live.length = 0
  }

  /** Lets out the wisps due by `now` (seconds, as given to `add`) into `smoke`, and forgets cold craters. */
  update(now: number, smoke: Pick<Particles, 'spawn'>) {
    let kept = 0
    for (const s of this.live) {
      const heat = 1 - (now - s.born) / SMOULDER_SECONDS
      if (heat <= 0) continue
      this.live[kept++] = s
      if (now < s.next) continue
      s.next = now + WISP_EVERY[0] + (WISP_EVERY[1] - WISP_EVERY[0]) * (1 - heat)
      const r = s.radius * 0.25
      smoke.spawn({
        position: [s.x + rand(-r, r), s.y + 0.3, s.z + rand(-r, r)],
        velocity: [WIND[0] * rand(0.3, 1) + rand(-0.4, 0.4), rand(1.2, 2.0) + heat * 1.5, WIND[1] * rand(0.3, 1) + rand(-0.4, 0.4)],
        life: rand(4, 6) * (0.6 + 0.4 * heat),
        size: [0.4 + s.radius * 0.06, rand(1.8, 3.0) * (0.5 + 0.5 * heat) + s.radius * 0.15],
        color: SMOKE[Math.floor(Math.random() * SMOKE.length)]!,
        colorEnd: 0x9a958d,
        alpha: 0.25 + 0.35 * heat,
        gravity: -0.8,
        drag: 0.6,
        spin: [-0.4, 0.4],
        shape: 'puff',
      })
    }
    this.live.length = kept
  }
}
