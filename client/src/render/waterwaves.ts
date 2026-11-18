import { MOST_STAMPS, WaveField, type WaveStamp, type XYZ } from '@ragnarok/engine'
import { setWaterWakes } from './water'

/** Texels a side of the water's wave field, and metres a texel: it covers 96 m square about the camera. */
export const WAVE_GRID = 256
export const WAVE_TEXEL = 0.375
/**
 * How fast its ripples run out, m/s. A boat faster than this leaves a V of waves, the narrower the
 * faster: about 22° either side of a submarine at 8 m/s, 8° of a Stridsbåt 90 flat out. That is how
 * real wakes narrow once a hull goes faster than about 0.6 √(g L) (Rabaud and Moisy), at
 * 0.25 √(g L) ≈ 3 m/s for hulls of 15 to 25 m.
 */
export const WAVE_SPEED = 3
/** Further than this from the camera, metres, nothing stirs the field (it reaches 48 m out). */
const REACH = 60

/**
 * How hard something moving at `speed` m/s pushes the water aside, 0 to 1: as the square of its speed
 * against the ripples', as the waves a hull makes grow with its Froude number squared, so a swimmer or a
 * boat creeping along barely stirs it and one at the ripples' speed or faster pushes it fully.
 */
export function pushShare(speed: number) {
  return Math.min(1, (speed / WAVE_SPEED) ** 2)
}

/** What can stir the water: hulls and bodies moving through it, and things landing in it. */
export interface Stir {
  /**
   * A hull or body moving through the water this frame (`WaveField.stamp`). `key` names it from frame to
   * frame, so how much deeper or shallower it sits than last time moves the water too.
   */
  hull(key: string, stamp: WaveStamp): void
  /** Something landing at `(x, z)`: a hole `radius` metres across and `depth` deep that runs out as rings, and `foam` (0 to 1). */
  splash(x: number, z: number, radius: number, depth: number, foam: number): void
}

type Queued = WaveStamp & { weight: number; key: string | null }

/**
 * The water's waves from what moves through it (`WaveField`, a wave equation stepped on the GPU about
 * the camera): boats and waders move through it each frame and things land in it, and their bow waves,
 * wakes and rings come out of it. When more come than it takes, the nearest and biggest go first.
 *
 * Each hull is followed by its key: it settles into the water as it starts to move and lifts out of it
 * as it stops or leaves, so it never leaves a hole or a heap of its own size behind.
 */
export class WaterWaves implements Stir {
  /**
   * `current` is the river's current at a point, m/s (x, z): a hull is judged by its speed through the
   * water, so one carried along by a river stirs it no more than one lying still in a lake.
   */
  constructor(private readonly current: (x: number, z: number) => readonly [number, number] = () => [0, 0]) {}

  readonly field = new WaveField({ grid: WAVE_GRID, texel: WAVE_TEXEL, speed: WAVE_SPEED })
  private queue: Queued[] = []
  /** Each hull as it was last handed to a frame that stepped: how deep the water there is pushed now. */
  private readonly sat = new Map<string, WaveStamp>()
  private eye = { x: 0, z: 0 }

  /** The effect to add to the scene. */
  get root() {
    return this.field
  }

  /** How many stamps are waiting for the next `update`. */
  get waiting() {
    return this.queue.length
  }

  /** How deep hull `key` pushes the water now, metres (0 when it pushes none). */
  pushing(key: string) {
    return this.sat.get(key)?.depth ?? 0
  }

  hull(key: string, stamp: WaveStamp) {
    // It pushes and churns the water by how fast it goes through it, not over the ground; its velocity
    // over the ground still says where it was a step ago, so it is stamped as it is.
    const [cx, cz] = this.current(stamp.x, stamp.z)
    const through = Math.hypot(stamp.velocity[0] - cx, stamp.velocity[1] - cz)
    const depth = stamp.depth * pushShare(through)
    const churn = stamp.churn * Math.min(1, through)
    this.push({ ...stamp, depth, churn }, key)
  }

  splash(x: number, z: number, radius: number, depth: number, foam: number) {
    this.push({ x, z, ahead: [0, 1], halfWidth: radius, halfLength: radius, depth, velocity: [0, 0], churn: foam }, null)
  }

  private push(stamp: WaveStamp, key: string | null) {
    const far = Math.hypot(stamp.x - this.eye.x, stamp.z - this.eye.z)
    if (far > REACH) return
    const size = Math.abs(stamp.depth) * Math.max(stamp.halfWidth, stamp.halfLength) + 0.05
    this.queue.push({ ...stamp, key, weight: size / (1 + far / 20) })
  }

  /** Hands this frame's stamps to the field and lets `dt` seconds pass with the camera at `eye`; the water then reads the new map. */
  update(dt: number, eye: Readonly<XYZ>) {
    this.eye.x = eye.x
    this.eye.z = eye.z
    // A hull stamped twice this frame counts once, where it was last.
    const hulls = new Map<string, Queued>()
    const splashes: Queued[] = []
    for (const q of this.queue) {
      if (q.key === null) splashes.push(q)
      else hulls.set(q.key, q)
    }
    // A hull gone from the water (stopped, out, or no longer drawn) lifts out where it last sat.
    for (const [key, last] of this.sat) {
      if (hulls.has(key)) continue
      if (Math.hypot(last.x - eye.x, last.z - eye.z) > REACH) this.sat.delete(key)
      else hulls.set(key, { ...last, velocity: [0, 0], depth: 0, key, weight: Infinity })
    }
    for (const [key, q] of hulls) q.was = this.sat.get(key)?.depth ?? 0
    const all = [...hulls.values(), ...splashes]
    if (all.length > MOST_STAMPS) all.sort((a, b) => b.weight - a.weight)
    const handed = all.slice(0, MOST_STAMPS)
    for (const s of handed) {
      if (s.key === null) this.field.splash(s.x, s.z, s.halfWidth, s.depth, s.churn)
      else this.field.stamp(s)
    }
    this.queue.length = 0
    this.field.update(dt, eye.x, eye.z)
    if (this.field.pending === 0) return
    // Only a frame that steps moves the water, so only then do the hulls sit where they were handed.
    for (const s of handed) {
      if (s.key === null) continue
      if (s.depth === 0) this.sat.delete(s.key)
      else this.sat.set(s.key, { x: s.x, z: s.z, ahead: s.ahead, halfWidth: s.halfWidth, halfLength: s.halfLength, depth: s.depth, velocity: s.velocity, churn: s.churn })
    }
    setWaterWakes(this.field.texture, this.field.area)
  }
}
