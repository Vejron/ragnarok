import { Color } from '@ragnarok/engine'
import { flatGroundLight } from './groundlight'
import type { Palette } from './look'

/** A whole day, midnight to midnight, in seconds of server time. */
export const DAY_SECONDS = 30 * 60
/** Where in the day a server's tick 0 falls: a fresh server opens at midnight. */
const START_PHASE = 0

/** How high the sun gets at noon. Higher and hillsides all fall in the same lit band of the toon ramp. */
const NOON_ELEVATION = (50 * Math.PI) / 180
/** The moon never sinks below this, so its light never comes from under the ground. */
const MOON_MIN_ELEVATION = (25 * Math.PI) / 180
/** The compass bearing of the noon sun, in the xz plane: where the old fixed sun stood. */
const NOON_BEARING = normalize2(70, 55)

const NIGHT_SKY = Color.hex(0x0b1630)
const NIGHT_HEMI_SKY = Color.hex(0x2a3a66)
const NIGHT_HEMI_GROUND = Color.hex(0x1a2418)
const DUSK_SKY = Color.hex(0xf0a878)
const DUSK_HEMI_SKY = Color.hex(0xffc9a0)
const DUSK_SUN = Color.hex(0xffa040)
const MOON = Color.hex(0x9fb4ff)
const MOON_INTENSITY = 0.5
/** The fill at night, as a share of the palette's. */
const NIGHT_HEMI_SCALE = 0.35

/**
 * Where in the day `tick` falls, 0 to 1: 0 midnight, 0.25 sunrise, 0.5 noon, 0.75 sunset. Every player
 * agrees, since it comes from the server's tick. `?time=0.5` in the address pins it, to look at one time.
 */
export function dayPhase(tick: number, tickRate: number): number {
  if (pinned !== null) return pinned
  const phase = (tick / tickRate / DAY_SECONDS + START_PHASE) % 1
  return phase < 0 ? phase + 1 : phase
}

const pinned = (() => {
  if (typeof location === 'undefined') return null
  const value = new URLSearchParams(location.search).get('time')
  if (value === null) return null
  const phase = Number(value)
  return Number.isFinite(phase) ? ((phase % 1) + 1) % 1 : null
})()

/** The light at one time of day. Colours are linear, as the engine takes them. */
export interface Sky {
  /** Unit vector toward the light: the sun by day, the moon by night. Always above the horizon. */
  readonly lightDir: [number, number, number]
  readonly lightColor: Color
  lightIntensity: number
  /** Background and fog. */
  readonly sky: Color
  readonly hemiSky: Color
  readonly hemiGround: Color
  hemiIntensity: number
  /**
   * The light on flat ground now against noon's, per channel (linear): exactly white at noon, dark and
   * blue at night. Whatever is not lit by the engine (smoke, dust) is multiplied by it to match the ground.
   */
  readonly tint: Color
}

const dayColor = { sky: new Color(), hemiSky: new Color(), hemiGround: new Color(), sun: new Color() }
const ground = { sun: new Color(), hemi: new Color(), noon: new Color() }

/** The light at `phase` (see `dayPhase`) for `palette`, written into `out`. At noon it is the palette exactly. */
export function skyAt(phase: number, palette: Palette, out: Sky = createSky()): Sky {
  // The sun turns about the rise-set line, east to west through the south (the noon bearing).
  const angle = 2 * Math.PI * (phase - 0.25)
  const tilt = Math.PI / 2 - NOON_ELEVATION
  const across = Math.cos(angle)
  const over = Math.sin(angle)
  const [nx, nz] = NOON_BEARING
  // East is the noon bearing turned a quarter, so the sun rises on one side and sets on the other.
  const sx = across * -nz + over * Math.sin(tilt) * nx
  const sy = over * Math.cos(tilt)
  const sz = across * nx + over * Math.sin(tilt) * nz
  /** The sun's height: its elevation's sine, 1 straight up. */
  const height = sy
  const noonHeight = Math.sin(NOON_ELEVATION)

  dayColor.sky.setHex(palette.sky)
  dayColor.hemiSky.setHex(palette.hemiSky)
  dayColor.hemiGround.setHex(palette.hemiGround)
  dayColor.sun.setHex(palette.sun)

  // Night to day as the sun climbs out of the twilight; golden near the horizon on either side of it.
  const day = smoothstep(-0.15, 0.25, height)
  const golden = 1 - smoothstep(0, 0.35, Math.abs(height - 0.03))
  out.sky.copy(NIGHT_SKY).lerp(dayColor.sky, day).lerp(DUSK_SKY, golden * 0.55)
  out.hemiSky.copy(NIGHT_HEMI_SKY).lerp(dayColor.hemiSky, day).lerp(DUSK_HEMI_SKY, golden * 0.4)
  out.hemiGround.copy(NIGHT_HEMI_GROUND).lerp(dayColor.hemiGround, day)
  out.hemiIntensity = palette.hemiIntensity * (NIGHT_HEMI_SCALE + (1 - NIGHT_HEMI_SCALE) * day)

  const dir = out.lightDir
  if (height >= 0) {
    // The sun fades in off the horizon and reaches the palette's strength by noon.
    dir[0] = sx
    dir[1] = sy
    dir[2] = sz
    out.lightColor.copy(dayColor.sun).lerp(DUSK_SUN, golden * 0.8)
    out.lightIntensity = palette.sunIntensity * smoothstep(0, 0.2, height) * (0.75 + 0.25 * Math.min(1, height / noonHeight))
  } else {
    // The moon stands opposite the sun, held up off the horizon, and fades in as the sun goes down.
    const minHeight = Math.sin(MOON_MIN_ELEVATION)
    const level = Math.hypot(sx, sz) || 1
    const up = Math.max(-sy, minHeight)
    const flat = Math.sqrt(1 - up * up) / level
    dir[0] = -sx * flat
    dir[1] = up
    dir[2] = -sz * flat
    out.lightColor.copy(MOON)
    out.lightIntensity = MOON_INTENSITY * smoothstep(0, 0.2, -height)
  }

  ground.sun.copy(out.lightColor).multiplyScalar(out.lightIntensity)
  ground.hemi.copy(out.hemiSky).multiplyScalar(out.hemiIntensity)
  flatGroundLight(ground.sun, dir[1], ground.hemi, out.tint)
  ground.sun.copy(dayColor.sun).multiplyScalar(palette.sunIntensity)
  ground.hemi.copy(dayColor.hemiSky).multiplyScalar(palette.hemiIntensity)
  const noon = flatGroundLight(ground.sun, noonHeight, ground.hemi, ground.noon)
  out.tint.set(out.tint.r / noon.r, out.tint.g / noon.g, out.tint.b / noon.b)
  return out
}

export function createSky(): Sky {
  return {
    lightDir: [0, 1, 0],
    lightColor: new Color(),
    lightIntensity: 1,
    sky: new Color(),
    hemiSky: new Color(),
    hemiGround: new Color(),
    hemiIntensity: 1,
    tint: new Color(),
  }
}

function smoothstep(edge0: number, edge1: number, x: number): number {
  const t = Math.min(1, Math.max(0, (x - edge0) / (edge1 - edge0)))
  return t * t * (3 - 2 * t)
}

function normalize2(x: number, z: number): [number, number] {
  const length = Math.hypot(x, z)
  return [x / length, z / length]
}
