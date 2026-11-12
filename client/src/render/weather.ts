import { smoothstep } from '@/math'

/** The weather at one moment, worked out from the server's clock so that every player has the same. */
export interface Weather {
  /** How much of the sky the clouds cover, 0 to 1. */
  cover: number
  /** How hard it rains, 0 to 1. */
  rain: number
  /** How wet the ground is, 0 to 1: it comes with the rain and dries some minutes after it stops. */
  wet: number
  /** How much of a rainbow shows, 0 to 1: as a shower clears, while the sun is out. */
  rainbow: number
  /** A lightning flash's brightness this instant, 0 for none. */
  flash: number
}

/** Seconds between the weather's turning points: a spell of rain or clear sky lasts some tens of minutes. */
const SPELL = 1500
/** Where on the weather's 0 to 1 scale rain begins and is at its hardest. */
const RAIN_FROM = 0.74
const RAIN_FULL = 0.86
/** How long the ground stays wet after the rain stops, seconds. */
const DRYING = 300
/** Lightning: in slots of this many seconds, each with this chance of a strike, once the rain is this hard. */
const STRIKE_SLOT = 6
const STRIKE_CHANCE = 0.4
const STORM = 0.6

/**
 * `?weather=0.9` in the address pins the weather's scale (0 clear, about 0.8 rain, 1 a storm), to look
 * at one kind of weather; lightning still comes and goes.
 */
const pinned = (() => {
  if (typeof location === 'undefined') return null
  const value = new URLSearchParams(location.search).get('weather')
  if (value === null) return null
  const n = Number(value)
  return Number.isFinite(n) ? Math.min(1, Math.max(0, n)) : null
})()

/** The weather `seconds` into the server's clock, into `out`. A winter world has heavier cloud and no rain. */
export function weatherAt(seconds: number, wintry: boolean, out: Weather = createWeather()): Weather {
  const n = spell(seconds)
  if (wintry) {
    out.cover = 0.6 + 0.3 * n
    out.rain = out.wet = out.rainbow = out.flash = 0
    return out
  }
  out.cover = 0.15 + 0.8 * n
  out.rain = smoothstep(RAIN_FROM, RAIN_FULL, n)
  out.wet = smoothstep(RAIN_FROM - 0.06, RAIN_FULL - 0.04, Math.max(n, spell(seconds - DRYING)))
  // A rainbow as the rain clears: the weather on its way down through the edge of the rain.
  const clearing = Math.min(1, Math.max(0, (n - spell(seconds + 120)) * 40))
  out.rainbow = smoothstep(RAIN_FROM - 0.12, RAIN_FROM - 0.04, n) * (1 - smoothstep(RAIN_FROM - 0.01, RAIN_FROM + 0.04, n)) * clearing
  out.flash = out.rain > STORM ? lightning(seconds) * smoothstep(STORM, 0.95, out.rain) : 0
  return out
}

export function createWeather(): Weather {
  return { cover: 0.4, rain: 0, wet: 0, rainbow: 0, flash: 0 }
}

/**
 * The weather's scale at `seconds`, 0 (clear) to 1 (a storm): smooth noise through a value every `SPELL`
 * seconds, skewed so that rain is the exception, with a smaller, quicker wobble over it.
 */
function spell(seconds: number): number {
  if (pinned !== null) return pinned
  const knot = (k: number) => hash(k) ** 1.4
  const at = (t: number, period: number, salt: number) => {
    const x = t / period
    const k = Math.floor(x)
    const f = x - k
    const s = f * f * f * (f * (f * 6 - 15) + 10)
    return knot(k * 3 + salt) * (1 - s) + knot((k + 1) * 3 + salt) * s
  }
  return Math.min(1, Math.max(0, at(seconds, SPELL, 0) + (at(seconds, SPELL / 4, 1) - 0.4) * 0.15))
}

/**
 * Lightning `seconds` into the clock: in each slot of `STRIKE_SLOT` seconds a strike comes or does not,
 * at a moment the slot's hash picks, as a flicker of three quick flashes.
 */
function lightning(seconds: number): number {
  const slot = Math.floor(seconds / STRIKE_SLOT)
  let flash = 0
  // The slot before too, so a strike late in it carries over the edge.
  for (const k of [slot - 1, slot]) {
    if (hash(k * 7 + 3) >= STRIKE_CHANCE) continue
    const t = seconds - (k * STRIKE_SLOT + hash(k * 13 + 1) * (STRIKE_SLOT - 1))
    if (t < -0.3 || t > 1) continue
    flash += Math.exp(-((t / 0.05) ** 2)) + 0.7 * Math.exp(-(((t - 0.16) / 0.04) ** 2)) + 0.45 * Math.exp(-(((t - 0.34) / 0.07) ** 2))
  }
  return flash
}

/** A hash of an integer, 0 to 1. */
function hash(n: number): number {
  let h = Math.imul(n ^ 0x27d4eb2d, 0x85ebca6b)
  h ^= h >>> 15
  h = Math.imul(h, 0xc2b2ae35)
  h ^= h >>> 13
  return (h >>> 0) / 4294967296
}
