import { audioBus, audioContext } from './audio'
import { SampleBank } from './sfx'
import { kindView } from '@/kinds'
import type { KindSound } from '@/kinds/kind'
import { crashSize, landingThud, mixOf, Revs, type SoundInput, type SoundMix } from '@/sim/engine'
import type { Footing } from '@/sim/groundsound'
import { PROJECTILE_AMV_BOMB, PROJECTILE_BALL, PROJECTILE_BOMB, PROJECTILE_CANNON, PROJECTILE_CANNON_AP, PROJECTILE_HE, PROJECTILE_MORTAR, PROJECTILE_ROCKET, PROJECTILE_ROTARY, PROJECTILE_SHELL } from '@/net/codec'
import type { EngineOptions } from '@/sound/engine.worklet'
import { workletReady } from '@/sound/worklet'

/**
 * Vehicle sound: each engine made firing by firing on the audio thread (`sound/enginesynth.ts`), the
 * crunch of the ground under it (`sound/gritsynth.ts`), noise run through filters for tyres, tracks,
 * blades, scrapes and fire, and one-shots for crashes, landings, footsteps, explosions and gunfire
 * (recorded where `sfx.ts` has a sample). `sim/engine.ts` says how fast the engine turns and how loud
 * and how high each layer is; this file only turns that into nodes.
 */

export type Vec3 = readonly [number, number, number]

/** Engines further than this are not heard, metres, and no more than this many play at once. */
const HEARING = 160
const MAX_VOICES = 6
/** One-shots still sounding at once, at most. */
const MAX_SHOTS = 14
/** Footsteps further off than this are not heard, metres, and they never take the last few slots. */
const STEP_HEARING = 40
const STEP_SPARE = 5
/** A splash this big or bigger is a heavy one (any vehicle going in, a section of bridge), and one under `SPLASH_SMALL` gives way like a step. */
const SPLASH_BIG = 0.8
const SPLASH_SMALL = 0.35
/** Rounds striking something further than this are not heard, metres. */
const IMPACT_HEARING = 60
/** A piece landing this big or bigger, metres, crashes; big landings within `COLLAPSE_CELL` metres and `COLLAPSE_EVERY` ms of each other are one. */
const DEBRIS_BIG = 2
const COLLAPSE_CELL = 12
const COLLAPSE_EVERY = 600
/** Projectiles in flight further than this are not heard, metres, and no more than this many at once. */
const FLIGHT_HEARING = 200
const MAX_FLIGHTS = 4
const SPEED_OF_SOUND = 343
/**
 * How far the loud one-shots carry, metres: rifles and machine guns, a small blast, and a cannon or a
 * vehicle blowing up. Beyond `HEARING` they arrive late, dull and mostly echo, which tells a player
 * where a fight is from a kilometre off.
 */
const GUN_REACH = 800
const BLAST_REACH = 1100
const BOOM_REACH = 1600
/** Slots a shot from beyond `HEARING` leaves free for nearer sounds. */
const FAR_SPARE = 6
/**
 * The echo off the land: how long it rings, seconds, and how loud it is against the shot close by and
 * far off, as a ratio of their energies' square roots (the impulse holds unit energy in each ear).
 */
const ECHO_LENGTH = 2.2
const ECHO_NEAR = 0.3
const ECHO_FAR = 0.9
/**
 * The reflections that come back from open ground: seconds after the shot, how strong against the first,
 * and how dull, the low-pass Hz. Tree lines and houses near, a hillside and the far woods later.
 */
const ECHO_SLAPS: readonly { at: number; level: number; hz: number }[] = [
  { at: 0.13, level: 1, hz: 2200 },
  { at: 0.31, level: 0.6, hz: 1500 },
  { at: 0.58, level: 0.45, hz: 950 },
  { at: 1.02, level: 0.3, hz: 600 },
]
/** How loud the low roll after the slaps is, against the first slap. */
const ECHO_ROLL = 0.22
/** Seconds a voice takes to fade out before its nodes stop. */
const FADE = 0.25
/** What is left of a plane's engine with the blip switch in, and how often it fires through it, s. */
const CUT_LEVEL = 0.05
const BLIP_EVERY = 0.2

const noises = new WeakMap<BaseAudioContext, AudioBuffer>()

/** Two seconds of white noise, made once per context and looped by everything that needs hiss. */
function noiseBuffer(ctx: BaseAudioContext): AudioBuffer {
  let buffer = noises.get(ctx)
  if (!buffer) {
    buffer = ctx.createBuffer(1, ctx.sampleRate * 2, ctx.sampleRate)
    const data = buffer.getChannelData(0)
    for (let i = 0; i < data.length; i++) data[i] = Math.random() * 2 - 1
    noises.set(ctx, buffer)
  }
  return buffer
}

function place(node: PannerNode, at: Vec3) {
  if (node.positionX) {
    node.positionX.value = at[0]
    node.positionY.value = at[1]
    node.positionZ.value = at[2]
  } else {
    node.setPosition(at[0], at[1], at[2])
  }
}

/** The highs air leaves of a sound from `distance` metres off: the cut-off of its low-pass, Hz. */
export function airCut(distance: number): number {
  return Math.max(350, 18000 / (1 + distance / 50))
}

/** How much of a loud shot from `distance` metres off is heard as its echo, against 1 for the shot itself up close. */
export function echoLevel(distance: number): number {
  const share = ECHO_NEAR + (ECHO_FAR - ECHO_NEAR) * (distance / (distance + 250))
  return share * Math.pow(12 / Math.max(12, distance), 0.6)
}

/** The pole of a one-pole low-pass at `hz`, and the gain that keeps white noise through it as loud. */
function onePole(hz: number, rate: number): { pole: number; gain: number } {
  const pole = Math.exp((-2 * Math.PI * hz) / rate)
  return { pole, gain: Math.sqrt((1 + pole) / (1 - pole)) }
}

/**
 * The echo's impulse, as open ground gives it back: silence while the sound travels out, then a few
 * separate dull slaps off tree lines, houses and hills, each later one duller, and under them a low
 * roll that swells and dies. Each ear hears the slaps at slightly different times. Nothing comes back
 * in the first tenth of a second and there is no dense ring, which is what made it sound like a room.
 * Scaled to unit energy in each ear, so the send's gain is how loud the echo is against the shot.
 */
export function echoImpulse(ctx: BaseAudioContext): AudioBuffer {
  const rate = ctx.sampleRate
  const length = Math.floor(rate * ECHO_LENGTH)
  const buffer = ctx.createBuffer(2, length, rate)
  for (let channel = 0; channel < 2; channel++) {
    const data = buffer.getChannelData(channel)
    for (const slap of ECHO_SLAPS) {
      const start = Math.floor((slap.at + (Math.random() - 0.5) * 0.04 * (1 + slap.at)) * rate)
      const { pole, gain } = onePole(slap.hz, rate)
      // Foliage and uneven ground smear a reflection: a quick rise and a fall of some tens of ms.
      const fall = 0.018 + 0.03 * slap.at
      let smooth = 0
      for (let i = 0; i < Math.floor(fall * 5 * rate) && start + i < length; i++) {
        const t = i / rate
        smooth = smooth * pole + (Math.random() * 2 - 1) * (1 - pole)
        const envelope = Math.min(1, t / 0.002) * Math.exp(-t / fall)
        data[start + i] = data[start + i]! + slap.level * gain * smooth * envelope
      }
    }
    // The roll: sound arriving from everywhere farther off, darker as it goes on, in uneven swells.
    const begin = 0.1
    const swell = Math.random() * Math.PI * 2
    let smooth = 0
    for (let i = Math.floor(begin * rate); i < length; i++) {
      const t = i / rate - begin
      const { pole, gain } = onePole(520 / (1 + 1.8 * t), rate)
      smooth = smooth * pole + (Math.random() * 2 - 1) * (1 - pole)
      const envelope = (1 - Math.exp(-t / 0.15)) * Math.exp(-t * 2.4) * (0.65 + 0.35 * Math.sin(t * 9.5 + swell) * Math.sin(t * 3.7))
      const fadeOut = Math.min(1, (ECHO_LENGTH - begin - t) / 0.2)
      data[i] = data[i]! + ECHO_ROLL * gain * smooth * envelope * fadeOut
    }
    let energy = 0
    for (let i = 0; i < length; i++) energy += data[i]! * data[i]!
    const scale = energy > 0 ? 1 / Math.sqrt(energy) : 0
    for (let i = 0; i < length; i++) data[i] = data[i]! * scale
  }
  return buffer
}

/** A loud sound's panner: it falls away more slowly than `makePanner`'s, so a shot is still heard a kilometre off. */
function makeLoudPanner(ctx: BaseAudioContext): PannerNode {
  const panner = ctx.createPanner()
  panner.panningModel = 'equalpower'
  panner.distanceModel = 'inverse'
  panner.refDistance = 12
  panner.rolloffFactor = 1
  panner.maxDistance = 10000
  return panner
}

function makePanner(ctx: BaseAudioContext): PannerNode {
  const panner = ctx.createPanner()
  panner.panningModel = 'equalpower'
  panner.distanceModel = 'inverse'
  panner.refDistance = 8
  panner.rolloffFactor = 1.6
  panner.maxDistance = 250
  return panner
}

/** One vehicle's continuous sound: engine, tyres, ground, damage and fire, all through one panner. */
class Voice {
  readonly panner: PannerNode
  private readonly sources: AudioScheduledSourceNode[] = []
  private readonly fade: GainNode
  /** The engine (`EngineSynth`) and the crunch under it (`GritSynth`), once the worklet has loaded. */
  private engineNode: AudioWorkletNode | null = null
  private gritNode: AudioWorkletNode | null = null
  private readonly engine: GainNode
  private readonly stutter: GainNode
  private readonly beat: OscillatorNode
  private readonly beatLevel: GainNode
  private readonly chopFilter: BiquadFilterNode
  private readonly squeal: { filter: BiquadFilterNode; gain: GainNode }
  private readonly ground: { filter: BiquadFilterNode; gain: GainNode }
  private readonly hiss: GainNode
  private readonly grind: GainNode
  private readonly fire: GainNode
  private stopped = false
  /** A plane's ignition is cut, and when it next blips. */
  private cut = false
  private nextBlip = 0

  constructor(
    private readonly ctx: BaseAudioContext,
    private readonly sound: KindSound,
    master: AudioNode,
    /** Seeds its engine's unevenness, so two of a kind side by side do not sound the same. */
    private readonly seed: number,
    /** It is the listener's own, heard in their seat. */
    readonly own = false,
  ) {
    this.fade = ctx.createGain()
    this.fade.connect(master)
    this.panner = makePanner(ctx)
    this.panner.connect(this.fade)

    // Engine: the worklet's engine → level → dropout gain → panner.
    this.stutter = ctx.createGain()
    this.stutter.connect(this.panner)
    this.engine = ctx.createGain()
    this.engine.gain.value = 0
    this.engine.connect(this.stutter)

    // Noise: one looping source split into the layers that use it.
    const noise = ctx.createBufferSource()
    noise.buffer = noiseBuffer(ctx)
    noise.loop = true
    this.sources.push(noise)
    const filtered = (type: BiquadFilterType, hz: number, q: number, to: AudioNode) => {
      const filter = ctx.createBiquadFilter()
      filter.type = type
      filter.frequency.value = hz
      filter.Q.value = q
      const gain = ctx.createGain()
      gain.gain.value = 0
      noise.connect(filter)
      filter.connect(gain).connect(to)
      return { filter, gain }
    }
    this.squeal = filtered('bandpass', 1200, 9, this.panner)
    this.ground = filtered('lowpass', 600, 0.7, this.panner)
    this.hiss = filtered('highpass', 3200, 0.7, this.panner).gain
    this.fire = filtered('highpass', 1800, 0.7, this.panner).gain
    // Blades and tracks: noise chopped by a low-frequency oscillator.
    const chopped = ctx.createGain()
    chopped.gain.value = 0
    this.chopFilter = ctx.createBiquadFilter()
    this.chopFilter.type = sound.chop.type
    this.chopFilter.frequency.value = sound.chop.hz
    noise.connect(this.chopFilter).connect(chopped).connect(this.panner)
    this.beat = ctx.createOscillator()
    this.beat.frequency.value = 10
    this.beatLevel = ctx.createGain()
    this.beatLevel.gain.value = 0
    this.beat.connect(this.beatLevel).connect(chopped.gain)
    this.sources.push(this.beat)
    // The grind of a scrape or a bad wheel: a rough low tone with some noise on it.
    const grind = ctx.createOscillator()
    grind.type = 'sawtooth'
    grind.frequency.value = 62
    this.grind = ctx.createGain()
    this.grind.gain.value = 0
    const grindTone = ctx.createBiquadFilter()
    grindTone.type = 'bandpass'
    grindTone.frequency.value = 900
    grindTone.Q.value = 2
    grind.connect(grindTone).connect(this.grind).connect(this.panner)
    this.sources.push(grind)
    noise.connect(grindTone)

    const start = ctx.currentTime
    for (const source of this.sources) source.start(start)
  }

  /** Makes the engine and grit nodes once their worklet has loaded. */
  private worklets() {
    if (this.gritNode || !workletReady(this.ctx)) return
    const { engine } = this.sound
    if (engine.name !== 'none') {
      const options: EngineOptions = { spec: engine.synth, redline: engine.redline, seed: this.seed }
      this.engineNode = new AudioWorkletNode(this.ctx, 'engine', { numberOfInputs: 0, numberOfOutputs: 1, outputChannelCount: [1], processorOptions: options })
      this.engineNode.connect(this.engine)
    }
    this.gritNode = new AudioWorkletNode(this.ctx, 'grit', { numberOfInputs: 0, numberOfOutputs: 1, outputChannelCount: [1], processorOptions: { seed: this.seed + 1 } })
    this.gritNode.connect(this.panner)
  }

  /** Moves the sound to `at` (the listener's own place for a vehicle the listener is in). */
  moveTo(at: Vec3) {
    place(this.panner, at)
  }

  apply(mix: SoundMix, doppler: number, scraping: boolean, dt: number) {
    if (this.stopped) return
    this.worklets()
    const t = this.ctx.currentTime
    if (this.engineNode) {
      this.engineNode.parameters.get('rpm')!.setTargetAtTime(mix.rpm * doppler, t, 0.03)
      this.engineNode.parameters.get('load')!.setTargetAtTime(mix.load, t, 0.04)
    }
    if (this.gritNode) {
      const { grit } = mix
      const param = (name: string) => this.gritNode!.parameters.get(name)!
      param('rate').setTargetAtTime(grit.rate, t, 0.08)
      param('level').setTargetAtTime(grit.level, t, 0.08)
      param('hz').setTargetAtTime(grit.hz, t, 0.08)
      param('ring').setTargetAtTime(grit.ring, t, 0.08)
      param('length').setTargetAtTime(grit.length, t, 0.08)
    }
    this.engine.gain.setTargetAtTime(mix.engine, t, 0.08)
    this.beat.frequency.setTargetAtTime(Math.max(1, mix.chop * doppler), t, 0.1)
    this.beatLevel.gain.setTargetAtTime(mix.chopLevel * 0.5, t, 0.1)
    this.chopFilter.frequency.setTargetAtTime(this.sound.chop.hz * mix.chopTone, t, 0.2)
    this.squeal.gain.gain.setTargetAtTime(mix.squeal * 0.35, t, 0.04)
    this.squeal.filter.frequency.setTargetAtTime(mix.squealHz, t, 0.05)
    // Across the edge of one ground and the next, the roll crossfades rather than jumps.
    this.ground.gain.gain.setTargetAtTime(mix.ground * 0.25, t, 0.1)
    this.ground.filter.frequency.setTargetAtTime(mix.groundHz, t, 0.1)
    this.hiss.gain.setTargetAtTime(mix.hiss, t, 0.2)
    this.grind.gain.setTargetAtTime(Math.max(mix.rattle * 0.25, scraping ? 0.35 : 0), t, 0.03)
    if (mix.cut !== this.cut) {
      // The blip switch: the engine cuts out at once and catches again as quickly.
      this.cut = mix.cut
      this.stutter.gain.cancelScheduledValues(t)
      this.stutter.gain.setTargetAtTime(mix.cut ? CUT_LEVEL : 1, t, mix.cut ? 0.01 : 0.03)
      this.nextBlip = t + BLIP_EVERY
    } else if (mix.cut && t >= this.nextBlip) {
      // Held in, it still fires a cylinder or two now and then: the rotary's "brrp".
      this.stutter.gain.cancelScheduledValues(t)
      this.stutter.gain.setValueAtTime(0.75, t)
      this.stutter.gain.setTargetAtTime(CUT_LEVEL, t + 0.05, 0.012)
      this.nextBlip = t + BLIP_EVERY + Math.random() * 0.12
    } else if (!mix.cut && mix.stutter > 0 && Math.random() < mix.stutter * dt) {
      this.stutter.gain.cancelScheduledValues(t)
      this.stutter.gain.setValueAtTime(0.1, t)
      this.stutter.gain.linearRampToValueAtTime(1, t + 0.15)
    }
    if (mix.fire) {
      // A rumble with crackles on it.
      if (Math.random() < dt * 26) {
        this.fire.gain.cancelScheduledValues(t)
        this.fire.gain.setValueAtTime(0.1 + Math.random() * 0.25, t)
        this.fire.gain.setTargetAtTime(0.03, t, 0.025)
      }
    } else {
      this.fire.gain.setTargetAtTime(0, t, 0.1)
    }
  }

  /** Fades out and stops every node. */
  stop() {
    if (this.stopped) return
    this.stopped = true
    const t = this.ctx.currentTime
    this.fade.gain.setTargetAtTime(0, t, FADE / 5)
    for (const source of this.sources) source.stop(t + FADE)
    // The worklets end themselves once told to, after the fade.
    const end = [this.engineNode, this.gritNode]
    setTimeout(() => end.forEach((node) => node?.port.postMessage('stop')), FADE * 1000)
  }
}

/** How a projectile is heard in flight: a rocket's motor roars, a shell whistles past, a dropped bomb's whistle falls with it. */
type FlightSound = 'motor' | 'whistle' | 'bomb'

/** How a `PROJECTILE_*` is heard in flight, or null for one that is silent (a grenade, a golf ball). */
function flightOf(kind: number): FlightSound | null {
  switch (kind) {
    case PROJECTILE_ROCKET: return 'motor'
    case PROJECTILE_BOMB: return 'bomb'
    case PROJECTILE_SHELL:
    case PROJECTILE_CANNON:
    case PROJECTILE_CANNON_AP:
    case PROJECTILE_HE:
    case PROJECTILE_ROTARY:
    case PROJECTILE_MORTAR:
    case PROJECTILE_AMV_BOMB:
    case PROJECTILE_BALL:
      return 'whistle'
    default:
      return null
  }
}

/** One projectile's sound in flight: noise through a band-pass, louder and higher as it closes on the listener. */
class FlightVoice {
  private readonly source: AudioBufferSourceNode
  private readonly filter: BiquadFilterNode
  private readonly gain: GainNode
  private readonly panner: PannerNode
  private stopped = false

  constructor(
    ctx: BaseAudioContext,
    master: AudioNode,
    private readonly sound: FlightSound,
  ) {
    this.source = ctx.createBufferSource()
    this.source.buffer = noiseBuffer(ctx)
    this.source.loop = true
    this.filter = ctx.createBiquadFilter()
    this.filter.type = 'bandpass'
    this.filter.Q.value = sound === 'motor' ? 0.8 : 6
    this.gain = ctx.createGain()
    this.gain.gain.value = 0
    this.panner = makePanner(ctx)
    this.source.connect(this.filter).connect(this.gain).connect(this.panner).connect(master)
    this.source.start(ctx.currentTime, Math.random())
  }

  /** Moves it to `at`, `distance` m off and `closing` m/s towards the listener, `above` metres over them. */
  apply(t: number, at: Vec3, distance: number, closing: number, above: number) {
    if (this.stopped) return
    place(this.panner, at)
    const doppler = Math.min(1.6, Math.max(0.6, SPEED_OF_SOUND / (SPEED_OF_SOUND - Math.min(closing, 300))))
    const near = Math.max(0, 1 - distance / FLIGHT_HEARING)
    let hz: number
    let level: number
    if (this.sound === 'motor') {
      hz = 1400 * doppler
      level = 0.45 * near * (0.85 + 0.15 * Math.random())
    } else if (this.sound === 'bomb') {
      // The whistle falls as the bomb comes down.
      hz = (500 + Math.min(Math.max(above, 0), 120) * 14) * doppler
      level = 0.3 * near
    } else {
      // Heard mostly as it comes, little once past.
      hz = 1100 * doppler
      level = 0.28 * near * near * (closing > 0 ? 1 : 0.4)
    }
    this.filter.frequency.setTargetAtTime(hz, t, 0.03)
    this.gain.gain.setTargetAtTime(level, t, 0.05)
  }

  stop(t: number) {
    if (this.stopped) return
    this.stopped = true
    this.gain.gain.setTargetAtTime(0, t, 0.04)
    this.source.stop(t + 0.3)
  }
}

interface Candidate {
  id: number
  kind: number
  distance: number
  own: boolean
  input: SoundInput
  position: Vec3
  doppler: number
}

/**
 * All the vehicle sounds of a game: a voice per vehicle that is near and running (the nearest few),
 * plus one-shots. Every call is harmless without a Web Audio context.
 */
export class VehicleSounds {
  private ctx: BaseAudioContext | null = null
  /** The world's sounds, dulled with the bus in a cabin or under water, and the listener's own (`dry`), never dulled. */
  private master: GainNode | null = null
  private dry: GainNode | null = null
  /** The echo off the land that loud one-shots send to. */
  private echo: ConvolverNode | null = null
  /** The hiss of skis on snow, once somebody here has skied. */
  private skiHiss: { filter: BiquadFilterNode; gain: GainNode } | null = null
  private bank: SampleBank | null = null
  private readonly voices = new Map<number, Voice>()
  /** The revs of each engine in earshot. */
  private readonly revs = new Map<number, Revs>()
  /** Projectiles in flight reported this frame, and the voices of the nearest few. */
  private readonly flying: { id: number; sound: FlightSound; distance: number; at: Vec3; velocity: Vec3 }[] = []
  private readonly flights = new Map<number, FlightVoice>()
  /** Until when a big landing near each spot (`COLLAPSE_CELL`) is merged into the last one, ms. */
  private readonly collapses = new Map<string, number>()
  private readonly candidates: Candidate[] = []
  private readonly scrapes = new Map<number, number>()
  private readonly shots: number[] = []
  private listener: Vec3 = [0, 0, 0]
  private listenerVelocity: Vec3 = [0, 0, 0]

  /** `context` replaces the shared one, for tests. */
  constructor(private readonly provided?: BaseAudioContext) {}

  /** Number of projectiles currently heard in flight. */
  get flightCount() {
    return this.flights.size
  }

  /** Number of vehicles currently sounding. */
  get voiceCount() {
    return this.voices.size
  }

  private ready(): BaseAudioContext | null {
    if (this.ctx) return this.ctx
    const ctx = this.provided ?? audioContext()
    if (!ctx) return null
    const bus = audioBus(ctx)
    this.master = ctx.createGain()
    this.master.gain.value = 0.7
    this.master.connect(bus.world)
    this.dry = ctx.createGain()
    this.dry.gain.value = 0.7
    this.dry.connect(bus.dry)
    this.echo = ctx.createConvolver()
    this.echo.normalize = false
    this.echo.buffer = echoImpulse(ctx)
    this.echo.connect(this.master)
    this.ctx = ctx
    this.bank = new SampleBank(ctx)
    this.bank.load()
    this.setListener(this.listener, [0, 0, -1], this.listenerVelocity)
    return ctx
  }

  /** Where the camera is and which way it looks, and how fast the listener moves (for Doppler). */
  setListener(position: Vec3, forward: Vec3, velocity: Vec3 = [0, 0, 0]) {
    this.listener = position
    this.listenerVelocity = velocity
    // The listener is shared with voice chat, so it must move even before any vehicle has sounded.
    const l = (this.ctx ?? this.provided ?? audioContext())?.listener
    if (!l) return
    if (l.positionX) {
      l.positionX.value = position[0]
      l.positionY.value = position[1]
      l.positionZ.value = position[2]
      l.forwardX.value = forward[0]
      l.forwardY.value = forward[1]
      l.forwardZ.value = forward[2]
      l.upX.value = 0
      l.upY.value = 1
      l.upZ.value = 0
    } else {
      l.setPosition(position[0], position[1], position[2])
      l.setOrientation(forward[0], forward[1], forward[2], 0, 1, 0)
    }
  }

  begin() {
    this.candidates.length = 0
  }

  /** Metres from the listener. */
  private distanceTo(at: Vec3) {
    return Math.hypot(at[0] - this.listener[0], at[1] - this.listener[1], at[2] - this.listener[2])
  }

  /**
   * Reports vehicle `id` for this frame. `own` is the one the listener sits in: heard without
   * distance or Doppler. Vehicles that are neither running nor wrecked, nor scraping, make no sound.
   */
  engine(id: number, input: SoundInput, position: Vec3, velocity: Vec3, own: boolean, now = performance.now()) {
    const scraping = (this.scrapes.get(id) ?? 0) > now
    if (!input.powered && input.hp > 0 && !scraping) return
    const distance = own ? 0 : this.distanceTo(position)
    if (distance > HEARING) return
    let doppler = 1
    if (!own && distance > 1) {
      const away = [(this.listener[0] - position[0]) / distance, (this.listener[1] - position[1]) / distance, (this.listener[2] - position[2]) / distance] as const
      const closing =
        (velocity[0] - this.listenerVelocity[0]) * away[0] + (velocity[1] - this.listenerVelocity[1]) * away[1] + (velocity[2] - this.listenerVelocity[2]) * away[2]
      doppler = Math.min(1.25, Math.max(0.8, SPEED_OF_SOUND / (SPEED_OF_SOUND - Math.min(closing, 250))))
    }
    this.candidates.push({ id, kind: input.kind, distance, own, input, position, doppler })
  }

  /** Starts, updates and stops voices for what was reported since `begin`. */
  end(dt: number, now = performance.now()) {
    if (this.candidates.length === 0 && this.voices.size === 0) return
    const ctx = this.ready()
    const master = this.master
    const dry = this.dry
    if (!ctx || !master || !dry) return
    this.candidates.sort((a, b) => Number(b.own) - Number(a.own) || a.distance - b.distance)
    // Every engine in earshot keeps its revs turning over, so one coming into the nearest few is heard where it is.
    const heard = new Set<number>()
    for (const candidate of this.candidates) heard.add(candidate.id)
    for (const id of this.revs.keys()) if (!heard.has(id)) this.revs.delete(id)
    const chosen = new Set<number>()
    this.candidates.forEach((candidate, index) => {
      const { engine } = kindView(candidate.kind).sound
      let revs = this.revs.get(candidate.id)
      if (!revs) {
        revs = new Revs()
        this.revs.set(candidate.id, revs)
      }
      const state = engine.name === 'none' ? { rpm: 0, load: 0, gear: 0 } : revs.step(engine, candidate.input, dt)
      if (index >= MAX_VOICES) return
      chosen.add(candidate.id)
      let voice = this.voices.get(candidate.id)
      // The listener's own vehicle is heard in their seat, past the cabin's muffle; one they get into or out of is made again.
      if (voice && voice.own !== candidate.own) {
        voice.stop()
        voice = undefined
      }
      if (!voice) {
        voice = new Voice(ctx, kindView(candidate.kind).sound, candidate.own ? dry : master, candidate.id, candidate.own)
        this.voices.set(candidate.id, voice)
      }
      voice.moveTo(candidate.own ? this.listener : candidate.position)
      voice.apply(mixOf(candidate.input, engine, state), candidate.doppler, (this.scrapes.get(candidate.id) ?? 0) > now, dt)
    })
    for (const [id, voice] of this.voices) {
      if (chosen.has(id)) continue
      voice.stop()
      this.voices.delete(id)
    }
  }

  /** Vehicle `id` is gone. */
  drop(id: number) {
    this.voices.get(id)?.stop()
    this.voices.delete(id)
    this.revs.delete(id)
    this.scrapes.delete(id)
  }

  /** Vehicle `id` is scraping something: a grind plays for the next fraction of a second. */
  scrape(id: number, now = performance.now()) {
    this.scrapes.set(id, now + 220)
  }

  // One-shots.

  /**
   * Plays a one-shot at `at` (null: in the listener's head), if a slot is free; `spare` slots are kept
   * back from it, for the sounds that matter more. It arrives late at the speed of sound, and dulled:
   * air takes the highs first. A loud one (`reach` past `HEARING`) falls away more slowly, carries to
   * `reach` metres and is heard partly as its echo off the land, more so the farther it is.
   */
  private emit(at: Vec3 | null, life: number, build: (ctx: BaseAudioContext, out: AudioNode, t: number) => void, spare = 0, reach = HEARING) {
    const ctx = this.ready()
    if (!ctx || !this.master || !this.dry) return
    const now = ctx.currentTime
    for (let i = this.shots.length - 1; i >= 0; i--) if (this.shots[i]! < now) this.shots.splice(i, 1)
    if (!at) {
      if (this.shots.length >= MAX_SHOTS - spare) return
      this.shots.push(now + life)
      build(ctx, this.dry, now + 0.01)
      return
    }
    const distance = this.distanceTo(at)
    if (distance > reach) return
    if (this.shots.length >= MAX_SHOTS - Math.max(spare, distance > HEARING ? FAR_SPARE : 0)) return
    const loud = reach > HEARING
    const panner = loud ? makeLoudPanner(ctx) : makePanner(ctx)
    place(panner, at)
    panner.connect(this.master)
    const air = ctx.createBiquadFilter()
    air.type = 'lowpass'
    air.frequency.value = airCut(distance)
    air.connect(panner)
    if (loud && this.echo) {
      const send = ctx.createGain()
      send.gain.value = echoLevel(distance)
      air.connect(send).connect(this.echo)
    }
    const late = distance / SPEED_OF_SOUND
    this.shots.push(now + late + life)
    build(ctx, air, now + 0.01 + late)
  }

  /** A burst of filtered noise that sweeps from `from` to `to` Hz and dies away over `length` s. */
  private static burst(ctx: BaseAudioContext, out: AudioNode, t: number, type: BiquadFilterType, from: number, to: number, length: number, level: number, delay = 0) {
    const source = ctx.createBufferSource()
    source.buffer = noiseBuffer(ctx)
    const filter = ctx.createBiquadFilter()
    filter.type = type
    filter.frequency.setValueAtTime(from, t + delay)
    filter.frequency.exponentialRampToValueAtTime(Math.max(20, to), t + delay + length)
    const gain = ctx.createGain()
    gain.gain.setValueAtTime(0.0001, t + delay)
    gain.gain.linearRampToValueAtTime(level, t + delay + 0.004)
    gain.gain.exponentialRampToValueAtTime(0.0001, t + delay + length)
    source.connect(filter).connect(gain).connect(out)
    source.start(t + delay, Math.random())
    source.stop(t + delay + length + 0.05)
  }

  /** A tone that glides from `from` to `to` Hz and dies away over `length` s. */
  private static ping(ctx: BaseAudioContext, out: AudioNode, t: number, type: OscillatorType, from: number, to: number, length: number, level: number, delay = 0) {
    const osc = ctx.createOscillator()
    osc.type = type
    osc.frequency.setValueAtTime(from, t + delay)
    osc.frequency.exponentialRampToValueAtTime(Math.max(20, to), t + delay + length)
    const gain = ctx.createGain()
    gain.gain.setValueAtTime(0.0001, t + delay)
    gain.gain.linearRampToValueAtTime(level, t + delay + 0.004)
    gain.gain.exponentialRampToValueAtTime(0.0001, t + delay + length)
    osc.connect(gain).connect(out)
    osc.start(t + delay)
    osc.stop(t + delay + length + 0.05)
  }

  /**
   * Plays a recording of `name` at `at` (null: in the listener's head), if one has loaded, as `emit` places
   * it. Returns false when there is none, so the caller can play its synthesised sound instead.
   */
  private play(name: string, at: Vec3 | null, level = 1, rate = 1, spare = 0, reach = HEARING): boolean {
    const buffer = this.ready() && this.bank ? this.bank.pick(name) : null
    if (!buffer) return false
    // Slots are for sounds that matter in the mix; a long tail is not one.
    this.emit(at, Math.min(buffer.duration, 0.8), (ctx, out, t) => VehicleSounds.sample(ctx, out, t, buffer, level, rate), spare, reach)
    return true
  }

  private static sample(ctx: BaseAudioContext, out: AudioNode, t: number, buffer: AudioBuffer, level: number, rate: number, delay = 0) {
    const source = ctx.createBufferSource()
    source.buffer = buffer
    source.playbackRate.value = rate * (0.95 + Math.random() * 0.1)
    const gain = ctx.createGain()
    gain.gain.value = level
    source.connect(gain).connect(out)
    source.start(t + delay)
  }

  /**
   * Something launched from `at`: a rocket's motor lighting with a rush (`rocket`), a bomb let go from
   * its rack with a clunk (`bomb`), or a grenade's pin and the swish of the throw (`grenade`).
   */
  launch(at: Vec3, what: 'rocket' | 'bomb' | 'grenade') {
    this.emit(at, 0.6, (ctx, out, t) => {
      if (what === 'rocket') {
        VehicleSounds.burst(ctx, out, t, 'bandpass', 500, 2600, 0.45, 0.6)
        VehicleSounds.burst(ctx, out, t, 'lowpass', 900, 150, 0.25, 0.5)
        VehicleSounds.ping(ctx, out, t, 'sine', 110, 45, 0.2, 0.45)
      } else if (what === 'bomb') {
        VehicleSounds.ping(ctx, out, t, 'square', 420, 300, 0.07, 0.18)
        VehicleSounds.ping(ctx, out, t, 'sine', 95, 45, 0.15, 0.35)
      } else {
        VehicleSounds.ping(ctx, out, t, 'triangle', 3200, 2600, 0.02, 0.12)
        VehicleSounds.burst(ctx, out, t, 'bandpass', 1600, 600, 0.18, 0.2, 0.08)
      }
    })
  }

  /**
   * Something going into the water at `at`: a round's plop, a body's splash, a vehicle's or a falling
   * section's heavy one. `size` is the splash's (`render/splash.ts`), metres, and `speed` how hard it went in, m/s.
   */
  splash(at: Vec3, size: number, speed: number) {
    if (this.distanceTo(at) > HEARING) return
    const big = size >= SPLASH_BIG
    const level = big ? 1 : Math.min(1, 0.55 + size * 0.5 + speed * 0.03)
    // Bigger is deeper.
    const rate = big ? Math.max(0.6, 1.05 - size * 0.08) : Math.max(0.8, 1.25 - size * 0.4)
    const spare = size < SPLASH_SMALL ? STEP_SPARE : 0
    if (this.play(big ? 'splash_big' : 'splash_small', at, level, rate, spare)) return
    this.emit(
      at,
      big ? 1.2 : 0.5,
      (ctx, out, t) => {
        VehicleSounds.ping(ctx, out, t, 'sine', big ? 120 : 380, big ? 45 : 160, big ? 0.3 : 0.08, level * 0.6)
        VehicleSounds.burst(ctx, out, t, 'bandpass', big ? 900 : 1600, big ? 300 : 700, big ? 0.9 : 0.25, level * 0.7, 0.01)
        if (big) VehicleSounds.burst(ctx, out, t, 'highpass', 3000, 2000, 1.1, level * 0.25, 0.15)
      },
      spare,
    )
  }

  /** A round passing close by the listener at `at`: the snap of something going faster than sound. */
  crack(at: Vec3) {
    this.emit(at, 0.08, (ctx, out, t) => VehicleSounds.burst(ctx, out, t, 'highpass', 5500, 3000, 0.035, 0.45), STEP_SPARE)
  }

  /** A round striking `what` at `at`: a thud of earth, a knock of wood, a crack off stone or brick. */
  impact(at: Vec3, what: 'dirt' | 'wood' | 'stone') {
    if (this.distanceTo(at) > IMPACT_HEARING) return
    if (this.play(`hit_${what}`, at, 0.5, 1, STEP_SPARE)) return
    this.emit(
      at,
      0.2,
      (ctx, out, t) => {
        if (what === 'dirt') VehicleSounds.burst(ctx, out, t, 'lowpass', 900, 200, 0.08, 0.35)
        else if (what === 'wood') VehicleSounds.ping(ctx, out, t, 'triangle', 420, 260, 0.06, 0.25)
        else {
          VehicleSounds.burst(ctx, out, t, 'bandpass', 2800, 1600, 0.05, 0.35)
          VehicleSounds.ping(ctx, out, t, 'triangle', 1900, 1400, 0.04, 0.08)
        }
      },
      STEP_SPARE,
    )
  }

  /** Window glass breaking at `at`, `panes` of it at once. */
  glass(at: Vec3, panes = 1) {
    const level = Math.min(1, 0.45 + 0.12 * panes)
    if (this.play('glass', at, level)) return
    this.emit(at, 0.8, (ctx, out, t) => {
      VehicleSounds.burst(ctx, out, t, 'highpass', 5000, 3500, 0.35, level * 0.5)
      for (let i = 0; i < 4 + Math.min(panes, 6); i++) VehicleSounds.ping(ctx, out, t, 'sine', 2600 + Math.random() * 4500, 2200, 0.08, 0.07 * level, Math.random() * 0.35)
    })
  }

  /**
   * A piece of a building, a bridge or a vehicle landing at `at`, `size` metres across: a clatter of
   * stone for a small one, a crash and a rumble for a section of wall. A building coming down is a few
   * crashes in one rumble, not one for each piece: big landings close together in time and place merge.
   */
  debris(at: Vec3, size: number, now = performance.now()) {
    if (this.distanceTo(at) > HEARING) return
    const big = size >= DEBRIS_BIG
    if (big) {
      const key = `${Math.round(at[0] / COLLAPSE_CELL)},${Math.round(at[2] / COLLAPSE_CELL)}`
      if ((this.collapses.get(key) ?? 0) > now) return
      this.collapses.set(key, now + COLLAPSE_EVERY)
      if (this.collapses.size > 64) for (const [k, until] of this.collapses) if (until < now) this.collapses.delete(k)
    }
    const level = Math.min(1, 0.3 + size * 0.12)
    if (this.play(big ? 'collapse' : 'debris', at, level, big ? 0.9 : 1, big ? 0 : STEP_SPARE)) {
      if (big) this.thud(at, 10 + size * 2)
      return
    }
    this.emit(
      at,
      big ? 1.4 : 0.4,
      (ctx, out, t) => {
        VehicleSounds.burst(ctx, out, t, 'lowpass', big ? 1400 : 2200, big ? 90 : 300, big ? 1.2 : 0.25, level)
        VehicleSounds.ping(ctx, out, t, 'sine', big ? 70 : 140, big ? 30 : 60, big ? 0.6 : 0.12, level * 0.7)
        for (let i = 0; i < (big ? 6 : 2); i++) VehicleSounds.burst(ctx, out, t, 'bandpass', 1800 + Math.random() * 1500, 900, 0.04, level * 0.3, Math.random() * (big ? 0.8 : 0.15))
      },
      big ? 0 : STEP_SPARE,
    )
  }

  // Projectiles in flight: a rocket's motor, a shell's whistle, a falling bomb's.

  beginFlights() {
    this.flying.length = 0
  }

  /** Projectile `id` of `kind` (`PROJECTILE_*`) is at `at` going `velocity` this frame. */
  flight(id: number, kind: number, at: Vec3, velocity: Vec3) {
    const sound = flightOf(kind)
    if (!sound) return
    const distance = this.distanceTo(at)
    if (distance > FLIGHT_HEARING) return
    this.flying.push({ id, sound, distance, at, velocity })
  }

  /** Starts, moves and stops the in-flight voices for the nearest few reported since `beginFlights`. */
  endFlights() {
    const ctx = this.ready()
    const master = this.master
    if (!ctx || !master) return
    this.flying.sort((a, b) => a.distance - b.distance)
    const kept = new Set<number>()
    for (const flight of this.flying.slice(0, MAX_FLIGHTS)) {
      kept.add(flight.id)
      let voice = this.flights.get(flight.id)
      if (!voice) {
        voice = new FlightVoice(ctx, master, flight.sound)
        this.flights.set(flight.id, voice)
      }
      const away = flight.distance > 0.5 ? flight.at.map((v, i) => (this.listener[i]! - v) / flight.distance) : [0, 0, 0]
      const closing = (flight.velocity[0] - this.listenerVelocity[0]) * away[0]! + (flight.velocity[1] - this.listenerVelocity[1]) * away[1]! + (flight.velocity[2] - this.listenerVelocity[2]) * away[2]!
      voice.apply(ctx.currentTime, flight.at, flight.distance, closing, flight.at[1] - this.listener[1])
    }
    for (const [id, voice] of this.flights) {
      if (kept.has(id)) continue
      voice.stop(ctx.currentTime)
      this.flights.delete(id)
    }
  }

  /** Projectile `id` has ended (burst, landed, gone out of view). */
  dropFlight(id: number) {
    const ctx = this.ctx
    const voice = this.flights.get(id)
    if (ctx && voice) voice.stop(ctx.currentTime)
    this.flights.delete(id)
  }

  /** A rifle (`kind` 'rifle'), revolver, sniper rifle or shotgun fired from `at`. The shotgun has no sample yet, so it is always synthesised. */
  gunshot(at: Vec3, kind: 'rifle' | 'revolver' | 'sniper' | 'shotgun') {
    if (kind !== 'shotgun' && this.play(kind, at, kind === 'sniper' ? 1 : 0.85, 1, 0, GUN_REACH)) return
    this.emit(
      at,
      0.6,
      (ctx, out, t) => {
        if (kind === 'shotgun') VehicleSounds.burst(ctx, out, t, 'lowpass', 2200, 200, 0.28, 0.9)
        VehicleSounds.burst(ctx, out, t, 'lowpass', kind === 'sniper' ? 4500 : 3200, 300, kind === 'sniper' ? 0.3 : 0.14, 0.7)
        VehicleSounds.ping(ctx, out, t, 'sine', kind === 'revolver' ? 200 : 140, 50, 0.08, 0.5)
      },
      0,
      GUN_REACH,
    )
  }

  /** A blast at `at`: 'big' for a wreck or bomb, 'small' for a grenade, rocket or round. */
  blast(at: Vec3, size: 'small' | 'big') {
    if (size === 'big') return this.explosion(at)
    if (this.play('blast', at, 0.9, 1, 0, BLAST_REACH)) return
    this.emit(
      at,
      1,
      (ctx, out, t) => {
        VehicleSounds.burst(ctx, out, t, 'lowpass', 1800, 100, 0.7, 0.8)
        VehicleSounds.ping(ctx, out, t, 'sine', 90, 35, 0.4, 0.8)
      },
      0,
      BLAST_REACH,
    )
  }

  /** A collision at `at` that took `speed` m/s out of the vehicle: a bump, a crunch or a smash with glass. */
  crash(at: Vec3, speed: number) {
    const size = crashSize(speed)
    const level = [0.5, 0.8, 1][size]!
    const length = [0.16, 0.32, 0.6][size]!
    this.emit(at, length + 0.6, (ctx, out, t) => {
      VehicleSounds.burst(ctx, out, t, 'lowpass', [1000, 1600, 2600][size]!, 200, length, level)
      VehicleSounds.ping(ctx, out, t, 'sine', 120, 42, length * 0.8, level * 0.8)
      if (size >= 1) {
        VehicleSounds.ping(ctx, out, t, 'square', 190, 140, 0.2, 0.1 * level)
        VehicleSounds.ping(ctx, out, t, 'square', 251, 170, 0.18, 0.08 * level, 0.02)
      }
      if (size === 2) {
        VehicleSounds.burst(ctx, out, t, 'highpass', 4500, 3000, 0.45, 0.3)
        for (let i = 0; i < 5; i++) VehicleSounds.ping(ctx, out, t, 'sine', 2500 + Math.random() * 4000, 2000, 0.09, 0.06, 0.03 + Math.random() * 0.3)
      }
    })
  }

  /** A vehicle striking a tree: a dull wooden whump, a shiver of leaves, louder for a harder blow. */
  treeHit(at: Vec3, size: number) {
    const level = Math.min(1, 0.3 + size / 25)
    this.emit(at, 1.0, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'sine', 140, 50, 0.25, 0.9 * level)
      VehicleSounds.burst(ctx, out, t, 'lowpass', 900, 200, 0.22, 0.8 * level)
      VehicleSounds.burst(ctx, out, t, 'highpass', 3000, 2000, 0.5, 0.2 * level, 0.03)
    })
  }

  /** A tree of `size` metres splitting at its foot. */
  treeCrack(at: Vec3, size: number) {
    const level = Math.min(1, 0.4 + size / 25)
    this.emit(at, 1.0, (ctx, out, t) => {
      VehicleSounds.burst(ctx, out, t, 'bandpass', 1800, 500, 0.35, 0.8 * level)
      VehicleSounds.ping(ctx, out, t, 'square', 320, 90, 0.15, 0.3 * level)
      for (let i = 0; i < 4; i++) VehicleSounds.ping(ctx, out, t, 'square', 600 + Math.random() * 900, 200, 0.05, 0.15 * level, i * 0.07 + Math.random() * 0.05)
    })
  }

  /** A felled tree of `size` metres hitting the ground: a heavy thump and a rustle of leaves. */
  treeThud(at: Vec3, size: number) {
    const level = Math.min(1, 0.4 + size / 25)
    this.emit(at, 1.2, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'sine', 90, 35, 0.35, 0.9 * level)
      VehicleSounds.burst(ctx, out, t, 'lowpass', 700, 150, 0.3, 0.6 * level)
      VehicleSounds.burst(ctx, out, t, 'highpass', 3500, 2500, 0.6, 0.15 * level, 0.05)
    })
  }

  /** A hard landing after a drop of `fall` m/s. */
  thud(at: Vec3, fall: number) {
    const level = landingThud(fall)
    if (level <= 0) return
    this.emit(at, 0.4, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'sine', 110, 45, 0.2, 0.7 * level)
      VehicleSounds.burst(ctx, out, t, 'lowpass', 500, 120, 0.18, 0.5 * level)
    })
  }

  /** A vehicle blowing up. */
  explosion(at: Vec3) {
    if (this.play('boom', at, 1, 1, 0, BOOM_REACH)) return
    this.emit(
      at,
      2,
      (ctx, out, t) => {
        VehicleSounds.burst(ctx, out, t, 'lowpass', 2200, 80, 1.6, 1)
        VehicleSounds.ping(ctx, out, t, 'sine', 80, 28, 0.9, 1)
        VehicleSounds.burst(ctx, out, t, 'bandpass', 900, 200, 0.5, 0.4, 0.08)
      },
      0,
      BOOM_REACH,
    )
  }

  /** A tank's or helicopter's gun, and the casing clanking after it. */
  cannon(at: Vec3, heavy: boolean) {
    const reach = heavy ? BOOM_REACH : BLAST_REACH
    if (this.play('cannon', at, heavy ? 1 : 0.7, heavy ? 1 : 1.35, 0, reach)) return
    this.emit(
      at,
      1.2,
      (ctx, out, t) => {
        VehicleSounds.burst(ctx, out, t, 'lowpass', heavy ? 1400 : 3000, 150, heavy ? 0.4 : 0.12, heavy ? 0.9 : 0.5)
        VehicleSounds.ping(ctx, out, t, 'sine', heavy ? 95 : 160, 38, heavy ? 0.35 : 0.1, heavy ? 0.9 : 0.4)
        if (heavy) {
          VehicleSounds.ping(ctx, out, t, 'square', 1700, 1500, 0.05, 0.06, 0.35)
          VehicleSounds.ping(ctx, out, t, 'square', 2300, 2000, 0.04, 0.05, 0.4)
        }
      },
      0,
      reach,
    )
  }

  /** A shell glancing off armor: a ringing metallic ping with a short scrape of sparks under it. */
  ricochet(at: Vec3) {
    this.emit(at, 0.9, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'triangle', 2400, 1500, 0.5, 0.5)
      VehicleSounds.ping(ctx, out, t, 'sine', 3700, 3000, 0.35, 0.3, 0.01)
      VehicleSounds.burst(ctx, out, t, 'highpass', 3500, 2500, 0.12, 0.35)
    })
  }

  /** A round punching through armour: a hard clang with a dull thump behind it, deeper for a shell. */
  pierce(at: Vec3, big: boolean) {
    this.emit(at, big ? 1.2 : 0.7, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'square', big ? 520 : 1400, big ? 180 : 700, big ? 0.35 : 0.25, big ? 0.35 : 0.15)
      VehicleSounds.ping(ctx, out, t, 'sine', big ? 90 : 220, big ? 40 : 120, big ? 0.6 : 0.3, big ? 0.4 : 0.15, 0.005)
      VehicleSounds.burst(ctx, out, t, 'lowpass', big ? 1800 : 3200, big ? 300 : 900, big ? 0.3 : 0.12, big ? 0.6 : 0.3)
    })
  }

  /** A turret ripped off its ring: a deep metallic bang and a long groan as it comes away. */
  turretOff(at: Vec3) {
    this.emit(at, 1.4, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'sine', 70, 28, 0.7, 1.0)
      VehicleSounds.burst(ctx, out, t, 'lowpass', 900, 120, 0.5, 0.9)
      VehicleSounds.ping(ctx, out, t, 'sawtooth', 220, 90, 0.6, 0.3, 0.05)
      VehicleSounds.ping(ctx, out, t, 'square', 1300, 600, 0.2, 0.2, 0.03)
    })
  }

  /** A ski pole going into the snow: a dry little crunch. */
  polePlant(at: Vec3) {
    this.emit(at, 0.2, (ctx, out, t) => {
      VehicleSounds.burst(ctx, out, t, 'bandpass', 2600, 900, 0.09, 0.16)
      VehicleSounds.burst(ctx, out, t, 'lowpass', 500, 180, 0.07, 0.12)
    })
  }

  /** Skis running over snow at `speed` m/s, heard as the skier hears them; 0 stops it. */
  glide(speed: number) {
    const ctx = this.ready()
    if (!ctx || !this.master) return
    if (!this.skiHiss && speed > 0) {
      const noise = ctx.createBufferSource()
      noise.buffer = noiseBuffer(ctx)
      noise.loop = true
      const filter = ctx.createBiquadFilter()
      filter.type = 'bandpass'
      filter.Q.value = 0.7
      const gain = ctx.createGain()
      gain.gain.value = 0
      noise.connect(filter).connect(gain).connect(this.dry ?? this.master)
      noise.start()
      this.skiHiss = { filter, gain }
    }
    if (!this.skiHiss) return
    const pace = Math.min(speed / 20, 1)
    this.skiHiss.filter.frequency.setTargetAtTime(700 + 2600 * pace, ctx.currentTime, 0.08)
    this.skiHiss.gain.gain.setTargetAtTime(0.22 * Math.sqrt(pace), ctx.currentTime, 0.08)
  }

  /**
   * A soldier's foot coming down at `at` (null: the listener's own) on `footing`, `weight` 0 a crawl's
   * shuffle to 1 a sprint: the heel's thump and what it lands on (a scuff on tarmac, gravel's crunch,
   * snow's squeak, a squelch, a splash, a hollow floor), never twice the same, and the kit rattling on a
   * sprint. Steps are heard only close by, and give way to everything else.
   */
  step(at: Vec3 | null, footing: Footing, weight: number) {
    if (at && this.distanceTo(at) > STEP_HEARING) return
    const level = 0.12 + 0.3 * weight
    const jitter = () => 0.85 + Math.random() * 0.3
    this.emit(
      at,
      0.25,
      (ctx, out, t) => {
        const burst = (type: BiquadFilterType, from: number, to: number, length: number, gain: number, delay = 0) =>
          VehicleSounds.burst(ctx, out, t, type, from * jitter(), to, length * jitter(), gain * level * jitter(), delay)
        const crunch = (hz: number, count: number, spread: number, gain: number) => {
          for (let i = 0; i < count; i++) burst('bandpass', hz, hz * 0.7, 0.014, gain, Math.random() * spread)
        }
        // The heel.
        if (footing !== 'wade' && footing !== 'snow') burst('lowpass', 420, 110, 0.06, 0.7)
        switch (footing) {
          case 'tarmac': burst('bandpass', 2600, 1800, 0.035, 0.35); break
          case 'gravel': crunch(3000, 5, 0.09, 0.45); break
          case 'grass': burst('highpass', 3200, 2200, 0.09, 0.18); break
          case 'sand': burst('highpass', 4200, 3000, 0.07, 0.25); break
          case 'mud':
            burst('lowpass', 750, 200, 0.14, 0.6)
            VehicleSounds.ping(ctx, out, t, 'sine', 200 * jitter(), 90, 0.07, 0.25 * level, 0.03)
            break
          case 'snow':
            burst('lowpass', 300, 120, 0.08, 0.5)
            crunch(1400, 6, 0.12, 0.4)
            break
          case 'ice': VehicleSounds.ping(ctx, out, t, 'triangle', 2100 * jitter(), 1700, 0.03, 0.12 * level); break
          case 'water':
          case 'wade':
            burst('bandpass', 950, 380, 0.24, 0.8)
            burst('highpass', 3200, 2400, 0.16, 0.25, 0.02)
            break
          case 'floor':
            VehicleSounds.ping(ctx, out, t, 'sine', 150 * jitter(), 105, 0.09, 0.45 * level)
            burst('bandpass', 850, 600, 0.04, 0.35)
            break
        }
        // At a sprint the kit on them rattles.
        if (weight > 0.8) burst('bandpass', 5200, 4200, 0.025, 0.12, 0.02 + Math.random() * 0.04)
      },
      STEP_SPARE,
    )
  }

  /** A walker's foot coming down: a heavy thump under a dull metal clank. `weight` is 0..1. */
  footstep(at: Vec3, weight: number) {
    const level = 0.5 + 0.5 * weight
    this.emit(at, 0.8, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'sine', 85, 30, 0.45, 1.0 * level)
      VehicleSounds.burst(ctx, out, t, 'lowpass', 400, 60, 0.3, 0.9 * level)
      VehicleSounds.ping(ctx, out, t, 'triangle', 190, 90, 0.12, 0.35 * level, 0.01)
      VehicleSounds.burst(ctx, out, t, 'bandpass', 1500, 600, 0.06, 0.22 * level, 0.01)
    })
  }

  /** A hen leaping: a giant, robotic "BA-WK!", a short rising squawk and a falling one over a square-wave growl. */
  bawk(at: Vec3) {
    this.emit(at, 0.9, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'sawtooth', 330, 500, 0.12, 0.45)
      VehicleSounds.ping(ctx, out, t, 'sawtooth', 560, 250, 0.3, 0.55, 0.13)
      VehicleSounds.ping(ctx, out, t, 'square', 165, 120, 0.42, 0.22)
      VehicleSounds.burst(ctx, out, t, 'bandpass', 1400, 600, 0.08, 0.15, 0.13)
    })
  }

  /**
   * A zombie's groan from `at`: a low, rasping moan that wavers as it swells and dies away, at `pitch`
   * times the usual note, so a horde does not groan in unison.
   */
  groan(at: Vec3, pitch = 1) {
    const length = 1.1 + Math.random() * 0.8
    this.emit(at, length, (ctx, out, t) => {
      const voice = ctx.createOscillator()
      voice.type = 'sawtooth'
      const note = 95 * pitch
      voice.frequency.setValueAtTime(note * 1.15, t)
      voice.frequency.exponentialRampToValueAtTime(note, t + length * 0.4)
      voice.frequency.exponentialRampToValueAtTime(note * 0.8, t + length)
      // The waver: a slow wobble of the note.
      const wobble = ctx.createOscillator()
      wobble.frequency.value = 5 + Math.random() * 3
      const depth = ctx.createGain()
      depth.gain.value = note * 0.06
      wobble.connect(depth).connect(voice.frequency)
      // The mouth: a formant round an "uh".
      const mouth = ctx.createBiquadFilter()
      mouth.type = 'bandpass'
      mouth.frequency.value = 520
      mouth.Q.value = 2.5
      const gain = ctx.createGain()
      gain.gain.setValueAtTime(0.0001, t)
      gain.gain.linearRampToValueAtTime(0.9, t + length * 0.3)
      gain.gain.exponentialRampToValueAtTime(0.0001, t + length)
      voice.connect(mouth).connect(gain).connect(out)
      voice.start(t)
      wobble.start(t)
      voice.stop(t + length + 0.05)
      wobble.stop(t + length + 0.05)
      // The rasp of breath through it.
      VehicleSounds.burst(ctx, out, t, 'bandpass', 900, 500, length * 0.9, 0.12, 0.05)
    })
  }

  /** A zombie's claws swung at someone from `at`: a swish, and a growl as it lunges. */
  claw(at: Vec3) {
    this.emit(at, 0.6, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'sawtooth', 160, 90, 0.35, 0.35)
      VehicleSounds.burst(ctx, out, t, 'bandpass', 1500, 500, 0.18, 0.4, 0.3)
    })
  }

  /** One beat of a hen's wings: a whoosh of air and a low flap. */
  wingBeat(at: Vec3) {
    this.emit(at, 0.4, (ctx, out, t) => {
      VehicleSounds.burst(ctx, out, t, 'bandpass', 650, 350, 0.18, 0.45)
      VehicleSounds.ping(ctx, out, t, 'sine', 95, 55, 0.14, 0.35)
    })
  }

  /** A golf club swung: a whoosh of air, and a crisp click where it met a ball (`struck`). */
  golfSwing(at: Vec3, struck: boolean) {
    this.emit(at, 0.5, (ctx, out, t) => {
      VehicleSounds.burst(ctx, out, t, 'bandpass', 900, 700, 0.16, 0.25)
      if (!struck) return
      VehicleSounds.ping(ctx, out, t, 'triangle', 2400, 1400, 0.05, 0.5, 0.12)
      VehicleSounds.burst(ctx, out, t, 'highpass', 3500, 2500, 0.04, 0.35, 0.12)
    })
  }

  /** A golf ball dropping into the cup: a hollow knock and a little rattle. */
  golfCup(at: Vec3) {
    this.emit(at, 0.6, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'sine', 700, 380, 0.12, 0.5)
      for (let i = 0; i < 3; i++) VehicleSounds.ping(ctx, out, t, 'triangle', 1100 + i * 180, 700, 0.04, 0.25 / (i + 1), 0.08 + i * 0.07)
    })
  }

  /** An egg leaving a hen: a hollow plop and a clunk of the hatch. */
  plop(at: Vec3) {
    this.emit(at, 0.4, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'sine', 460, 150, 0.14, 0.55)
      VehicleSounds.burst(ctx, out, t, 'lowpass', 900, 200, 0.05, 0.25)
      VehicleSounds.ping(ctx, out, t, 'triangle', 220, 140, 0.08, 0.2, 0.06)
    })
  }

  /** A blaster bolt: a bright zap that falls steeply in pitch, with a crack at the front and a low thrum. */
  /** A sound that can be cut off: `stop` fades it out in a moment. */
  private voice(at: Vec3 | null, life: number, build: (ctx: BaseAudioContext, out: AudioNode, t: number) => void) {
    let stop = () => {}
    this.emit(at, life, (ctx, out, t) => {
      const level = ctx.createGain()
      level.gain.value = 0.55
      level.connect(out)
      build(ctx, level, t)
      stop = () => level.gain.setTargetAtTime(0, ctx.currentTime, 0.04)
    })
    return () => stop()
  }

  /** An oscillator held for `length` s, gliding from `from` to `to` Hz and swelling from `start` to `end`. */
  private static drone(ctx: BaseAudioContext, out: AudioNode, t: number, type: OscillatorType, from: number, to: number, length: number, start: number, end: number, detune = 0) {
    const osc = ctx.createOscillator()
    osc.type = type
    osc.detune.value = detune
    osc.frequency.setValueAtTime(from, t)
    osc.frequency.exponentialRampToValueAtTime(to, t + length)
    const gain = ctx.createGain()
    gain.gain.setValueAtTime(Math.max(0.0001, start), t)
    gain.gain.exponentialRampToValueAtTime(Math.max(0.0001, end), t + length)
    gain.gain.linearRampToValueAtTime(0.0001, t + length + 0.08)
    osc.connect(gain).connect(out)
    osc.start(t)
    osc.stop(t + length + 0.1)
    return gain
  }

  /** Noise through a band that sweeps from `from` to `to` Hz over `length` s, held at `level`. */
  private static roar(ctx: BaseAudioContext, out: AudioNode, t: number, from: number, to: number, q: number, length: number, level: number) {
    const source = ctx.createBufferSource()
    source.buffer = noiseBuffer(ctx)
    source.loop = true
    const filter = ctx.createBiquadFilter()
    filter.type = 'bandpass'
    filter.Q.value = q
    filter.frequency.setValueAtTime(from, t)
    filter.frequency.exponentialRampToValueAtTime(to, t + length)
    const gain = ctx.createGain()
    gain.gain.setValueAtTime(0.0001, t)
    gain.gain.linearRampToValueAtTime(level, t + 0.03)
    gain.gain.setValueAtTime(level, t + length - 0.05)
    gain.gain.linearRampToValueAtTime(0.0001, t + length + 0.15)
    source.connect(filter).connect(gain).connect(out)
    source.start(t, Math.random())
    source.stop(t + length + 0.2)
  }

  /** A saturating stage that everything fearsome goes through, into `out`. */
  private static grit(ctx: BaseAudioContext, out: AudioNode, drive: number) {
    const curve = new Float32Array(1024)
    for (let i = 0; i < curve.length; i++) curve[i] = Math.tanh(((i / (curve.length - 1)) * 2 - 1) * drive)
    const shaper = ctx.createWaveShaper()
    shaper.curve = curve
    shaper.oversample = '2x'
    shaper.connect(out)
    return shaper
  }

  /** Something throbbing `gain` at `hz`, faster and faster up to `to` over `length` s. */
  private static flutter(ctx: BaseAudioContext, gain: AudioParam, t: number, hz: number, to: number, depth: number, length: number) {
    const lfo = ctx.createOscillator()
    lfo.type = 'sawtooth'
    lfo.frequency.setValueAtTime(hz, t)
    lfo.frequency.exponentialRampToValueAtTime(to, t + length)
    const amount = ctx.createGain()
    amount.gain.value = depth
    lfo.connect(amount).connect(gain)
    lfo.start(t)
    lfo.stop(t + length + 0.1)
  }

  /** Eyes charging over `seconds`: a growl climbing into a shriek, pulsing faster and faster. Returns how to cut it off. */
  laserCharge(at: Vec3, seconds: number) {
    return this.voice(at, seconds + 0.2, (ctx, out, t) => {
      const pulse = ctx.createGain()
      pulse.gain.value = 0.7
      VehicleSounds.flutter(ctx, pulse.gain, t, 4, 42, 0.35, seconds)
      const filter = ctx.createBiquadFilter()
      filter.type = 'lowpass'
      filter.Q.value = 9
      filter.frequency.setValueAtTime(180, t)
      filter.frequency.exponentialRampToValueAtTime(5200, t + seconds)
      pulse.connect(filter).connect(VehicleSounds.grit(ctx, out, 2.5))
      for (const detune of [-14, 0, 13]) VehicleSounds.drone(ctx, pulse, t, 'sawtooth', 55, 740, seconds, 0.1, 0.34, detune)
      VehicleSounds.drone(ctx, out, t, 'sine', 40, 110, seconds, 0.2, 0.5)
      VehicleSounds.drone(ctx, out, t, 'sine', 900, 6200, seconds, 0.003, 0.09)
      VehicleSounds.roar(ctx, out, t, 500, 7000, 5, seconds, 0.12)
    })
  }

  /** Charged eyes dying away unfired: the whine falling off. */
  laserFizzle(at: Vec3) {
    this.emit(at, 0.5, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'sawtooth', 500, 45, 0.4, 0.22)
      VehicleSounds.burst(ctx, out, t, 'highpass', 3000, 900, 0.3, 0.1)
    })
  }

  /**
   * The beam, for `seconds`: a crack and a drop like a shell's, then a wall of distorted sawtooth
   * fluttering at the edge of a pitch, a tearing roar, and over it all a robot rooster's scream.
   */
  laserBeam(at: Vec3, seconds: number) {
    return this.voice(at, seconds + 0.5, (ctx, out, t) => {
      VehicleSounds.burst(ctx, out, t, 'lowpass', 7000, 90, 0.7, 1)
      VehicleSounds.ping(ctx, out, t, 'sine', 170, 26, 0.9, 1)
      const wall = ctx.createGain()
      wall.gain.value = 0.6
      VehicleSounds.flutter(ctx, wall.gain, t, 38, 31, 0.4, seconds)
      wall.connect(VehicleSounds.grit(ctx, out, 4))
      for (const [hz, detune] of [[46, -9], [46, 8], [69, 5], [92, -6], [139, 7]] as const) VehicleSounds.drone(ctx, wall, t, 'sawtooth', hz, hz * 0.84, seconds, 0.3, 0.26, detune)
      VehicleSounds.drone(ctx, out, t, 'square', 34, 28, seconds, 0.3, 0.26)
      VehicleSounds.roar(ctx, out, t, 1300, 650, 0.8, seconds, 0.7)
      VehicleSounds.roar(ctx, out, t, 5200, 3000, 3, seconds, 0.22)
      const scream = VehicleSounds.grit(ctx, out, 3)
      for (const [hz, level] of [[640, 0.2], [963, 0.13]] as const) {
        VehicleSounds.drone(ctx, scream, t, 'sawtooth', hz, hz * 1.5, seconds * 0.4, level * 0.5, level)
        const fall = t + seconds * 0.4
        VehicleSounds.drone(ctx, scream, fall, 'sawtooth', hz * 1.5, hz * 0.7, seconds * 0.6, level, level * 0.4)
      }
    })
  }

  /** The beam biting into something. */
  laserSizzle(at: Vec3) {
    this.emit(at, 0.2, (ctx, out, t) => {
      VehicleSounds.burst(ctx, out, t, 'highpass', 5000, 2500, 0.16, 0.35)
      VehicleSounds.burst(ctx, out, t, 'bandpass', 900, 300, 0.12, 0.3)
    })
  }

  laser(at: Vec3) {
    this.emit(at, 0.8, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'sawtooth', 2600, 260, 0.22, 0.4)
      VehicleSounds.ping(ctx, out, t, 'square', 1300, 140, 0.26, 0.3, 0.005)
      VehicleSounds.ping(ctx, out, t, 'sine', 5200, 900, 0.1, 0.25)
      VehicleSounds.burst(ctx, out, t, 'highpass', 4000, 2000, 0.05, 0.3)
      VehicleSounds.ping(ctx, out, t, 'sine', 120, 50, 0.3, 0.5, 0.02)
    })
  }

  /**
   * `rounds` from a machine gun at `at`, a twelfth of a second apart: a crack and a thump each. A
   * plane's ring gun (`rear`) is a lighter, higher one than the pair on its cowling. One slot covers them.
   */
  machineGun(at: Vec3, rounds: number, rear = false) {
    if (rounds <= 0) return
    const spacing = 1 / 12
    const recorded = this.ready() && this.bank ? this.bank.pick('mg') : null
    this.emit(
      at,
      0.2 + rounds * spacing,
      (ctx, out, t) => {
        for (let k = 0; k < rounds; k++) {
          const delay = k * spacing
          if (recorded) {
            const buffer = this.bank?.pick('mg') ?? recorded
            VehicleSounds.sample(ctx, out, t, buffer, rear ? 0.55 : 0.75, rear ? 1.25 : 1, delay)
            continue
          }
          VehicleSounds.burst(ctx, out, t, 'bandpass', rear ? 3400 : 2600, 700, 0.06, rear ? 0.45 : 0.55, delay)
          VehicleSounds.ping(ctx, out, t, 'sine', rear ? 220 : 170, 60, 0.05, rear ? 0.35 : 0.5, delay)
        }
      },
      0,
      GUN_REACH,
    )
  }

  /** A door closing, heard in the listener's own head. */
  door() {
    this.emit(null, 0.4, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'sine', 140, 60, 0.12, 0.5)
      VehicleSounds.burst(ctx, out, t, 'bandpass', 1800, 900, 0.05, 0.3, 0.02)
    })
  }

  /** A hit you landed, heard in your own head: a bright tick, and a low thump under it if it finished them. */
  hit(kill: boolean) {
    this.emit(null, 0.5, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'square', 1900, 1500, 0.05, 0.16)
      VehicleSounds.burst(ctx, out, t, 'highpass', 3500, 2500, 0.04, 0.18)
      if (kill) {
        VehicleSounds.ping(ctx, out, t, 'sine', 160, 60, 0.28, 0.6, 0.03)
        VehicleSounds.ping(ctx, out, t, 'square', 1300, 1000, 0.08, 0.14, 0.07)
      }
    })
  }

  /** `amount` hp of harm taken, heard in your own head: a dull thump, harder the more it hurt. */
  hurt(amount: number) {
    const level = Math.min(1, 0.35 + amount / 50)
    this.emit(null, 0.5, (ctx, out, t) => {
      VehicleSounds.ping(ctx, out, t, 'sine', 130, 50, 0.22, 0.8 * level)
      VehicleSounds.burst(ctx, out, t, 'lowpass', 1100, 250, 0.16, 0.55 * level)
    })
  }

  dispose() {
    for (const voice of this.voices.values()) voice.stop()
    this.voices.clear()
    if (this.ctx) for (const voice of this.flights.values()) voice.stop(this.ctx.currentTime)
    this.flights.clear()
    this.candidates.length = 0
  }
}
