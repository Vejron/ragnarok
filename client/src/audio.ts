/**
 * The game's one audio context, the bus everything in the world is heard through, and the rooster's
 * crow. Vehicle sounds and one-shots are in `vehicleaudio.ts`, recorded samples in `sfx.ts`, the
 * wind, weather and wildlife in `ambience.ts`.
 */

let context: AudioContext | null = null

/**
 * Where the world's sounds go. `world` is everything heard through the air outside, dulled by `muffle`
 * in a closed cabin or under water; `dry` is what the listener hears in their own head or seat (their
 * own engine, their hits and hurts), never dulled. Both meet in one compressor before the speakers.
 */
export interface AudioBus {
  world: GainNode
  dry: GainNode
  muffle: BiquadFilterNode
  /** How loud the world is after the muffle: turned down with it. */
  through: GainNode
}

/** What stands between the listener and the world: nothing, a vehicle's closed cabin, or water. */
export type Muffle = 'none' | 'cabin' | 'water'

/** The low-pass, Hz, and the level left, of each muffle. */
const MUFFLES: Record<Muffle, { hz: number; level: number }> = {
  none: { hz: 20000, level: 1 },
  cabin: { hz: 1400, level: 0.7 },
  water: { hz: 380, level: 0.4 },
}
/** Seconds a change of muffle takes to settle, as a time constant. */
const MUFFLE_EASE = 0.12

const buses = new WeakMap<BaseAudioContext, AudioBus>()

/** The bus of `ctx`, made the first time it is asked for. */
export function audioBus(ctx: BaseAudioContext): AudioBus {
  let bus = buses.get(ctx)
  if (!bus) {
    const compressor = ctx.createDynamicsCompressor()
    compressor.connect(ctx.destination)
    const through = ctx.createGain()
    through.connect(compressor)
    const muffle = ctx.createBiquadFilter()
    muffle.type = 'lowpass'
    muffle.frequency.value = MUFFLES.none.hz
    muffle.connect(through)
    const world = ctx.createGain()
    world.connect(muffle)
    const dry = ctx.createGain()
    dry.connect(compressor)
    bus = { world, dry, muffle, through }
    buses.set(ctx, bus)
  }
  return bus
}

let muffled: Muffle = 'none'

/** Dulls the world for a listener in a closed cabin or under water, easing into it. */
export function setMuffle(muffle: Muffle, provided?: BaseAudioContext) {
  if (muffle === muffled && !provided) return
  const ctx = provided ?? context
  if (!ctx) return
  muffled = muffle
  const bus = audioBus(ctx)
  const { hz, level } = MUFFLES[muffle]
  bus.muffle.frequency.setTargetAtTime(hz, ctx.currentTime, MUFFLE_EASE)
  bus.through.gain.setTargetAtTime(level, ctx.currentTime, MUFFLE_EASE)
}

/** The shared context, or null where the browser has none (or has not allowed one yet). */
export function audioContext(): AudioContext | null {
  try {
    context ??= new AudioContext()
    if (context.state === 'suspended') void context.resume()
    return context
  } catch {
    return null
  }
}

/** One voiced note: a sawtooth sliding `from` → `to` Hz, through a low-pass, with a short attack and a decaying tail. */
function note(ctx: AudioContext, at: number, length: number, from: number, to: number, volume: number, wobble = 0) {
  const osc = ctx.createOscillator()
  osc.type = 'sawtooth'
  osc.frequency.setValueAtTime(from, at)
  osc.frequency.exponentialRampToValueAtTime(to, at + length * 0.7)
  osc.frequency.exponentialRampToValueAtTime(from * 0.85, at + length)
  if (wobble > 0) {
    const lfo = ctx.createOscillator()
    const depth = ctx.createGain()
    lfo.frequency.value = 22
    depth.gain.value = wobble
    lfo.connect(depth).connect(osc.frequency)
    lfo.start(at)
    lfo.stop(at + length + 0.05)
  }
  const filter = ctx.createBiquadFilter()
  filter.type = 'lowpass'
  filter.frequency.value = 2200
  const gain = ctx.createGain()
  gain.gain.setValueAtTime(0.0001, at)
  gain.gain.exponentialRampToValueAtTime(volume, at + 0.03)
  gain.gain.exponentialRampToValueAtTime(0.0001, at + length)
  osc.connect(filter).connect(gain).connect(audioBus(ctx).world)
  osc.start(at)
  osc.stop(at + length + 0.05)
}

/** A rooster's "ko-ko-koo-koooo!", over about as long as the crow animation. `volume` is 0..1. */
export function playCrow(volume: number) {
  if (volume < 0.02) return
  const ctx = audioContext()
  if (!ctx) return
  const t = ctx.currentTime + 0.02
  const v = 0.25 * Math.min(1, volume)
  note(ctx, t + 0.25, 0.14, 520, 720, v)
  note(ctx, t + 0.47, 0.14, 560, 780, v)
  note(ctx, t + 0.7, 0.85, 600, 1050, v * 1.2, 40)
}
