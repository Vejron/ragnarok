import { Texture, type Vec3 } from '@ragnarok/engine'

/**
 * Wakes: what moves through open water carves it. Each source is a point something passed, dropped
 * every metre or so along its way: it digs a short-lived trench where the hull was and sends a ring of
 * ripples out from there. A line of them from something quick overlaps into the narrow V behind a speed
 * boat; something slow leaves wide rings. Only drawn: nothing rides on them.
 *
 * They are worked out here, once a frame, into a small map around the camera (`texture`): each source
 * is drawn only over the band its ring covers, from a profile worked out once. The water's shader then
 * reads its height, slope and foam with one texture fetch (`WAKES_WGSL`), however many sources there are.
 */

/** Texels a side of the wake map, and metres a texel: the map covers this many metres square about the camera. */
export const WAKE_GRID = 192
export const WAKE_TEXEL = 0.5
const WAKE_SIZE = WAKE_GRID * WAKE_TEXEL
/** The heights (metres) and slopes the map's bytes reach, either way. */
const HEIGHT_RANGE = 0.5
const SLOPE_RANGE = 1.5
/** Seconds a source lasts. */
const LIFE = 4
/** How fast its ring runs out, m/s, from how far out it starts, metres. */
const RING_SPEED = 3.2
const RING_START = 0.6
/** The ring's width and its ripples' wavelength, metres. */
const RING_WIDTH = 1.1
const RING_LENGTH = 2.2
/** Seconds the ring takes to die to a third. */
const RING_FADE = 1.3
/** The trench a hull digs: its radius (metres), and how quickly it fills back in, per second. */
const TROUGH = 1.1
const TROUGH_FILL = 3.5
/** Seconds the churned foam on a source takes to die to a third. */
const FOAM_FADE = 1.2
/** A ring or trench lower than this, metres, is not drawn. */
const FAINT = 0.02
/** Most sources drawn in a frame: the strongest, nearest the camera. */
const MOST = 128
/** Sources further than this from the camera count for half as much when choosing, metres. */
const NEAR = 30

/** The ring's profile across its front, `RING_REACH` either side, sampled every `PROFILE_STEP` metres: height and its slope. */
const RING_REACH = 2 * RING_WIDTH
const PROFILE_STEP = 0.02
const PROFILE = (() => {
  const k = (Math.PI * 2) / RING_LENGTH
  const w2 = RING_WIDTH * RING_WIDTH
  const n = Math.ceil((2 * RING_REACH) / PROFILE_STEP) + 1
  const height = new Float32Array(n)
  const slope = new Float32Array(n)
  for (let i = 0; i < n; i++) {
    const front = -RING_REACH + i * PROFILE_STEP
    const envelope = Math.exp((-front * front) / w2)
    height[i] = envelope * Math.cos(k * front)
    slope[i] = envelope * ((-2 * front * Math.cos(k * front)) / w2 - k * Math.sin(k * front))
  }
  return { height, slope }
})()

interface Source {
  x: number
  z: number
  born: number
  strength: number
}

/** Something moving through the water at `(x, z)` stirs it by `strength` metres (a ring's height). */
export type DropWake = (x: number, z: number, strength: number) => void

const byte = (value: number) => Math.round(Math.min(1, Math.max(0, value)) * 255)

export class Wakes {
  private sources: Source[] = []
  private clock = 0
  /** Height, slope x, slope z and foam at each texel, before they are packed into `pixels`. */
  private readonly field = new Float32Array(WAKE_GRID * WAKE_GRID * 4)
  private readonly pixels = new Uint8Array(WAKE_GRID * WAKE_GRID * 4)
  /** Whether the map has anything on it. */
  private drawn = true
  /** The map: height, slope x and slope z about mid-grey, and foam, `wakeOf` in WGSL undoes it. */
  readonly texture = new Texture({ data: this.pixels, width: WAKE_GRID, height: WAKE_GRID }, { srgb: false, filter: 'linear', wrap: 'clamp', mipmaps: false })
  /** Where the map's corner is (x, z) and how wide it is, metres. */
  readonly area: [number, number, number, number] = [0, 0, WAKE_SIZE, 0]

  drop(x: number, z: number, strength: number) {
    if (strength > 0) this.sources.push({ x, z, born: this.clock, strength })
  }

  /** How many sources are alive. */
  get count() {
    return this.sources.length
  }

  /** The wakes' height, slope x, slope z and foam at `(x, z)` on the map, unpacked as the shader reads them; for tests. */
  sample(x: number, z: number): [number, number, number, number] {
    const i = Math.floor((x - this.area[0]) / WAKE_TEXEL)
    const j = Math.floor((z - this.area[1]) / WAKE_TEXEL)
    if (i < 0 || j < 0 || i >= WAKE_GRID || j >= WAKE_GRID) return [0, 0, 0, 0]
    const at = (j * WAKE_GRID + i) * 4
    const p = this.pixels
    return [(p[at]! / 255 * 2 - 1) * HEIGHT_RANGE, (p[at + 1]! / 255 * 2 - 1) * SLOPE_RANGE, (p[at + 2]! / 255 * 2 - 1) * SLOPE_RANGE, p[at + 3]! / 255]
  }

  /**
   * Ages the sources by `dt` seconds and draws them into the map about `camera`. Returns whether the
   * map changed (it is left alone while the water is still).
   */
  update(dt: number, camera: Readonly<Vec3>): boolean {
    this.clock += dt
    this.sources = this.sources.filter((source) => this.clock - source.born < LIFE)
    if (this.sources.length === 0 && !this.drawn) return false
    // On whole texels, so the wakes do not crawl as the camera moves.
    this.area[0] = Math.floor(camera.x / WAKE_TEXEL) * WAKE_TEXEL - WAKE_SIZE / 2
    this.area[1] = Math.floor(camera.z / WAKE_TEXEL) * WAKE_TEXEL - WAKE_SIZE / 2
    this.field.fill(0)
    const weight = (source: Source) => (source.strength * Math.exp(-(this.clock - source.born) / RING_FADE)) / (1 + Math.hypot(source.x - camera.x, source.z - camera.z) / NEAR)
    const drawn = this.sources.length > MOST ? [...this.sources].sort((a, b) => weight(b) - weight(a)).slice(0, MOST) : this.sources
    for (const source of drawn) this.splat(source)
    const { field, pixels } = this
    for (let at = 0; at < field.length; at += 4) {
      pixels[at] = byte(field[at]! / HEIGHT_RANGE / 2 + 0.5)
      pixels[at + 1] = byte(field[at + 1]! / SLOPE_RANGE / 2 + 0.5)
      pixels[at + 2] = byte(field[at + 2]! / SLOPE_RANGE / 2 + 0.5)
      pixels[at + 3] = byte(field[at + 3]!)
    }
    this.drawn = this.sources.length > 0
    this.texture.needsUpdate()
    return true
  }

  /** Adds one source's ring, trench and foam to the field, over only the texels they reach. */
  private splat(source: Source) {
    const age = this.clock - source.born
    const ring = (source.strength * Math.exp(-age / RING_FADE)) / (1 + 0.3 * age)
    const dig = source.strength * Math.exp(-age * TROUGH_FILL)
    const foam = source.strength * 4 * Math.exp(-age / FOAM_FADE)
    const radius = RING_START + RING_SPEED * age
    if (ring > FAINT) this.band(source, Math.max(0, radius - RING_REACH), radius + RING_REACH, radius, ring, 0, 0)
    if (dig > FAINT || foam > 0.02) this.band(source, 0, 2.5 * TROUGH, 0, 0, dig, foam)
  }

  /**
   * Adds a source's ring (of height `ring`, its crest `radius` out) or its trench (`dig` deep, with
   * `foam`) to every texel between `inner` and `outer` metres from it, row by row over only the spans
   * the band crosses.
   */
  private band(source: Source, inner: number, outer: number, radius: number, ring: number, dig: number, foam: number) {
    const [x0, z0] = this.area
    const { field } = this
    const { height, slope } = PROFILE
    const first = Math.max(0, Math.floor((source.z - outer - z0) / WAKE_TEXEL))
    const last = Math.min(WAKE_GRID - 1, Math.floor((source.z + outer - z0) / WAKE_TEXEL))
    const inner2 = inner * inner
    const outer2 = outer * outer
    const trough2 = TROUGH * TROUGH
    for (let j = first; j <= last; j++) {
      const dz = z0 + (j + 0.5) * WAKE_TEXEL - source.z
      const dz2 = dz * dz
      if (dz2 > outer2) continue
      const out = Math.sqrt(outer2 - dz2)
      const hole = dz2 < inner2 ? Math.sqrt(inner2 - dz2) : 0
      // The band's span either side of the source, skipping the middle it does not reach.
      for (let side = hole > 0 ? 0 : 1; side < 2; side++) {
        const from = hole > 0 ? (side === 0 ? -out : hole) : -out
        const to = hole > 0 ? (side === 0 ? -hole : out) : out
        const i0 = Math.max(0, Math.floor((source.x + from - x0) / WAKE_TEXEL))
        const i1 = Math.min(WAKE_GRID - 1, Math.floor((source.x + to - x0) / WAKE_TEXEL))
        let at = (j * WAKE_GRID + i0) * 4
        for (let i = i0; i <= i1; i++, at += 4) {
          const dx = x0 + (i + 0.5) * WAKE_TEXEL - source.x
          const r2 = dx * dx + dz2
          if (r2 < inner2 || r2 > outer2) continue
          const r = Math.sqrt(r2)
          let h: number
          let dr: number
          let churn = 0
          if (ring > 0) {
            const step = Math.round((r - radius + RING_REACH) / PROFILE_STEP)
            if (step < 0 || step >= height.length) continue
            h = ring * height[step]!
            dr = ring * slope[step]!
          } else {
            const tr2 = r2 / trough2
            const e = Math.exp(-tr2)
            h = -dig * e
            dr = (dig * e * 2 * r) / trough2
            churn = foam * Math.exp(-tr2 * 0.5)
          }
          const toward = r > 1e-3 ? dr / r : 0
          field[at] = field[at]! + h
          field[at + 1] = field[at + 1]! + toward * dx
          field[at + 2] = field[at + 2]! + toward * dz
          field[at + 3] = field[at + 3]! + churn
        }
      }
    }
  }
}

/**
 * Drops a mover's wake sources a step apart along its way: `step` metres between them, so a quick
 * one leaves them as close as a slow one does.
 */
export class WakeTrail {
  private last: { x: number; z: number } | null = null

  constructor(private readonly step = 1.2) {}

  /** The mover is at `(x, z)`, stirring the water by `strength`; drops a source if it has come far enough. */
  move(drop: DropWake | undefined, x: number, z: number, strength: number) {
    if (!drop) return
    if (this.last && Math.hypot(x - this.last.x, z - this.last.z) < this.step) return
    drop(x, z, strength)
    this.last = { x, z }
  }

  /** The mover has left the water. */
  lift() {
    this.last = null
  }
}

const f = (value: number) => value.toFixed(6)

/**
 * `fn wake(p: vec2f) -> vec4f`: the wakes' height at `p`, their slope `(dh/dx, dh/dz)`, and how churned
 * the water is there (0 to 1), read from the material's `wakeMap` texture over its `wakeArea`. They fade
 * out towards the map's edge.
 */
export const WAKES_WGSL = /* wgsl */ `
fn wake(p: vec2f) -> vec4f {
  let uv = (p - params.wakeArea.xy) / params.wakeArea.z;
  let edge = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
  if (edge <= 0.0) {
    return vec4f(0.0);
  }
  let t = textureSampleLevel(wakeMap, materialSampler, uv, 0.0);
  let unpacked = vec4f((t.x * 2.0 - 1.0) * ${f(HEIGHT_RANGE)}, (t.yz * 2.0 - 1.0) * ${f(SLOPE_RANGE)}, t.w);
  return unpacked * smoothstep(0.0, 0.08, edge);
}
`
