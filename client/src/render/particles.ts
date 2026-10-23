import { ParticleSystem, type ParticleSpec as EngineSpec } from '@ragnarok/engine'

/** How one particle moves and looks over its life. */
export interface ParticleSpec {
  position: { x: number; y: number; z: number } | readonly [number, number, number]
  velocity: readonly [number, number, number]
  /** Seconds. */
  life: number
  /** Diameter at birth and at death, metres. */
  size: [number, number]
  color: number
  /** Opacity at birth; it fades to nothing by the end. */
  alpha: number
  /** Metres per second² pulling it down; negative floats it up. */
  gravity?: number
  /** Share of its speed lost per second. */
  drag?: number
  /** Bounces off the ground (a stone) instead of passing through it. */
  bounce?: boolean
  /** The colour it has cooled or aged to by the end of its life. */
  colorEnd?: number
  /** Radians per second it turns, or a [min, max] range. */
  spin?: number | readonly [number, number]
  /** Metres of extra length per m/s of speed, along its motion (sparks). */
  stretch?: number
  /** Multiplies the colour past 1 so it blooms. */
  glow?: number
  /** `puff` gives smoke a billowy edge. */
  shape?: 'round' | 'puff'
}

/** The darkest a particle gets at night, against its colour by day. */
const NIGHT_LEVEL = 0.28

/** `color` as it looks in `light` (1 by day, 0 at night). */
export function dimmed(color: number, light: number): number {
  if (light >= 1) return color
  const k = NIGHT_LEVEL + (1 - NIGHT_LEVEL) * Math.max(0, light)
  const channel = (shift: number) => Math.round(((color >> shift) & 0xff) * k) << shift
  return channel(16) | channel(8) | channel(0)
}

/** The height field particles bounce on is sampled this far to a side of the camera, metres, at `CELL` spacing. */
const REACH = 128
const CELL = 2
const FIELD = (REACH * 2) / CELL
/** The field is resampled once the camera has moved this far from its centre, metres, or this often, seconds. */
const RESAMPLE_DISTANCE = 32
const RESAMPLE_SECONDS = 3

/**
 * A pool of soft round billboards in world space: tyre smoke, dust, gravel, spray and sparks. They are
 * simulated and drawn on the GPU (`ParticleSystem`): spawning one writes a 128-byte command, however
 * many are alive. When the pool is full new ones are dropped. The ground a bouncing particle lands on is
 * a height field sampled from `groundAt` around `focus`.
 */
export class Particles {
  readonly root: ParticleSystem
  readonly capacity: number
  /** Ground height under a point, for particles that bounce. */
  groundAt: (x: number, z: number) => number = () => 0
  /** How bright the day is, 1 at noon and 0 at midnight. Particles are not lit, so the plain ones are dimmed by it as they are born. */
  light = 1
  private readonly blend: 'alpha' | 'additive'
  /** When each live particle dies, by the clock below: a tally of what the GPU is holding, for budgets and tests. */
  private readonly deaths: number[] = []
  private clock = 0
  private bounces = false
  private field: Float32Array | null = null
  private fieldAt = { x: Infinity, z: Infinity, time: -Infinity }
  private focus = { x: 0, z: 0 }

  constructor(capacity = 3000, additive = false, halfResolution = false) {
    this.capacity = capacity
    this.blend = additive ? 'additive' : 'alpha'
    // The GPU pool is sized in blocks of 64.
    this.root = new ParticleSystem(Math.ceil(capacity / 64) * 64)
    // The particles are anywhere in the world, so there is no bounding sphere to cull by; effects draw after meshes.
    this.root.renderOrder = additive ? 3 : 2
    // Smoke is big soft overdraw; drawn at half the screen's size it costs a quarter and looks the same.
    this.root.halfResolution = halfResolution
  }

  /** How many particles are alive. */
  get alive() {
    this.reap()
    return this.deaths.length
  }

  spawn(spec: ParticleSpec) {
    this.reap()
    if (this.deaths.length >= this.capacity) return
    this.deaths.push(this.clock + spec.life)
    if (spec.bounce) this.bounces = true
    const dim = this.blend === 'alpha' ? (c: number) => dimmed(c, this.light) : (c: number) => c
    const engine: EngineSpec = {
      position: spec.position,
      velocity: spec.velocity,
      life: spec.life,
      size: spec.size,
      color: dim(spec.color),
      alpha: spec.alpha,
      gravity: spec.gravity,
      drag: spec.drag,
      bounce: spec.bounce,
      blend: this.blend,
      colorEnd: spec.colorEnd === undefined ? undefined : dim(spec.colorEnd),
      spin: spec.spin,
      stretch: spec.stretch,
      glow: spec.glow,
      shape: spec.shape,
    }
    this.root.spawn(engine)
  }

  /** Where the camera is: the ground field for bouncing particles is kept around it. */
  follow(x: number, z: number) {
    this.focus.x = x
    this.focus.z = z
  }

  update(dt: number) {
    this.clock += dt
    this.root.update(dt)
    if (this.bounces) this.resample()
  }

  private reap() {
    // Deaths are not in order; a pool of a few thousand is cheap to filter when it is asked about.
    if (this.deaths.length === 0) return
    let kept = 0
    for (const death of this.deaths) if (death > this.clock) this.deaths[kept++] = death
    this.deaths.length = kept
  }

  private resample() {
    const moved = Math.hypot(this.focus.x - this.fieldAt.x, this.focus.z - this.fieldAt.z)
    if (moved < RESAMPLE_DISTANCE && this.clock - this.fieldAt.time < RESAMPLE_SECONDS) return
    const field = (this.field ??= new Float32Array(FIELD * FIELD))
    const x0 = Math.round((this.focus.x - REACH) / CELL) * CELL
    const z0 = Math.round((this.focus.z - REACH) / CELL) * CELL
    for (let row = 0; row < FIELD; row++) {
      for (let column = 0; column < FIELD; column++) field[row * FIELD + column] = this.groundAt(x0 + column * CELL, z0 + row * CELL) + 0.02
    }
    this.root.setGround(field, FIELD, FIELD, x0, z0, CELL)
    this.fieldAt = { x: this.focus.x, z: this.focus.z, time: this.clock }
  }

  dispose() {
    this.root.removeFromParent()
  }
}

/** A billow of dust about `size` metres across at `at`: soft, uneven puffs that swell, rise and thin out, not one round ball. */
export function dustCloud(smoke: Pick<Particles, 'spawn'>, at: { x: number; y: number; z: number }, size: number) {
  const rand = (min: number, max: number) => min + Math.random() * (max - min)
  const count = Math.min(7, 2 + Math.round(size))
  for (let i = 0; i < count; i++) {
    const r = size * 0.3
    smoke.spawn({
      position: [at.x + rand(-r, r), at.y + rand(0, r * 0.5), at.z + rand(-r, r)],
      velocity: [rand(-0.8, 0.8), rand(0.3, 1.2), rand(-0.8, 0.8)],
      life: rand(1.6, 3),
      size: [size * 0.15, size * rand(0.35, 0.6)],
      color: i % 2 ? 0xb5a88c : 0xcbc2b0,
      colorEnd: 0x9a9080,
      alpha: 0.35,
      gravity: -0.15,
      drag: 1.4,
      spin: [-0.5, 0.5],
      shape: 'puff',
    })
  }
}
