import { FLOW_GRID, MOST_STAMPS, WaveField, type WaveStamp, type XYZ } from '@ragnarok/engine'
import { FLOW_STEP } from '@/sim/waves'
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
 * The river's current is handed to the field as a map this wide, metres, about the camera (the field's
 * own 96 m and room to move in), laid on lines this far apart so it is made again only when the camera
 * has gone that far.
 */
const FLOW_SIDE = 192
const FLOW_SNAP = 24
/**
 * A boulder stirs the water when its top lies no deeper than the first of these, metres, in a current
 * of at least the second, m/s (`Waters.rocksNear`), within the third of the camera, metres; the nearest
 * few of them do, at most the fourth.
 */
const ROCK_DEPTH = 0.45
const ROCK_SPEED = 1
const ROCK_REACH = 40
const ROCKS_MOST = 24
/** A boulder pushes the water as a round body this wide from its middle, metres, sitting this deep in a fast current. */
const ROCK_RADIUS = 1.3
const ROCK_PUSH = 0.22
/** The boulders are looked for again when the camera has moved this far, metres, or this long has passed, seconds (tiles come and go). */
const ROCKS_MOVED = 6
const ROCKS_AGE = 2

/** A boulder under a river's surface, as `Waters.rocksNear` gives it. */
export interface RiverRock {
  x: number
  z: number
  depth: number
  flow: readonly [number, number]
}

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
 * wakes and rings come out of it. When more come than it takes, the nearest and biggest go first. In a
 * river the field's water runs with the current, and the boulders just under the surface trail wakes.
 *
 * Each hull is followed by its key: it settles into the water as it starts to move and lifts out of it
 * as it stops or leaves, so it never leaves a hole or a heap of its own size behind.
 */
export class WaterWaves implements Stir {
  /**
   * `current` is the river's current at a point, m/s (x, z): a hull is judged by its speed through the
   * water, so one carried along by a river stirs it no more than one lying still in a lake. The field's
   * water is carried along by it too, so what stands in a river trails a wake: `rocks` gives the
   * boulders under the surface near a point (`Waters.rocksNear`), which do.
   */
  constructor(
    private readonly current: (x: number, z: number) => readonly [number, number] = () => [0, 0],
    private readonly rocks: (x: number, z: number, reach: number, depth: number, speed: number) => readonly RiverRock[] = () => [],
  ) {}

  /** The current's map as the field has it: its corner (x, z), or null before the first. */
  private mapped: [number, number] | null = null
  private readonly flowBytes = new Uint8Array(FLOW_GRID * FLOW_GRID * 4)
  /** The boulders stirring the water now, nearest first, where they were looked for and how long ago. */
  private stirring: RiverRock[] = []
  private looked: { x: number; z: number; age: number } | null = null

  /** How many boulders stir the water now. */
  get rocky() {
    return this.stirring.length
  }

  /** Hands the field the current round `eye`, when the camera has moved on to another square of it. */
  private chart(eye: Readonly<XYZ>) {
    const x0 = Math.round(eye.x / FLOW_SNAP) * FLOW_SNAP - FLOW_SIDE / 2
    const z0 = Math.round(eye.z / FLOW_SNAP) * FLOW_SNAP - FLOW_SIDE / 2
    if (this.mapped && this.mapped[0] === x0 && this.mapped[1] === z0) return
    this.mapped = [x0, z0]
    const texel = FLOW_SIDE / FLOW_GRID
    const byte = (v: number) => Math.min(255, Math.max(1, 128 + Math.round(v / FLOW_STEP)))
    let running = false
    for (let j = 0; j < FLOW_GRID; j++) {
      for (let i = 0; i < FLOW_GRID; i++) {
        const [vx, vz] = this.current(x0 + (i + 0.5) * texel, z0 + (j + 0.5) * texel)
        const o = (j * FLOW_GRID + i) * 4
        this.flowBytes[o] = byte(vx)
        this.flowBytes[o + 1] = byte(vz)
        running ||= vx !== 0 || vz !== 0
      }
    }
    if (running) this.field.setFlow(this.flowBytes, x0, z0, FLOW_SIDE, FLOW_STEP)
    else this.field.setFlow(null)
  }

  /** Looks for the boulders round `eye` again when the camera has moved or time has passed. */
  private survey(dt: number, eye: Readonly<XYZ>) {
    if (this.looked) this.looked.age += dt
    if (this.looked && this.looked.age < ROCKS_AGE && Math.hypot(eye.x - this.looked.x, eye.z - this.looked.z) < ROCKS_MOVED) return
    // The water's tiles may have come in since the current was last charted.
    if (this.looked && this.looked.age >= ROCKS_AGE) this.mapped = null
    this.looked = { x: eye.x, z: eye.z, age: 0 }
    const far = (rock: RiverRock) => Math.hypot(rock.x - eye.x, rock.z - eye.z)
    this.stirring = this.rocks(eye.x, eye.z, ROCK_REACH, ROCK_DEPTH, ROCK_SPEED).filter((rock) => far(rock) <= ROCK_REACH).sort((a, b) => far(a) - far(b)).slice(0, ROCKS_MOST)
  }

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
    // The boulders the river runs over take what room is left: they stand still, and the field, which
    // carries its water along, has them push what runs onto them, harder the faster it runs.
    this.survey(dt, eye)
    this.chart(eye)
    for (const rock of this.stirring.slice(0, MOST_STAMPS - handed.length)) {
      const share = pushShare(Math.hypot(rock.flow[0], rock.flow[1]))
      this.field.stamp({ x: rock.x, z: rock.z, ahead: [0, 1], halfWidth: ROCK_RADIUS, halfLength: ROCK_RADIUS, depth: ROCK_PUSH * share, velocity: [0, 0], churn: 0.6 * share })
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
