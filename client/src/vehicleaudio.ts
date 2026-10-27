import { audioContext } from './audio'
import { SampleBank } from './sfx'
import { kindView } from '@/kinds'
import type { KindSound } from '@/kinds/kind'
import { crashSize, landingThud, mixOf, type SoundInput, type SoundMix } from '@/sim/engine'

/**
 * Vehicle sound, synthesised like the rest: an engine from a few detuned oscillators through a
 * low-pass, noise run through filters for tyres, gravel, scrapes and fire, and one-shot bursts for
 * crashes, landings, explosions and cannon fire. `sim/engine.ts` says how loud and how high each
 * layer is; this file only turns that into nodes.
 */

export type Vec3 = readonly [number, number, number]

/** Engines further than this are not heard, metres, and no more than this many play at once. */
const HEARING = 160
const MAX_VOICES = 6
/** One-shots still sounding at once, at most. */
const MAX_SHOTS = 14
const SPEED_OF_SOUND = 343
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
  private readonly oscillators: { node: OscillatorNode; ratio: number }[] = []
  private readonly tone: BiquadFilterNode
  private readonly engine: GainNode
  private readonly stutter: GainNode
  private readonly beat: OscillatorNode
  private readonly beatLevel: GainNode
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
    sound: KindSound,
    master: AudioNode,
  ) {
    this.fade = ctx.createGain()
    this.fade.connect(master)
    this.panner = makePanner(ctx)
    this.panner.connect(this.fade)

    // Engine: oscillators → low-pass → level → dropout gain → panner.
    this.stutter = ctx.createGain()
    this.stutter.connect(this.panner)
    this.engine = ctx.createGain()
    this.engine.gain.value = 0
    this.engine.connect(this.stutter)
    this.tone = ctx.createBiquadFilter()
    this.tone.type = 'lowpass'
    this.tone.Q.value = sound.q
    for (const [type, ratio, level] of sound.voices) {
      const node = ctx.createOscillator()
      node.type = type
      node.frequency.value = 50 * ratio
      const gain = ctx.createGain()
      gain.gain.value = level
      node.connect(gain).connect(this.tone)
      this.sources.push(node)
      this.oscillators.push({ node, ratio })
    }

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
    // Blades and tracks: noise chopped by a low-frequency oscillator, into the engine chain.
    const chopped = ctx.createGain()
    chopped.gain.value = 0
    const chopFilter = ctx.createBiquadFilter()
    chopFilter.type = sound.chop.type
    chopFilter.frequency.value = sound.chop.hz
    noise.connect(chopFilter).connect(chopped).connect(this.tone)
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

  /** Moves the sound to `at` (the listener's own place for a vehicle the listener is in). */
  moveTo(at: Vec3) {
    place(this.panner, at)
  }

  apply(mix: SoundMix, doppler: number, scraping: boolean, dt: number) {
    if (this.stopped) return
    const t = this.ctx.currentTime
    for (const { node, ratio } of this.oscillators) node.frequency.setTargetAtTime(mix.pitch * ratio * doppler, t, 0.05)
    this.tone.frequency.setTargetAtTime(mix.tone, t, 0.1)
    this.engine.gain.setTargetAtTime(mix.engine, t, 0.08)
    this.beat.frequency.setTargetAtTime(Math.max(1, mix.chop * doppler), t, 0.1)
    this.beatLevel.gain.setTargetAtTime(mix.chopLevel * 0.5, t, 0.1)
    this.squeal.gain.gain.setTargetAtTime(mix.squeal * 0.35, t, 0.04)
    this.squeal.filter.frequency.setTargetAtTime(mix.squealHz, t, 0.05)
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
  }
}

interface Candidate {
  id: number
  kind: number
  distance: number
  own: boolean
  mix: SoundMix
  position: Vec3
  doppler: number
}

/**
 * All the vehicle sounds of a game: a voice per vehicle that is near and running (the nearest few),
 * plus one-shots. Every call is harmless without a Web Audio context.
 */
export class VehicleSounds {
  private ctx: BaseAudioContext | null = null
  private master: GainNode | null = null
  private bank: SampleBank | null = null
  private readonly voices = new Map<number, Voice>()
  private readonly candidates: Candidate[] = []
  private readonly scrapes = new Map<number, number>()
  private readonly shots: number[] = []
  private listener: Vec3 = [0, 0, 0]
  private listenerVelocity: Vec3 = [0, 0, 0]

  /** `context` replaces the shared one, for tests. */
  constructor(private readonly provided?: BaseAudioContext) {}

  /** Number of vehicles currently sounding. */
  get voiceCount() {
    return this.voices.size
  }

  private ready(): BaseAudioContext | null {
    if (this.ctx) return this.ctx
    const ctx = this.provided ?? audioContext()
    if (!ctx) return null
    const compressor = ctx.createDynamicsCompressor()
    compressor.connect(ctx.destination)
    this.master = ctx.createGain()
    this.master.gain.value = 0.7
    this.master.connect(compressor)
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
    this.candidates.push({ id, kind: input.kind, distance, own, mix: mixOf(input, kindView(input.kind).sound.engine), position, doppler })
  }

  /** Starts, updates and stops voices for what was reported since `begin`. */
  end(dt: number, now = performance.now()) {
    if (this.candidates.length === 0 && this.voices.size === 0) return
    const ctx = this.ready()
    if (!ctx || !this.master) return
    this.candidates.sort((a, b) => Number(b.own) - Number(a.own) || a.distance - b.distance)
    const chosen = new Set<number>()
    for (const candidate of this.candidates.slice(0, MAX_VOICES)) {
      chosen.add(candidate.id)
      let voice = this.voices.get(candidate.id)
      if (!voice) {
        voice = new Voice(ctx, kindView(candidate.kind).sound, this.master)
        this.voices.set(candidate.id, voice)
      }
      voice.moveTo(candidate.own ? this.listener : candidate.position)
      voice.apply(candidate.mix, candidate.doppler, (this.scrapes.get(candidate.id) ?? 0) > now, dt)
    }
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
    this.scrapes.delete(id)
  }

  /** Vehicle `id` is scraping something: a grind plays for the next fraction of a second. */
  scrape(id: number, now = performance.now()) {
    this.scrapes.set(id, now + 220)
  }

  // One-shots.

  private emit(at: Vec3 | null, life: number, build: (ctx: BaseAudioContext, out: AudioNode, t: number) => void) {
    const ctx = this.ready()
    if (!ctx || !this.master) return
    const now = ctx.currentTime
    while (this.shots.length > 0 && this.shots[0]! < now) this.shots.shift()
    if (this.shots.length >= MAX_SHOTS) return
    let out: AudioNode = this.master
    if (at) {
      if (this.distanceTo(at) > HEARING) return
      const panner = makePanner(ctx)
      place(panner, at)
      panner.connect(this.master)
      out = panner
    }
    this.shots.push(now + life)
    build(ctx, out, now + 0.01)
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
   * Plays a recording of `name` at `at` (null: in the listener's head), if one has loaded. Far shots arrive
   * late, at the speed of sound, and dull: air takes the highs first. Returns false when there is none, so
   * the caller can play its synthesised sound instead.
   */
  private play(name: string, at: Vec3 | null, level = 1, rate = 1): boolean {
    const buffer = this.ready() && this.bank ? this.bank.pick(name) : null
    if (!buffer) return false
    const distance = at ? this.distanceTo(at) : 0
    // Slots are for sounds that matter in the mix; a long tail is not one.
    this.emit(at, Math.min(buffer.duration, 0.8), (ctx, out, t) => VehicleSounds.sample(ctx, out, t, buffer, level, rate, distance))
    return true
  }

  private static sample(ctx: BaseAudioContext, out: AudioNode, t: number, buffer: AudioBuffer, level: number, rate: number, distance: number, delay = 0) {
    const source = ctx.createBufferSource()
    source.buffer = buffer
    source.playbackRate.value = rate * (0.95 + Math.random() * 0.1)
    const gain = ctx.createGain()
    gain.gain.value = level
    const filter = ctx.createBiquadFilter()
    filter.type = 'lowpass'
    filter.frequency.value = Math.max(1500, 18000 / (1 + distance / 50))
    source.connect(filter).connect(gain).connect(out)
    source.start(t + delay + Math.min(0.6, distance / SPEED_OF_SOUND))
  }

  /** A rifle (`kind` 'rifle'), revolver, sniper rifle or shotgun fired from `at`. The shotgun has no sample yet, so it is always synthesised. */
  gunshot(at: Vec3, kind: 'rifle' | 'revolver' | 'sniper' | 'shotgun') {
    if (kind !== 'shotgun' && this.play(kind, at, kind === 'sniper' ? 1 : 0.85)) return
    this.emit(at, 0.6, (ctx, out, t) => {
      if (kind === 'shotgun') VehicleSounds.burst(ctx, out, t, 'lowpass', 2200, 200, 0.28, 0.9)
      VehicleSounds.burst(ctx, out, t, 'lowpass', kind === 'sniper' ? 4500 : 3200, 300, kind === 'sniper' ? 0.3 : 0.14, 0.7)
      VehicleSounds.ping(ctx, out, t, 'sine', kind === 'revolver' ? 200 : 140, 50, 0.08, 0.5)
    })
  }

  /** A blast at `at`: 'big' for a wreck or bomb, 'small' for a grenade, rocket or round. */
  blast(at: Vec3, size: 'small' | 'big') {
    if (size === 'big') return this.explosion(at)
    if (this.play('blast', at, 0.9)) return
    this.emit(at, 1, (ctx, out, t) => {
      VehicleSounds.burst(ctx, out, t, 'lowpass', 1800, 100, 0.7, 0.8)
      VehicleSounds.ping(ctx, out, t, 'sine', 90, 35, 0.4, 0.8)
    })
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
    if (this.play('boom', at, 1)) return
    this.emit(at, 2, (ctx, out, t) => {
      VehicleSounds.burst(ctx, out, t, 'lowpass', 2200, 80, 1.6, 1)
      VehicleSounds.ping(ctx, out, t, 'sine', 80, 28, 0.9, 1)
      VehicleSounds.burst(ctx, out, t, 'bandpass', 900, 200, 0.5, 0.4, 0.08)
    })
  }

  /** A tank's or helicopter's gun, and the casing clanking after it. */
  cannon(at: Vec3, heavy: boolean) {
    if (this.play('cannon', at, heavy ? 1 : 0.7, heavy ? 1 : 1.35)) return
    this.emit(at, 1.2, (ctx, out, t) => {
      VehicleSounds.burst(ctx, out, t, 'lowpass', heavy ? 1400 : 3000, 150, heavy ? 0.4 : 0.12, heavy ? 0.9 : 0.5)
      VehicleSounds.ping(ctx, out, t, 'sine', heavy ? 95 : 160, 38, heavy ? 0.35 : 0.1, heavy ? 0.9 : 0.4)
      if (heavy) {
        VehicleSounds.ping(ctx, out, t, 'square', 1700, 1500, 0.05, 0.06, 0.35)
        VehicleSounds.ping(ctx, out, t, 'square', 2300, 2000, 0.04, 0.05, 0.4)
      }
    })
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

  /** One beat of a hen's wings: a whoosh of air and a low flap. */
  wingBeat(at: Vec3) {
    this.emit(at, 0.4, (ctx, out, t) => {
      VehicleSounds.burst(ctx, out, t, 'bandpass', 650, 350, 0.18, 0.45)
      VehicleSounds.ping(ctx, out, t, 'sine', 95, 55, 0.14, 0.35)
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
    this.emit(at, 0.2 + rounds * spacing, (ctx, out, t) => {
      for (let k = 0; k < rounds; k++) {
        const delay = k * spacing
        if (recorded) {
          const buffer = this.bank?.pick('mg') ?? recorded
          VehicleSounds.sample(ctx, out, t, buffer, rear ? 0.55 : 0.75, rear ? 1.25 : 1, this.distanceTo(at), delay)
          continue
        }
        VehicleSounds.burst(ctx, out, t, 'bandpass', rear ? 3400 : 2600, 700, 0.06, rear ? 0.45 : 0.55, delay)
        VehicleSounds.ping(ctx, out, t, 'sine', rear ? 220 : 170, 60, 0.05, rear ? 0.35 : 0.5, delay)
      }
    })
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
    this.candidates.length = 0
  }
}
