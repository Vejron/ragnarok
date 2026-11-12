import { Color, hueOf, skyRadiance, transmittance, type Grade, type Sky as EngineSky } from '@ragnarok/engine'
import { flatGroundLight } from './groundlight'
import type { Palette } from './look'
import { smoothstep } from '@/math'
import { DAY_SECONDS, DAY_START_PHASE } from '@/sim/consts'

export { DAY_SECONDS }

/** How high the sun gets at noon. Higher and hillsides all fall in the same lit band of the toon ramp; here a slope of 5° away from it drops a band. */
const NOON_ELEVATION = (35 * Math.PI) / 180
/** The moon never sinks below this, so its light never comes from under the ground. */
const MOON_MIN_ELEVATION = (25 * Math.PI) / 180
/** The compass bearing of the noon sun, in the xz plane: where the old fixed sun stood. */
const NOON_BEARING = normalize2(70, 55)

const NIGHT_SKY = Color.hex(0x0b1630)
const NIGHT_HEMI_SKY = Color.hex(0x2a3a66)
const NIGHT_HEMI_GROUND = Color.hex(0x1a2418)
const MOON = Color.hex(0x9fb4ff)
/** Game days from one full moon to the next. */
const LUNAR_DAYS = 8
/** The sky's own glow at night: what is left of the twilight, airglow and starlight, against `NIGHT_SKY`. */
const NIGHT_GLOW = 0.7
/** How bright the sun's disc is drawn against the sunlight on a white cloud. */
const SUN_DISC = 24
/**
 * How much of the atmosphere's change of colour the palette takes on: the sunlight's reddening toward
 * the horizon, and the sky fill's. Not all of it: a toy world under a fully red sun goes muddy.
 */
const SUN_SHIFT = 0.75
const FILL_SHIFT = 0.5
/** The most the twilight sky is brightened (`twilightGain`). */
const TWILIGHT_GAIN = 14
const MOON_INTENSITY = 0.5
/** The fill at night, as a share of the palette's. */
const NIGHT_HEMI_SCALE = 0.35

/**
 * Where in the day `tick` falls, 0 to 1: 0 midnight, 0.25 sunrise, 0.5 noon, 0.75 sunset. Every player
 * agrees, since it comes from the server's tick. `?time=0.5` in the address pins it, to look at one time.
 */
export function dayPhase(tick: number, tickRate: number): number {
  if (pinned !== null) return pinned
  const phase = (tick / tickRate / DAY_SECONDS + DAY_START_PHASE) % 1
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
  /** Unit vector toward the sun, wherever it is (below the horizon at night). */
  readonly sunDir: [number, number, number]
  /** Unit vector toward the moon: where the light comes from at night. */
  readonly moonDir: [number, number, number]
  /** How much of the moon shows (0 by day) and of the stars, 0 to 1. */
  moon: number
  stars: number
  /** How much of the night's own glow the sky has, 0 by day to 1 at night. */
  night: number
  /**
   * How much brighter the atmosphere is drawn than its physics says, as an eye adapts: 1 by day, more
   * in the twilight, so the afterglow shows instead of going black within minutes of sunset.
   */
  skyGain: number
  readonly lightColor: Color
  lightIntensity: number
  /** The horizon's colour all round: the background and fog where no sky is drawn, and the fog of particles. */
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
const scratch = { a: [0, 0, 0], b: [0, 0, 0], dir: [0, 0, 0], hue: new Color(), noonHue: new Color() }

/**
 * The atmosphere at noon, which the palette's colours stand for: the sun through the air, the sky's
 * light on an upturned face, and the horizon all round. Other hours change the palette by how these change.
 */
const atNoon = (() => {
  const sun = sunDirection(0.5, [0, 0, 0])
  const light = transmittance(sun[1]!, [0, 0, 0])
  return { sun, light, fill: skyFill(sun, [0, 0, 0]), horizon: horizonAround(sun, [0, 0, 0]) }
})()

/** Where the sun is at `phase`, a unit vector into `out`: up at 35° at noon, down as far at midnight. */
function sunDirection(phase: number, out: number[]): number[] {
  // The sun turns about the rise-set line, east to west through the south (the noon bearing).
  const angle = 2 * Math.PI * (phase - 0.25)
  const tilt = Math.PI / 2 - NOON_ELEVATION
  const across = Math.cos(angle)
  const over = Math.sin(angle)
  const [nx, nz] = NOON_BEARING
  // East is the noon bearing turned a quarter, so the sun rises on one side and sets on the other.
  out[0] = across * -nz + over * Math.sin(tilt) * nx
  out[1] = over * Math.cos(tilt)
  out[2] = across * nx + over * Math.sin(tilt) * nz
  return out
}

/** The sky's radiance just above the horizon, averaged round the compass, for the sun at `sun`. */
function horizonAround(sun: readonly number[], out: number[]): number[] {
  const up = Math.sin((3 * Math.PI) / 180)
  const flat = Math.sqrt(1 - up * up)
  out[0] = out[1] = out[2] = 0
  const n = 8
  for (let i = 0; i < n; i++) {
    const a = (2 * Math.PI * i) / n
    scratch.dir[0] = Math.cos(a) * flat
    scratch.dir[1] = up
    scratch.dir[2] = Math.sin(a) * flat
    skyRadiance(scratch.dir, sun, scratch.b)
    for (let c = 0; c < 3; c++) out[c]! += scratch.b[c]! / n
  }
  return out
}

/** Roughly the sky's light on a face turned up: the zenith, and a ring 30° up round the compass. */
function skyFill(sun: readonly number[], out: number[]): number[] {
  skyRadiance([0, 1, 0], sun, out)
  const up = 0.5
  const flat = Math.sqrt(1 - up * up)
  const n = 6
  for (let i = 0; i < n; i++) {
    const a = (2 * Math.PI * i) / n
    scratch.dir[0] = Math.cos(a) * flat
    scratch.dir[1] = up
    scratch.dir[2] = Math.sin(a) * flat
    skyRadiance(scratch.dir, sun, scratch.b)
    for (let c = 0; c < 3; c++) out[c]! += scratch.b[c]! / n
  }
  return out
}

/**
 * Multiplies `color` per channel by how the hue of `now` differs from the hue of `then`, scaled so the
 * strongest channel keeps its value: the palette's colour, shifted as the atmosphere's light shifts.
 */
function shiftHue(color: Color, now: readonly number[], then: readonly number[], share: number): Color {
  const a = hueOf(now, scratch.hue)
  const b = hueOf(then, scratch.noonHue)
  const r = a.r / b.r, g = a.g / b.g, bl = a.b / b.b
  const peak = Math.max(r, g, bl, 1e-9)
  const k = (x: number) => 1 - share + share * (x / peak)
  return color.set(color.r * k(r), color.g * k(g), color.b * k(bl))
}

/** The light at `phase` (see `dayPhase`) for `palette`, written into `out`. At noon it is the palette exactly. */
export function skyAt(phase: number, palette: Palette, out: Sky = createSky()): Sky {
  const sun = sunDirection(phase, out.sunDir)
  const [sx, sy, sz] = sun as [number, number, number]
  /** The sun's height: its elevation's sine, 1 straight up. */
  const height = sy
  const noonHeight = Math.sin(NOON_ELEVATION)

  dayColor.sky.setHex(palette.sky)
  dayColor.hemiSky.setHex(palette.hemiSky)
  dayColor.hemiGround.setHex(palette.hemiGround)
  dayColor.sun.setHex(palette.sun)

  // Night to day as the sun climbs out of the twilight. The day's colours shift as the atmosphere's
  // light does: the horizon reddens toward a low sun, and the sky's fill with it.
  const day = smoothstep(-0.15, 0.25, height)
  const balance = balanceFor(palette)
  horizonAround(sun, scratch.a)
  out.skyGain = twilightGain(scratch.a)
  const gain = out.skyGain
  out.sky.set(scratch.a[0]! * balance.r * gain, scratch.a[1]! * balance.g * gain, scratch.a[2]! * balance.b * gain)
  out.night = 1 - day
  out.sky.r += NIGHT_SKY.r * NIGHT_GLOW * out.night
  out.sky.g += NIGHT_SKY.g * NIGHT_GLOW * out.night
  out.sky.b += NIGHT_SKY.b * NIGHT_GLOW * out.night
  out.hemiSky.copy(dayColor.hemiSky)
  // Only while the sun is up: after it the fill's light is the night's, not the afterglow's red.
  const shift = FILL_SHIFT * smoothstep(-0.02, 0.08, height)
  if (shift > 0) shiftHue(out.hemiSky, skyFill(sun, scratch.a), atNoon.fill, shift)
  out.hemiSky.lerp(NIGHT_HEMI_SKY, 1 - day)
  out.hemiGround.copy(NIGHT_HEMI_GROUND).lerp(dayColor.hemiGround, day)
  out.hemiIntensity = palette.hemiIntensity * (NIGHT_HEMI_SCALE + (1 - NIGHT_HEMI_SCALE) * day)

  // The moon stands opposite the sun, held up off the horizon.
  const minHeight = Math.sin(MOON_MIN_ELEVATION)
  const level = Math.hypot(sx, sz) || 1
  const up = Math.max(-sy, minHeight)
  const flat = Math.sqrt(1 - up * up) / level
  const moon = out.moonDir
  moon[0] = -sx * flat
  moon[1] = up
  moon[2] = -sz * flat
  out.moon = smoothstep(-0.03, 0.15, -height)
  out.stars = 1 - smoothstep(-0.2, -0.04, height)

  const dir = out.lightDir
  if (height >= 0) {
    // The sun fades in off the horizon and reaches the palette's strength by noon, reddening as the air it comes through thickens.
    dir[0] = sx
    dir[1] = sy
    dir[2] = sz
    shiftHue(out.lightColor.copy(dayColor.sun), transmittance(height, scratch.a), atNoon.light, SUN_SHIFT)
    out.lightIntensity = palette.sunIntensity * smoothstep(0, 0.2, height) * (0.75 + 0.25 * Math.min(1, height / noonHeight))
  } else {
    // By night the light is the moon's, fading in as the sun goes down.
    dir[0] = moon[0]
    dir[1] = moon[1]
    dir[2] = moon[2]
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
    sunDir: [0, 1, 0],
    moonDir: [0, 1, 0],
    moon: 0,
    stars: 0,
    night: 0,
    skyGain: 1,
    lightColor: new Color(),
    lightIntensity: 1,
    sky: new Color(),
    hemiSky: new Color(),
    hemiGround: new Color(),
    hemiIntensity: 1,
    tint: new Color(),
  }
}

/**
 * How the atmosphere's radiance is scaled for `palette`, per channel: so that the horizon at noon,
 * averaged round the compass, is the palette's sky. Kept per palette.
 */
function balanceFor(palette: Palette): Color {
  let balance = balances.get(palette)
  if (!balance) {
    const sky = new Color().setHex(palette.sky)
    balance = new Color(sky.r / atNoon.horizon[0]!, sky.g / atNoon.horizon[1]!, sky.b / atNoon.horizon[2]!)
    balances.set(palette, balance)
  }
  return balance
}

const balances = new WeakMap<Palette, Color>()

/**
 * The twilight's gain (`Sky.skyGain`) for a horizon of radiance `horizon`: the square root of how much
 * darker it is than at noon, the way an eye's adapting halves a fall of light on a log scale, up to
 * `TWILIGHT_GAIN`, past which the night is left dark.
 */
function twilightGain(horizon: readonly number[]): number {
  const luma = (c: readonly number[]) => 0.2126 * c[0]! + 0.7152 * c[1]! + 0.0722 * c[2]!
  const share = luma(horizon) / luma(atNoon.horizon)
  return Math.min(TWILIGHT_GAIN, Math.max(1, 1 / Math.sqrt(Math.max(share, 1e-9))))
}

/**
 * Sets the engine's `sky` from `light` (what `skyAt` worked out for `palette` at this hour): the sun, the
 * moon showing `moonPhase`, the stars turned for `phase`, and the atmosphere scaled to the palette.
 */
export function drawnSky(light: Sky, palette: Palette, phase: number, moonPhase: number, sky: EngineSky) {
  const [sx, sy, sz] = light.sunDir
  sky.sunDir.set(sx, sy, sz)
  const [mx, my, mz] = light.moonDir
  sky.moonDir.set(mx, my, mz)
  sky.moon = light.moon
  sky.moonPhase = moonPhase
  sky.stars = light.stars
  sky.starTurn = 2 * Math.PI * phase
  sky.tint.copy(balanceFor(palette)).multiplyScalar(light.skyGain)
  sky.night.copy(NIGHT_SKY).multiplyScalar(NIGHT_GLOW * light.night)
  sky.sunDisc = SUN_DISC
}

/** How fast the wind carries the clouds, m/s along x and z. */
const WIND = [5.5, 3.2] as const
/** The cloud cover the weather swings about, fair and winter, and how far either way. */
const FAIR_COVER = 0.4
const WINTER_COVER = 0.72
const COVER_SWING = 0.18

/**
 * The cloud layer `seconds` into the server's clock, the same for every player: how far the wind has
 * carried it (into `drift`, metres), and how much of the sky it covers, which drifts slowly between
 * nearly clear and well clouded over the hours.
 */
export function cloudsAt(seconds: number, wintry: boolean, drift: [number, number]): number {
  // Wrapped to the cloud map's repeat, so the drift stays small enough for 32-bit floats.
  drift[0] = (seconds * WIND[0]) % CLOUD_REPEAT
  drift[1] = (seconds * WIND[1]) % CLOUD_REPEAT
  const swing = Math.sin((2 * Math.PI * seconds) / 2700) * 0.7 + Math.sin((2 * Math.PI * seconds) / 1100 + 1.3) * 0.3
  return (wintry ? WINTER_COVER : FAIR_COVER) + COVER_SWING * swing
}

/** A whole number of cloud map tiles (`CLOUD_TILE`, 6 km), past which the drift wraps unseen. */
const CLOUD_REPEAT = 60000

const GOLDEN_HIGHLIGHTS = Color.hex(0xffeedd)
const GOLDEN_SHADOWS = Color.hex(0xe4ecff)
const DAY_SHADOWS = Color.hex(0xf8fbff)
const WHITE = new Color(1, 1, 1)
/** How much the corners darken, all day. */
const VIGNETTE = 0.16
/** How far dim colours go toward the blue of rod vision at full night (`Grade.purkinje`). */
const PURKINJE = 0.85

/**
 * The colour grade for the light at this hour (`light`, from `skyAt`): warm highlights and cool
 * shadows round the golden hour, a touch more colour then, a slight cool in the day's shade, and by
 * night dim colours drifting toward the blue-grey of moonlit vision.
 */
export function gradeAt(light: Sky, grade: Grade): Grade {
  const height = light.sunDir[1]
  const golden = 1 - smoothstep(0.0, 0.3, Math.abs(height - 0.06))
  grade.highlights.copy(WHITE).lerp(GOLDEN_HIGHLIGHTS, golden)
  grade.shadows.copy(WHITE).lerp(DAY_SHADOWS, 1 - light.night).lerp(GOLDEN_SHADOWS, golden)
  grade.saturation = 1 + 0.1 * golden
  grade.purkinje = PURKINJE * light.night
  grade.vignette = VIGNETTE
  return grade
}

/**
 * The moon's phase angle (see `Sky.moonPhase`) `seconds` into the server's clock: full on the first
 * night, new `LUNAR_DAYS / 2` days later. It waxes with its lit side toward the setting sun.
 */
export function moonPhaseAt(seconds: number): number {
  const days = seconds / DAY_SECONDS + DAY_START_PHASE
  const lunation = (((days / LUNAR_DAYS) % 1) + 1) % 1
  return 2 * Math.PI * lunation
}

/**
 * How dark it is at `phase` for driving: 0 by day, 1 from a little after sunset to a little before
 * sunrise, the sun's last 10° or so between. Headlights come on by it.
 */
export function nightAt(phase: number): number {
  const height = Math.sin(2 * Math.PI * (phase - 0.25)) * Math.sin(NOON_ELEVATION)
  return 1 - smoothstep(-0.02, 0.18, height)
}

function normalize2(x: number, z: number): [number, number] {
  const length = Math.hypot(x, z)
  return [x / length, z / length]
}
