import type { Color, XYZ } from '@ragnarok/engine'
import { Particles, type EmitSpec } from './particles'
import type { Stir } from './waterwaves'

/** How many drops and streaks the spray can hold: a pair of boats crash stopping at once. */
const DROP_POOL = 8192
/** How many puffs of mist it can hold; they are big and soft, so drawn at half the screen's size. */
const MIST_POOL = 1536

/** A whole number of particles for `rate` a second over `dt`, carrying the fraction by chance. */
const count = (rate: number, dt: number) => Math.floor(rate * dt + Math.random())
/** How many times a second each stream of drops marks where it comes down on the water. */
const LANDINGS = 15
/** Metres per second² the drops fall at, and the share of their speed they lose a second (`droplets`). */
const GRAVITY = 9.8
const DROP_DRAG = 0.2

/** Where a waterjet boat is and what it is doing, for one frame of its spray. */
export interface JetBoat {
  /** The middle of the hull, metres. */
  at: XYZ
  /** Unit directions to its right and ahead, level with the water. */
  right: XYZ
  ahead: XYZ
  /** How fast it is going, m/s (world). */
  velocity: XYZ
  /** Speed along its nose, m/s, signed. */
  speed: number
  /** The water's height under it, metres. */
  level: number
  /** Half its length, metres. */
  halfLength: number
  /** The speed its rooster tail is at its biggest, m/s. */
  fullSpeed: number
  /** How big a rooster tail its drive throws up, 0 to 1: waterjets 1, an outboard's propeller none. */
  tail: number
}

/** Where an outboard boat is and what it is doing, for one frame of its prop wash. */
export interface Outboard {
  /** The middle of the hull, metres. */
  at: XYZ
  /** Unit directions to its right and ahead, level with the water. */
  right: XYZ
  ahead: XYZ
  /** How fast it is going, m/s (world). */
  velocity: XYZ
  /** Speed ahead through the water, m/s, signed. */
  speed: number
  /** The water's height under it, metres. */
  level: number
  /** Half its beam and half its length, metres. */
  halfBeam: number
  halfLength: number
  /** The speed its wash is at its fullest, m/s. */
  fullSpeed: number
  /** How hard its hull is slapping down onto the water, 0 to 1: a burst of spray as it lands off a wave. */
  slap: number
}

/** Where a hull skimming over the water is and what it is doing, for one frame of its spray. */
export interface Skimmer {
  /** The middle of the hull, metres. */
  at: XYZ
  /** Unit directions to its right and ahead, level with the water. */
  right: XYZ
  ahead: XYZ
  /** How fast it is going, m/s (world). */
  velocity: XYZ
  /** Speed along its nose, m/s, signed. */
  speed: number
  /** The water's height under it, metres. */
  level: number
  /** Half its width and half its length, metres. */
  halfBeam: number
  halfLength: number
  /** How much of a rooster tail its drive throws up behind it, 0 to 1: a snowmobile's track 1, a car's wheels less. */
  tail: number
}

/** The speed a skimming hull's spray is at its fullest, m/s. */
const SKIM_FULL_SPEED = 20

/** Bright, faintly blue water, and the grey-green of a darker sheet of it. */
const WATER = 0xf2f9ff
const WATER_DARK = 0xc9dde6
const MIST = 0xf6faff

/**
 * The water a waterjet boat throws, in the layers real spray is made of: sheets of fast water seen as
 * streaks, which break up into drops that arc and fall back, a mist that hangs and drifts downwind, and
 * glints of sun in it: the rooster tail off a waterjet boat's jets under way. What a hull does to the
 * surface, its bow wave, wake and white wash, is the wave field's. All of it is water (`scatter`): it
 * blazes when seen against the sun.
 */
export class JetSpray {
  private readonly drops = new Particles(DROP_POOL)
  private readonly mist = new Particles(MIST_POOL, false, true)

  /** Where streams of drops will come down on the water, and in how many seconds. */
  private readonly landings: { in: number; x: number; z: number; radius: number }[] = []
  /** The water's height and the time step of the spray being thrown, for working out where it lands. */
  private level = 0
  private dt = 0

  constructor(
    private readonly glow: Particles,
    private readonly stir?: Stir,
  ) {}

  get roots() {
    return [this.drops.root, this.mist.root] as const
  }

  /** How many particles of water are in the air. */
  get alive() {
    return this.drops.alive + this.mist.alive
  }

  /** How many puffs of mist are in the air. */
  get misty() {
    return this.mist.alive
  }

  /** The light on the ground (`Particles.light`), for both pools: it dims the water at dusk. */
  setLight(light: Color) {
    this.drops.light.copy(light)
    this.mist.light.copy(light)
  }

  /** Which way the wind blows, a unit `[x, z]` (`Particles.blowing`): the mist drifts off along it. */
  blowing(direction: readonly [number, number]) {
    this.mist.blowing(direction)
  }

  /** Throws `dt` seconds of the boat's spray. */
  throw(boat: JetBoat, dt: number) {
    const { at, right, ahead, velocity: v, level, halfLength: hz } = boat
    this.level = level
    this.dt = dt
    const fast = Math.min(1, Math.max(0, boat.speed) / boat.fullSpeed)
    const point = (x: number, z: number, up: number) =>
      [at.x + right.x * x + ahead.x * -z, level + up, at.z + right.z * x + ahead.z * -z] as const
    // A velocity: `keep` of the boat's, `side` metres a second to its right, `on` ahead and `up` up.
    const flung = (keep: number, side: number, on: number, up: number) =>
      [v.x * keep + right.x * side + ahead.x * on, up, v.z * keep + right.z * side + ahead.z * on] as const
    const light = this.drops.light
    const sun = Math.max(0, Math.min(1, ((light.r + light.g + light.b) / 3 - 0.3) / 0.7))

    // The rooster tail: a column of water thrown up off the jets, which keeps much of the boat's speed,
    // breaks into drops as it climbs and leaves a cloud behind and above it.
    const tail = boat.speed > 4 ? fast * boat.tail : 0
    if (tail > 0) {
      const from = point(0, hz + 0.4, 0.3)
      const rise = 4 + 5 * tail
      this.streaks(count(900 * tail, dt), from, 0.7, flung(0.6, 0, -2, rise), [0.5, rise * 0.15, 0.5], [0.9, 1.5])
      this.droplets(count(260 * tail, dt), from, 0.9, flung(0.55, 0, -2.5, rise * 0.85), 2.2, [1, 1.8])
      this.puffs(count(36 * tail, dt), point(0, hz + 2, 1.2), 1.2, flung(0.45, 0, -1, 1.5 + tail), [1.5, 4.5], [1.8, 3.2], 0.2)
      this.glints(count(80 * tail * sun, dt), from, flung(0.6, 0, -2, rise), sun)
    }
  }

  /**
   * Throws `dt` seconds of an outboard's prop wash: not a column of water but a low fan thrown back
   * and out from behind the transom, a few big drops that arc and drop back, a scatter of fine ones
   * and a thin mist, growing with its speed and bursting up as the hull slaps down off a wave.
   */
  outboard(boat: Outboard, dt: number) {
    const { at, right, ahead, velocity: v, level, halfBeam: hx, halfLength: hz } = boat
    this.level = level
    this.dt = dt
    const fast = Math.min(1, Math.max(0, boat.speed) / boat.fullSpeed)
    const wash = boat.speed > 2 ? fast * (1 + 1.5 * boat.slap) : 0
    if (wash <= 0) return
    const point = (x: number, z: number, up: number) =>
      [at.x + right.x * x + ahead.x * -z, level + up, at.z + right.z * x + ahead.z * -z] as const
    const flung = (keep: number, side: number, on: number, up: number) =>
      [v.x * keep + right.x * side + ahead.x * on, up, v.z * keep + right.z * side + ahead.z * on] as const
    const light = this.drops.light
    const sun = Math.max(0, Math.min(1, ((light.r + light.g + light.b) / 3 - 0.3) / 0.7))
    const from = point(0, hz + 0.25, 0.05)
    const rise = 1.2 + 1.8 * fast + 2 * boat.slap
    // The fan: drops thrown back and out to either side of the prop, keeping some of the boat's speed.
    for (const side of [-1, 1]) {
      const out = flung(0.55, side * (0.6 + 1.2 * fast), -1.5, rise)
      this.droplets(count(70 * wash, dt), from, hx * 0.3, out, 1.4, [0.5, 1])
      this.streaks(count(60 * wash, dt), from, hx * 0.25, out, [0.5, 0.6, 0.5], [0.3, 0.6])
    }
    // The fine stuff the fan breaks into, and a thin mist that hangs behind it.
    this.droplets(count(50 * wash, dt), point(0, hz + 0.6, 0.2), hx * 0.5, flung(0.5, 0, -1, rise * 0.6), 2, [0.4, 0.8])
    this.puffs(count(8 * wash, dt), point(0, hz + 1.2, 0.4), 0.6, flung(0.4, 0, -0.5, 0.6), [0.6, 1.6], [1, 1.8], 0.1)
    this.glints(count(25 * wash * sun, dt), from, flung(0.55, 0, -1.5, rise), sun)
  }

  /**
   * Throws `dt` seconds of the spray off a car or snowmobile skimming over the water: fans of streaks
   * and drops out to either side from its back corners, a rooster tail off its drive, a little thrown
   * off its nose, and a mist behind, all growing with its speed.
   */
  skim(hull: Skimmer, dt: number) {
    const { at, right, ahead, velocity: v, level, halfBeam: hx, halfLength: hz } = hull
    this.level = level
    this.dt = dt
    const fast = Math.min(1, Math.max(0, hull.speed) / SKIM_FULL_SPEED)
    if (fast <= 0) return
    const point = (x: number, z: number, up: number) =>
      [at.x + right.x * x + ahead.x * -z, level + up, at.z + right.z * x + ahead.z * -z] as const
    const flung = (keep: number, side: number, on: number, up: number) =>
      [v.x * keep + right.x * side + ahead.x * on, up, v.z * keep + right.z * side + ahead.z * on] as const
    const light = this.drops.light
    const sun = Math.max(0, Math.min(1, ((light.r + light.g + light.b) / 3 - 0.3) / 0.7))
    // Fans out to either side from the back corners, where the wheels or the skis cut the surface.
    for (const side of [-1, 1]) {
      const from = point(side * hx * 0.9, hz * 0.6, 0.1)
      const out = flung(0.4, side * (1.5 + 3 * fast), 0, 1 + 2 * fast)
      this.streaks(count(260 * fast, dt), from, 0.3, out, [0.5, 0.5, 0.5], [0.4, 0.8])
      this.droplets(count(60 * fast, dt), from, 0.4, out, 1, [0.5, 1])
      this.puffs(count(4 * fast, dt), point(side * (hx + 0.6), hz, 0.4), 0.6, flung(0.3, side * 1.2, 0, 0.5), [0.6, 1.8], [1, 1.8], 0.12)
    }
    // A little off the nose as it slaps the water.
    const nose = point(0, -hz, 0.1)
    this.droplets(count(40 * fast, dt), nose, hx * 0.8, flung(0.8, 0, 0, 1 + fast), 1.5, [0.3, 0.6])
    // The rooster tail off its drive, kept from its speed, breaking into drops and a mist behind.
    const tail = hull.tail * fast
    if (tail > 0) {
      const from = point(0, hz + 0.2, 0.15)
      const rise = 2.5 + 4 * tail
      this.streaks(count(500 * tail, dt), from, 0.3, flung(0.45, 0, -1, rise), [0.4, rise * 0.15, 0.4], [0.7, 1.2])
      this.droplets(count(140 * tail, dt), from, 0.4, flung(0.4, 0, -1.5, rise * 0.8), 1.5, [0.8, 1.4])
      this.puffs(count(16 * tail, dt), point(0, hz + 1.5, 0.8), 0.8, flung(0.35, 0, -0.5, 1 + tail), [1, 3], [1.4, 2.4], 0.16)
      this.glints(count(40 * tail * sun, dt), from, flung(0.45, 0, -1, rise), sun)
    }
  }

  /** Steps the water in the air. */
  update(dt: number) {
    this.drops.update(dt)
    this.mist.update(dt)
    // Drops coming down knock the water about and leave it white.
    for (let i = this.landings.length - 1; i >= 0; i--) {
      const landing = this.landings[i]!
      landing.in -= dt
      if (landing.in > 0) continue
      this.stir?.splash(landing.x, landing.z, landing.radius, 0.04, 0.5)
      this.landings.splice(i, 1)
    }
  }

  dispose() {
    this.drops.dispose()
    this.mist.dispose()
  }

  /** Fast thin sheets of water: small crisp drops drawn long along their motion, so they read as streaks. */
  private streaks(n: number, position: Vec, spread: number, velocity: Vec, jitter: Vec, life: readonly [number, number]) {
    this.emit(this.drops, { count: n, position, spread, velocity, jitter, life, size: [0.11, 0.08], scale: [0.6, 1.4], color: WATER, color2: WATER_DARK, alpha: 0.6, gravity: GRAVITY, drag: 0.35, stretch: 0.025, shape: 'drop', scatter: true })
  }

  /** What a sheet breaks into: bigger drops, flung every way, that arc and fall back. */
  private droplets(n: number, position: Vec, spread: number, velocity: Vec, burst: number, life: readonly [number, number]) {
    if (n > 0 && this.stir && Math.random() < this.dt * LANDINGS) this.land(position, spread + burst * 0.4, velocity)
    this.emit(this.drops, { count: n, position, spread, velocity, burst, jitter: [0.8, 0.8, 0.8], life, size: [0.08, 0.06], scale: [0.5, 1.4], color: WATER, color2: WATER_DARK, alpha: 0.85, gravity: GRAVITY, drag: DROP_DRAG, stretch: 0.015, shape: 'drop', scatter: true })
  }

  /** Mist: big soft billows that hang, roll and drift off downwind. */
  private puffs(n: number, position: Vec, spread: number, velocity: Vec, size: [number, number], life: readonly [number, number], alpha: number) {
    this.emit(this.mist, { count: n, position, spread, velocity, jitter: [1.2, 0.8, 1.2], life, size, scale: [0.7, 1.3], color: MIST, color2: WATER_DARK, alpha, gravity: 0.25, drag: 1.5, turbulence: 0.9, wind: 0.85, spin: [-0.5, 0.5], hold: true, shape: 'puff', scatter: true })
  }

  /** The sun caught in the flying water: only by day, and brighter the brighter it is. */
  private glints(n: number, position: Vec, velocity: Vec, sun: number) {
    this.emit(this.glow, { count: n, position, spread: 0.8, velocity, jitter: [1.5, 1.5, 1.5], life: [0.3, 0.7], size: [0.08, 0.03], color: 0xffffff, color2: 0xd8ecff, alpha: 1, gravity: 9.8, drag: 0.3, stretch: 0.02, glow: 1 + 1.5 * sun, shape: 'glare' })
  }

  /**
   * Notes where drops thrown from `from` at `velocity` come down on the water: their arc's end, slowed
   * by their drag, `radius` metres about it, for `update` to stir the water there when they arrive.
   */
  private land(from: Vec, radius: number, velocity: Vec) {
    const up = velocity[1]
    const high = Math.max(0, from[1] - this.level)
    const t = (up + Math.sqrt(up * up + 2 * GRAVITY * high)) / GRAVITY
    const reach = (1 - Math.exp(-DROP_DRAG * t)) / DROP_DRAG
    this.landings.push({ in: t, x: from[0] + velocity[0] * reach, z: from[2] + velocity[2] * reach, radius })
  }

  private emit(pool: Particles, spec: EmitSpec) {
    if (spec.count > 0) pool.emit(spec)
  }
}

type Vec = readonly [number, number, number]
