import { audioBus, audioContext } from './audio'
import { nightAt } from '@/render/daynight'
import { STRIKE_SLOT, storminess, strikeDistance, strikeIn } from '@/render/weather'

/**
 * The sound of the world when nobody is shooting: wind over open ground and through the trees, birds by
 * day with a chorus at dawn, crickets and owls at night, water lapping at a shore or running in a
 * stream, rain (drumming on the roof when under one) and thunder that follows the lightning by as long
 * as sound takes to come from the strike. All of it is made here from noise and tones, with nothing to
 * load, and is heard through the world's bus, so a cabin or water muffles it with everything else.
 */

export type Vec3 = readonly [number, number, number]

/** What the listener has about them, which picks what they hear. Worked out by the game a few times a second. */
export interface Surround {
  /** Where the listener's ears are. */
  at: Vec3
  /** The time of day (`dayPhase`): 0 midnight, 0.25 sunrise, 0.5 noon. */
  phase: number
  /** Seconds into the server's clock, for when lightning strikes; null before the clock is known. */
  seconds: number | null
  /** How much of the sky is cloud, and how hard it rains, 0 to 1 (`Weather`). */
  cover: number
  rain: number
  /** A winter world: no crickets, few birds, frozen still water. */
  winter: boolean
  /** How wooded it is about the listener, 0 (open ground) to 1 (deep forest). */
  forest: number
  /** How near water is, 0 (none within `WATER_REACH`) to 1 (at its edge or on it), and how much of it is running, 0 (a lake) to 1 (a stream). */
  water: number
  flowing: number
  /** Metres above the ground: the wind is stronger up on a roof or in the air. */
  height: number
  /** Under a roof. */
  indoors: boolean
}

/** Water farther than this is not heard, metres. */
export const WATER_REACH = 50
/** Trees within this, metres, make a place wooded, and this many of them make it deep forest. */
export const FOREST_REACH = 30
export const FOREST_FULL = 25

/** Seconds a bed's level takes to follow what it should be, as a time constant. */
const EASE = 0.8
/** A bed this quiet is silent, and one silent this many seconds stops its sources. */
const SILENT = 0.001
const SILENT_STOP = 5
/** Seconds between updates of the beds' levels and filters: they change slowly. */
const STEP = 0.1

/** Loudest level of each bed. */
const WIND_LEVEL = 0.1
const LEAVES_LEVEL = 0.07
const WATER_LEVEL = 0.12
const RAIN_LEVEL = 0.12
const ROOF_LEVEL = 0.2
const CRICKET_LEVEL = 0.045

/** Calls a second at most: birds at the peak of the dawn chorus, gulls by water, owls in a wood at night. */
const BIRD_RATE = 1.2
const GULL_RATE = 0.06
const OWL_RATE = 0.04
/** How far off a call comes from, metres, and how high up. */
const CALL_NEAR = 12
const CALL_FAR = 55
/** A strike this long past when its thunder could still be heard is let go: a player who has just joined. */
const THUNDER_LATE = 2

/** The time of the dawn chorus and the evening song, as day phases, and how wide each is. */
const DAWN = 0.27
const DAWN_WIDTH = 0.04
const DUSK = 0.74
const DUSK_WIDTH = 0.03

const clamp01 = (v: number) => Math.min(1, Math.max(0, v))

type Bed = 'wind' | 'leaves' | 'water' | 'rain' | 'roof' | 'crickets'

/** One continuous layer: a looping buffer through a filter and a gain, started only while it is heard. */
class Layer {
  readonly gain: GainNode
  readonly filter: BiquadFilterNode
  private source: AudioBufferSourceNode | null = null
  private quiet = 0
  level = 0

  constructor(
    private readonly ctx: BaseAudioContext,
    out: AudioNode,
    private readonly buffer: () => AudioBuffer,
    type: BiquadFilterType,
    hz: number,
    q = 0.7,
  ) {
    this.gain = ctx.createGain()
    this.gain.gain.value = 0
    this.gain.connect(out)
    this.filter = ctx.createBiquadFilter()
    this.filter.type = type
    this.filter.frequency.value = hz
    this.filter.Q.value = q
    this.filter.connect(this.gain)
  }

  get playing() {
    return this.source !== null
  }

  /** Eases towards `level`, starting the source as it becomes heard and stopping it once it has been silent a while. */
  set(level: number, dt: number) {
    this.level = level
    const t = this.ctx.currentTime
    if (level > SILENT) {
      this.quiet = 0
      if (!this.source) {
        const source = this.ctx.createBufferSource()
        const buffer = this.buffer()
        source.buffer = buffer
        source.loop = true
        source.connect(this.filter)
        source.start(t, Math.random() * buffer.duration)
        this.source = source
      }
    } else if (this.source) {
      this.quiet += dt
      if (this.quiet > SILENT_STOP) this.stop()
    }
    this.gain.gain.setTargetAtTime(level > SILENT ? level : 0, t, EASE)
  }

  stop() {
    this.source?.stop()
    this.source = null
  }
}

/** Buffers made once per context. */
const buffers = new WeakMap<BaseAudioContext, Map<string, AudioBuffer>>()

function cached(ctx: BaseAudioContext, name: string, make: () => AudioBuffer): AudioBuffer {
  let map = buffers.get(ctx)
  if (!map) {
    map = new Map()
    buffers.set(ctx, map)
  }
  let buffer = map.get(name)
  if (!buffer) {
    buffer = make()
    map.set(name, buffer)
  }
  return buffer
}

/** Four seconds of noise, different in each ear: white, or brown (each sample a step from the last) for the wind's low roar. */
function noise(ctx: BaseAudioContext, brown: boolean): AudioBuffer {
  return cached(ctx, brown ? 'brown' : 'white', () => {
    const buffer = ctx.createBuffer(2, ctx.sampleRate * 4, ctx.sampleRate)
    for (let channel = 0; channel < 2; channel++) {
      const data = buffer.getChannelData(channel)
      let last = 0
      for (let i = 0; i < data.length; i++) {
        const white = Math.random() * 2 - 1
        if (!brown) {
          data[i] = white
          continue
        }
        last = (last + 0.02 * white) / 1.02
        data[i] = last * 3.5
      }
    }
    return buffer
  })
}

/**
 * Four seconds of crickets: in each ear a few of them, each chirping in its own time, a chirp being
 * three or four quick pulses of a high tone.
 */
function crickets(ctx: BaseAudioContext): AudioBuffer {
  return cached(ctx, 'crickets', () => {
    const rate = ctx.sampleRate
    const buffer = ctx.createBuffer(2, rate * 4, rate)
    for (let channel = 0; channel < 2; channel++) {
      const data = buffer.getChannelData(channel)
      for (let cricket = 0; cricket < 3; cricket++) {
        const pitch = 4200 + Math.random() * 900
        // Each chirp repeats so the loop's four seconds hold a whole number of them.
        const chirps = 6 + Math.floor(Math.random() * 5)
        const every = 4 / chirps
        const pulses = 3 + Math.floor(Math.random() * 2)
        const level = 0.25 + Math.random() * 0.3
        const offset = Math.random() * every
        for (let c = 0; c < chirps; c++) {
          for (let p = 0; p < pulses; p++) {
            const start = Math.floor(((offset + c * every + p * 0.032) % 4) * rate)
            const length = Math.floor(0.018 * rate)
            for (let i = 0; i < length; i++) {
              const at = (start + i) % data.length
              const envelope = Math.sin((Math.PI * i) / length)
              data[at] = data[at]! + level * envelope * Math.sin((2 * Math.PI * pitch * i) / rate)
            }
          }
        }
      }
    }
    return buffer
  })
}

/** How strong the wind is in gusts, 0 to 1, `t` seconds on: a few slow swells that never line up. */
export function gust(t: number): number {
  return clamp01(0.5 + 0.5 * (0.55 * Math.sin(t * 0.21 + 1) + 0.3 * Math.sin(t * 0.53 + 2) + 0.15 * Math.sin(t * 1.37)))
}

/** How much the birds are singing, 0 to 1, at day phase `phase`: by day, most at the dawn chorus, again in the evening. */
export function birdsong(phase: number): number {
  const day = 1 - nightAt(phase)
  const chorus = Math.exp(-(((phase - DAWN) / DAWN_WIDTH) ** 2)) + 0.5 * Math.exp(-(((phase - DUSK) / DUSK_WIDTH) ** 2))
  return clamp01(day * (0.35 + 0.65 * chorus))
}

/** What each bed should be at for `s`, with the wind at `wind` (0 to 1): the mix, without any audio. */
export function mix(s: Surround, wind: number): Record<Bed, number> {
  const night = nightAt(s.phase)
  const outdoors = s.indoors ? 0 : 1
  const summer = s.winter ? 0 : 1
  return {
    wind: WIND_LEVEL * wind * (1 - 0.4 * s.forest) * (s.indoors ? 0.35 : 1),
    leaves: LEAVES_LEVEL * wind * s.forest * (s.winter ? 0.6 : 1) * (s.indoors ? 0.2 : 1),
    water: WATER_LEVEL * s.water * (s.indoors ? 0.3 : 1),
    rain: RAIN_LEVEL * s.rain * outdoors,
    roof: ROOF_LEVEL * s.rain * (1 - outdoors),
    crickets: CRICKET_LEVEL * night * summer * (1 - s.rain) * (1 - 0.6 * s.forest) * (s.indoors ? 0.3 : 1),
  }
}

/** The wind's strength for `s` at gust `g`, 0 to 1: windier under cloud and in rain, and higher up. */
export function windOf(s: Surround, g: number): number {
  const base = 0.25 + 0.35 * s.cover + 0.4 * s.rain
  return clamp01(base * (0.55 + 0.45 * g) + Math.min(0.4, Math.max(0, s.height) / 60))
}

/** Calls a second, for the birds, gulls and owls about `s`. */
export function callRates(s: Surround): { bird: number; gull: number; owl: number } {
  const night = nightAt(s.phase)
  const dry = (1 - s.rain) ** 2
  const inside = s.indoors ? 0.5 : 1
  return {
    bird: BIRD_RATE * birdsong(s.phase) * (0.15 + 0.85 * s.forest) * dry * (s.winter ? 0.15 : 1) * inside,
    gull: GULL_RATE * s.water * (1 - s.flowing) * (1 - night) * dry * (s.winter ? 0.3 : 1) * inside,
    owl: OWL_RATE * night * s.forest * dry * inside,
  }
}

export class Ambience {
  private ctx: BaseAudioContext | null = null
  private out: GainNode | null = null
  /** Dulls what is outside while the listener is under a roof. */
  private walls: BiquadFilterNode | null = null
  private layers: Record<Bed, Layer> | null = null
  private clock = 0
  private sinceStep = STEP
  /** When the next call of each comes, the context's seconds. */
  private readonly next = { bird: 0, gull: 0, owl: 0 }
  /** Lightning slots whose thunder has been sent on its way. */
  private readonly thundered = new Set<number>()
  /** Thunder sent on its way, for tests: when it is heard, seconds of the context's clock, and from how far, metres. */
  readonly thunders: { at: number; distance: number }[] = []

  /** `provided` replaces the shared context, and `random` the chance of each call, for tests. */
  constructor(
    private readonly provided?: BaseAudioContext,
    private readonly random: () => number = Math.random,
  ) {}

  /** Each bed's level now, 0 to its loudest, for tests and for looking in from the console. */
  get levels(): Record<Bed, number> | null {
    if (!this.layers) return null
    const out = {} as Record<Bed, number>
    for (const [name, layer] of Object.entries(this.layers) as [Bed, Layer][]) out[name] = layer.level
    return out
  }

  /** How many beds are running now. */
  get running(): number {
    return this.layers ? Object.values(this.layers).filter((layer) => layer.playing).length : 0
  }

  private ready(): BaseAudioContext | null {
    if (this.ctx) return this.ctx
    const ctx = this.provided ?? audioContext()
    if (!ctx) return null
    this.walls = ctx.createBiquadFilter()
    this.walls.type = 'lowpass'
    this.walls.frequency.value = 20000
    this.walls.connect(audioBus(ctx).world)
    const out = ctx.createGain()
    out.connect(this.walls)
    this.out = out
    const white = () => noise(ctx, false)
    this.layers = {
      wind: new Layer(ctx, out, () => noise(ctx, true), 'lowpass', 400, 0.9),
      leaves: new Layer(ctx, out, white, 'bandpass', 2600, 0.6),
      water: new Layer(ctx, out, white, 'lowpass', 450, 0.8),
      rain: new Layer(ctx, out, white, 'bandpass', 5000, 0.35),
      roof: new Layer(ctx, out, white, 'lowpass', 700, 0.7),
      crickets: new Layer(ctx, out, () => crickets(ctx), 'highpass', 2500, 0.7),
    }
    this.ctx = ctx
    const now = ctx.currentTime
    this.next.bird = this.next.gull = this.next.owl = now + 2
    return ctx
  }

  /** Brings what is heard up to date with `s`, `dt` seconds after the last update. */
  update(s: Surround, dt: number) {
    const ctx = this.ready()
    const layers = this.layers
    if (!ctx || !layers || !this.out || !this.walls) return
    this.clock += dt
    this.thunder(ctx, s)
    this.sinceStep += dt
    if (this.sinceStep < STEP) return
    const step = this.sinceStep
    this.sinceStep = 0
    const t = ctx.currentTime
    const g = gust(this.clock)
    const wind = windOf(s, g)
    const levels = mix(s, wind)
    for (const name of Object.keys(levels) as Bed[]) layers[name].set(levels[name], step)
    // The wind's roar opens up in a gust; the leaves hiss brighter.
    layers.wind.filter.frequency.setTargetAtTime(220 + 520 * g * wind, t, 0.4)
    layers.leaves.filter.frequency.setTargetAtTime(1800 + 1600 * g, t, 0.4)
    // A lake laps in slow swells; a stream runs on, brighter and steady.
    const swell = 0.65 + 0.35 * Math.sin((this.clock * 2 * Math.PI) / 6.5) * Math.sin((this.clock * 2 * Math.PI) / 2.3)
    layers.water.gain.gain.setTargetAtTime(levels.water * (s.flowing + (1 - s.flowing) * swell), t, 0.3)
    layers.water.filter.frequency.setTargetAtTime(450 + 900 * s.flowing, t, 0.5)
    layers.water.filter.type = s.flowing > 0.5 ? 'bandpass' : 'lowpass'
    this.walls.frequency.setTargetAtTime(s.indoors ? 1500 : 20000, t, 0.15)
    this.out.gain.setTargetAtTime(s.indoors ? 0.7 : 1, t, 0.15)
    this.calls(ctx, s, t)
  }

  /** Starts the birds', gulls' and owls' calls that are due, and picks when each comes next. */
  private calls(ctx: BaseAudioContext, s: Surround, t: number) {
    const rates = callRates(s)
    for (const kind of ['bird', 'gull', 'owl'] as const) {
      const rate = rates[kind]
      if (rate <= 0.001) {
        this.next[kind] = t + 1
        continue
      }
      if (t < this.next[kind]) {
        // A rate that has risen (the dawn coming) is not left waiting on a call picked while it was low.
        this.next[kind] = Math.min(this.next[kind], t + 3 / rate)
        continue
      }
      this.next[kind] = t - Math.log(1 - this.random() * 0.999) / rate
      const out = this.around(ctx, s.at, kind === 'gull' ? 25 : 8)
      const at = t + 0.05 + this.random() * 0.3
      if (kind === 'bird') this.bird(ctx, out, at)
      else if (kind === 'gull') this.gull(ctx, out, at)
      else this.owl(ctx, out, at)
    }
  }

  /** A panner at a random point about the listener, `up` metres up or so, that one call is heard through. */
  private around(ctx: BaseAudioContext, at: Vec3, up: number): PannerNode {
    const angle = this.random() * Math.PI * 2
    const distance = CALL_NEAR + this.random() * (CALL_FAR - CALL_NEAR)
    const panner = ctx.createPanner()
    panner.panningModel = 'equalpower'
    panner.distanceModel = 'inverse'
    panner.refDistance = 10
    panner.rolloffFactor = 1
    const x = at[0] + Math.cos(angle) * distance
    const y = at[1] + up * (0.5 + this.random())
    const z = at[2] + Math.sin(angle) * distance
    if (panner.positionX) {
      panner.positionX.value = x
      panner.positionY.value = y
      panner.positionZ.value = z
    } else {
      panner.setPosition(x, y, z)
    }
    panner.connect(this.out!)
    return panner
  }

  /** One note: a `type` tone gliding `from` → `to` Hz over `length` s, with a quick attack and a fall. */
  private static note(ctx: BaseAudioContext, out: AudioNode, t: number, length: number, from: number, to: number, level: number, type: OscillatorType = 'sine', attack = 0.008) {
    const osc = ctx.createOscillator()
    osc.type = type
    osc.frequency.setValueAtTime(from, t)
    osc.frequency.exponentialRampToValueAtTime(Math.max(20, to), t + length)
    const gain = ctx.createGain()
    gain.gain.setValueAtTime(0.0001, t)
    gain.gain.exponentialRampToValueAtTime(level, t + attack)
    gain.gain.exponentialRampToValueAtTime(0.0001, t + length)
    osc.connect(gain).connect(out)
    osc.start(t)
    osc.stop(t + length + 0.05)
  }

  /** A songbird: a falling trill ending in a flourish, a two-note whistle again and again, or a warble. */
  private bird(ctx: BaseAudioContext, out: AudioNode, t: number) {
    const r = this.random
    const song = r()
    const level = 0.05 + r() * 0.05
    if (song < 0.4) {
      let f = 3800 + r() * 1800
      const notes = 6 + Math.floor(r() * 7)
      for (let i = 0; i < notes; i++, f *= 0.965) Ambience.note(ctx, out, t + i * 0.075, 0.055, f * 1.06, f, level)
      const end = t + notes * 0.075
      Ambience.note(ctx, out, end, 0.12, f * 1.1, f * 1.45, level * 1.2)
      Ambience.note(ctx, out, end + 0.13, 0.09, f * 1.5, f * 0.9, level)
    } else if (song < 0.75) {
      const high = 4800 + r() * 900
      const low = high * (0.66 + r() * 0.1)
      const times = 2 + Math.floor(r() * 3)
      for (let i = 0; i < times; i++) {
        const at = t + i * 0.36
        Ambience.note(ctx, out, at, 0.13, high, high * 0.97, level)
        Ambience.note(ctx, out, at + 0.17, 0.14, low * 1.02, low, level * 0.9)
      }
    } else {
      const notes = 8 + Math.floor(r() * 9)
      let at = t
      for (let i = 0; i < notes; i++) {
        const length = 0.04 + r() * 0.06
        const from = 2200 + r() * 4300
        Ambience.note(ctx, out, at, length, from, from * (0.8 + r() * 0.45), level * (0.6 + r() * 0.5))
        at += length + r() * 0.03
      }
    }
  }

  /** A gull's "kyow, kyow": a few harsh falling cries. */
  private gull(ctx: BaseAudioContext, out: AudioNode, t: number) {
    const filter = ctx.createBiquadFilter()
    filter.type = 'bandpass'
    filter.frequency.value = 1500
    filter.Q.value = 2
    filter.connect(out)
    const times = 2 + Math.floor(this.random() * 3)
    const pitch = 1200 + this.random() * 300
    for (let i = 0; i < times; i++) Ambience.note(ctx, filter, t + i * 0.42, 0.32, pitch * 1.05, pitch * 0.7, 0.12, 'sawtooth', 0.03)
  }

  /** A tawny owl: a long "hoo", a pause, a short "hu" and a long wavering "hoooo". */
  private owl(ctx: BaseAudioContext, out: AudioNode, t: number) {
    const filter = ctx.createBiquadFilter()
    filter.type = 'lowpass'
    filter.frequency.value = 900
    filter.connect(out)
    const pitch = 380 + this.random() * 60
    Ambience.note(ctx, filter, t, 0.45, pitch, pitch * 0.95, 0.16, 'triangle', 0.08)
    Ambience.note(ctx, filter, t + 1.4, 0.16, pitch * 1.05, pitch, 0.1, 'triangle', 0.03)
    const osc = ctx.createOscillator()
    osc.type = 'triangle'
    osc.frequency.value = pitch * 0.98
    const wobble = ctx.createOscillator()
    wobble.frequency.value = 9
    const depth = ctx.createGain()
    depth.gain.value = 14
    wobble.connect(depth).connect(osc.frequency)
    const gain = ctx.createGain()
    const start = t + 1.7
    gain.gain.setValueAtTime(0.0001, start)
    gain.gain.exponentialRampToValueAtTime(0.14, start + 0.12)
    gain.gain.exponentialRampToValueAtTime(0.0001, start + 1.1)
    osc.connect(gain).connect(filter)
    for (const node of [osc, wobble]) {
      node.start(start)
      node.stop(start + 1.2)
    }
  }

  /**
   * Sends the thunder of each lightning strike on its way as the flash is seen: it comes as many
   * seconds later as sound takes from the strike, which is as far off for every player.
   */
  private thunder(ctx: BaseAudioContext, s: Surround) {
    if (s.seconds === null) return
    const storm = storminess(s.rain)
    const slot = Math.floor(s.seconds / STRIKE_SLOT)
    for (const k of this.thundered) if (k < slot - 2) this.thundered.delete(k)
    if (storm <= 0) return
    for (const k of [slot - 1, slot]) {
      if (this.thundered.has(k)) continue
      const strike = strikeIn(k)
      if (strike === null || strike > s.seconds) continue
      this.thundered.add(k)
      const distance = strikeDistance(k)
      const wait = distance / 343 - (s.seconds - strike)
      if (wait < -THUNDER_LATE) continue
      const at = ctx.currentTime + Math.max(0, wait)
      this.thunders.push({ at, distance })
      if (this.thunders.length > 8) this.thunders.shift()
      this.rumble(ctx, at, distance, storm, s.at)
    }
  }

  /** A thunderclap from `distance` metres: a crack overhead when near, then a long low roll, duller and longer the farther it came. */
  private rumble(ctx: BaseAudioContext, t: number, distance: number, storm: number, listener: Vec3) {
    const out = this.around(ctx, listener, 20)
    // The thunder's loudness is its own, from `distance`: the panner only says which way it is.
    out.refDistance = CALL_FAR + 10
    const level = storm * (0.2 + 0.8 * (800 / (800 + distance))) * 0.7
    const length = 3 + distance / 1500 + this.random() * 2
    if (distance < 1500) {
      const crack = ctx.createBufferSource()
      crack.buffer = noise(ctx, false)
      const high = ctx.createBiquadFilter()
      high.type = 'highpass'
      high.frequency.value = 900
      const gain = ctx.createGain()
      gain.gain.setValueAtTime(0.0001, t)
      gain.gain.exponentialRampToValueAtTime(level * 0.8, t + 0.01)
      gain.gain.exponentialRampToValueAtTime(0.0001, t + 0.35)
      crack.connect(high).connect(gain).connect(out)
      crack.start(t, this.random())
      crack.stop(t + 0.4)
    }
    const roll = ctx.createBufferSource()
    roll.buffer = noise(ctx, true)
    roll.loop = true
    const low = ctx.createBiquadFilter()
    low.type = 'lowpass'
    low.frequency.value = Math.max(110, 2200 / (1 + distance / 700))
    const gain = ctx.createGain()
    gain.gain.setValueAtTime(0.0001, t)
    gain.gain.exponentialRampToValueAtTime(level, t + 0.1 + distance / 8000)
    // The roll comes in swells as sound arrives from farther along the bolt.
    let at = t + 0.3
    for (;;) {
      at += 0.3 + this.random() * 0.9
      if (at > t + length * 0.85) break
      gain.gain.exponentialRampToValueAtTime(Math.max(0.0001, level * (0.3 + 0.7 * this.random()) * (1 - (at - t) / (length * 1.4))), at)
    }
    gain.gain.exponentialRampToValueAtTime(0.0001, t + length)
    roll.connect(low).connect(gain).connect(out)
    roll.start(t, this.random() * 3)
    roll.stop(t + length + 0.1)
  }

  dispose() {
    if (this.layers) for (const layer of Object.values(this.layers)) layer.stop()
  }
}
