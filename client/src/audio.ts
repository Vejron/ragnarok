/** Sounds the game makes, synthesised: there are no audio files. */

let context: AudioContext | null = null

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
  osc.connect(filter).connect(gain).connect(ctx.destination)
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
