/**
 * An engine's sound, sample by sample: one pressure pulse each time a cylinder fires, rung through a
 * resonant exhaust pipe, with the intake's roar, the valves' ticking and the pops of fuel burning in the
 * pipe on the overrun. A turbine's or a motor's whine and a jet's or rocket's roar are layers of their
 * own. Nothing here touches Web Audio: the worklet (`engine.worklet.ts`) runs it on the audio thread,
 * and tests render it straight into an array.
 */

/** How an engine is built, for the sound it makes. Each kind's is in `kinds/` (`Engine.synth`). */
export interface SynthSpec {
  /** Cylinders that fire, 0 for an engine with no pistons (a turbine, a motor, a rocket). */
  cylinders: number
  /** A four-stroke fires each cylinder every other turn of the crank; a two-stroke every turn. */
  stroke: 2 | 4
  /**
   * Where in its cycle each cylinder fires, as fractions 0..1 of the cycle; evenly spaced when left
   * out. A twin-cylinder two-stroke fires at 0 and 0.5, a big-bang V8 bunches them.
   */
  firing?: readonly number[]
  /** How unevenly the cylinders burn, 0..1: a fixed difference between them, heard as a burble below the note. */
  imbalance: number
  /** How much each firing differs from the last, 0..1: roughness. */
  jitter: number
  /** How long a firing's pulse rings in the pipe before it dies away, milliseconds: short and sharp for a small engine. */
  pulse: number
  /** A diesel's hard knock on top of each firing, 0..1. */
  knock: number
  /** The exhaust pipe's length, metres: it rings at the speed of sound in hot gas over twice its length. */
  pipe: number
  /** How strongly the pipe rings, 0..0.95. */
  resonance: number
  /** Cutoff of the exhaust's muffling at idle, Hz; it opens with the revs and the throttle. */
  body: number
  /** The roar of air drawn in, in time with the firings, 0..1. */
  intake: number
  /** The valve train's ticking, 0..1. */
  mech: number
  /** Pops and crackles off the throttle at high revs, 0..1. */
  crackle: number
  /** A whine at `whineRatio` times the crank's turns a second: a turbo's, a supercharger's, a turbine's, a motor's. 0..1. */
  whine: number
  whineRatio: number
  /** Broadband roar, 0..1, low-passed at `roarHz` at full power: a turbine's exhaust, a rocket. */
  roar: number
  roarHz: number
  /** Overall loudness before the voice's own level. */
  gain: number
}

/** The speed of sound in an exhaust pipe's hot gas, m/s. */
const EXHAUST_SOUND = 480
/** Below this share of full throttle the engine is on the overrun: some firings miss, and fuel pops in the pipe. */
const OVERRUN = 0.04

/** A small, fast, seeded random source, so a render is the same every time for the same seed. */
class Noise {
  private state: number
  constructor(seed: number) {
    this.state = (seed * 2654435761) >>> 0 || 0x9e3779b9
  }
  /** -1..1. */
  next() {
    let x = this.state
    x ^= x << 13
    x ^= x >>> 17
    x ^= x << 5
    this.state = x >>> 0
    return this.state / 2147483648 - 1
  }
  /** 0..1. */
  unit() {
    return (this.next() + 1) / 2
  }
}

/** The coefficient of a one-pole low-pass at `hz` for `sampleRate`. */
const onePole = (hz: number, sampleRate: number) => 1 - Math.exp((-2 * Math.PI * Math.min(hz, sampleRate * 0.45)) / sampleRate)

/** A two-pole band-pass (a state-variable filter), for the intake's roar. */
class BandPass {
  private low = 0
  private band = 0
  run(input: number, f: number, damping: number) {
    this.low += f * this.band
    const high = input - this.low - damping * this.band
    this.band += f * high
    return this.band
  }
}

/**
 * One engine's sound generator. `render` fills a buffer with the engine at `rpm` (crank turns a
 * minute) and `load` (0 off the throttle, 1 flat out), easing from where the last buffer left them.
 */
export class EngineSynth {
  private readonly noise: Noise
  private readonly firing: number[]
  private readonly strength: number[]
  private readonly delay: Float32Array
  private delayAt = 0
  private reflected = 0
  /** Where in its cycle the crank is, 0..1, and which firing comes next. */
  private phase = 0
  private next = 0
  private pulse = 0
  private shaped = 0
  private knock = 0
  private tick = 0
  private pop = 0
  private muffled = 0
  private dcIn = 0
  private dcOut = 0
  private whinePhase = 0
  private roarLow = 0
  private lastNoise = 0
  private readonly intakeFilter = new BandPass()
  private rpm = 0
  private load = 0
  private readonly pulseDecay: number
  private readonly attack: number
  private readonly knockDecay: number
  private readonly tickDecay: number
  private readonly popDecay: number
  private readonly dcCoefficient: number

  constructor(
    private readonly spec: SynthSpec,
    /** Its highest revs, rpm: where its whine and roar are at their loudest. */
    private readonly redline: number,
    private readonly sampleRate: number,
    seed = 1,
  ) {
    this.noise = new Noise(seed)
    const cylinders = Math.max(0, Math.round(spec.cylinders))
    this.firing = (spec.firing ?? Array.from({ length: cylinders }, (_, i) => i / Math.max(1, cylinders))).slice().sort((a, b) => a - b)
    // Each cylinder burns a little stronger or weaker than the others, always the same way.
    this.strength = this.firing.map(() => 1 + spec.imbalance * 0.6 * this.noise.next())
    const samples = Math.max(2, Math.round(((2 * spec.pipe) / EXHAUST_SOUND) * sampleRate))
    this.delay = new Float32Array(samples)
    this.pulseDecay = Math.exp(-1 / ((Math.max(0.2, spec.pulse) / 1000) * sampleRate))
    this.attack = onePole(2500, sampleRate)
    this.knockDecay = Math.exp(-1 / (0.0007 * sampleRate))
    this.tickDecay = Math.exp(-1 / (0.0005 * sampleRate))
    this.popDecay = Math.exp(-1 / (0.012 * sampleRate))
    this.dcCoefficient = Math.exp((-2 * Math.PI * 25) / sampleRate)
  }

  /** Fills `out` with the engine easing from its last `rpm` and `load` to these. */
  render(out: Float32Array, rpm: number, load: number) {
    const { spec, sampleRate } = this
    const n = out.length
    const startRpm = this.rpm
    const startLoad = this.load
    const cylinders = this.firing.length
    const cycleTurns = spec.stroke === 4 ? 2 : 1
    const redlineHz = this.redline / 60
    for (let i = 0; i < n; i++) {
      const along = (i + 1) / n
      const r = Math.max(0, startRpm + (rpm - startRpm) * along)
      const l = Math.min(1, Math.max(0, startLoad + (load - startLoad) * along))
      const crankHz = r / 60
      const white = this.noise.next()
      let sample = 0

      if (cylinders > 0 && crankHz > 0) {
        // The crank turns on; each cylinder whose moment has come fires.
        this.phase += crankHz / cycleTurns / sampleRate
        for (;;) {
          if (this.next < cylinders && this.phase >= this.firing[this.next]!) {
            this.fire(this.next, l, r)
            this.next++
          } else if (this.next >= cylinders && this.phase >= 1) {
            this.phase -= 1
            this.next = 0
          } else break
        }
        // The pulse rises over a fraction of a millisecond and rings away, with the burn's roughness on it.
        this.pulse *= this.pulseDecay
        this.shaped += (this.pulse - this.shaped) * this.attack
        let excite = this.shaped * (0.85 + 0.3 * white)
        this.knock *= this.knockDecay
        excite += this.knock * (white - this.lastNoise)
        this.pop *= this.popDecay
        excite += this.pop * white
        // The pipe: what went down it comes back from the open end inverted, so it rings at its odd modes.
        const back = this.delay[this.delayAt]!
        this.reflected += (back - this.reflected) * 0.55
        const piped = excite - spec.resonance * this.reflected
        this.delay[this.delayAt] = piped
        this.delayAt = (this.delayAt + 1) % this.delay.length
        // Muffled, less so the harder it works and the faster it turns.
        const firingHz = (crankHz / cycleTurns) * cylinders
        const open = spec.body + firingHz * (1.5 + 4 * l)
        this.muffled += (piped - this.muffled) * onePole(open, sampleRate)
        // A slow high-pass takes out the steady push of the pulses, which is pressure, not sound.
        const dc = this.muffled - this.dcIn + this.dcCoefficient * this.dcOut
        this.dcIn = this.muffled
        this.dcOut = dc
        sample += dc * 2.2
        // Air drawn in roars in time with the strokes.
        const intake = this.intakeFilter.run(white, onePole(900 + firingHz * 3, sampleRate), 0.7)
        sample += intake * spec.intake * (0.15 + 0.85 * l) * (0.3 + this.shaped * 2) * 0.6
        // The valves.
        this.tick *= this.tickDecay
        sample += this.tick * (white - this.lastNoise) * 0.5
      }

      if (spec.whine > 0 && crankHz > 0) {
        this.whinePhase = (this.whinePhase + (crankHz * spec.whineRatio) / sampleRate) % 1
        const angle = 2 * Math.PI * this.whinePhase
        const share = Math.min(1, crankHz / redlineHz)
        sample += spec.whine * share * (0.4 + 0.6 * l) * (Math.sin(angle) * 0.7 + Math.sin(angle * 2.01) * 0.2 + Math.sin(angle * 3) * 0.1) * 0.4
      }

      if (spec.roar > 0 && crankHz > 0) {
        const share = Math.min(1, crankHz / redlineHz)
        this.roarLow += (white - this.roarLow) * onePole(spec.roarHz * (0.35 + 0.65 * l) * (0.5 + 0.5 * share), sampleRate)
        sample += this.roarLow * spec.roar * share * (0.3 + 0.7 * l) * 1.6
      }

      this.lastNoise = white
      // Never past full scale: soft clip.
      const x = sample * spec.gain
      out[i] = x / (1 + Math.abs(x))
    }
    this.rpm = rpm
    this.load = load
  }

  /** Cylinder `index` fires: a pulse as strong as the throttle, or on the overrun, maybe a miss or a pop. */
  private fire(index: number, load: number, rpm: number) {
    const { spec } = this
    let strength = this.strength[index]! * (1 + spec.jitter * 0.5 * this.noise.next())
    if (load < OVERRUN && rpm > this.redline * 0.35) {
      // Off the throttle high in the revs: a share of the firings miss, and some burn in the pipe.
      if (this.noise.unit() < 0.45) strength = 0
      if (spec.crackle > 0 && this.noise.unit() < spec.crackle * 0.12) this.pop = Math.min(2, this.pop + 0.8 + this.noise.unit())
    }
    const level = strength * (0.2 + 0.8 * load)
    // Many cylinders at high revs overlap their pulses; scale them so a V12 is not twelve times a single.
    const overlap = 1 / Math.sqrt(1 + ((rpm / 60) * this.firing.length * spec.pulse) / 1000 / (spec.stroke === 4 ? 2 : 1))
    this.pulse = Math.min(3, this.pulse + level * overlap)
    this.knock = Math.min(2, this.knock + level * spec.knock)
    this.tick = Math.min(1, this.tick + spec.mech * 0.5)
  }
}
