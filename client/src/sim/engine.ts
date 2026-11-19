import type { SynthSpec } from '@/sound/enginesynth'
import type { Grit } from '@/sound/gritsynth'
import { rolling, type Ground } from './groundsound'

/**
 * What a vehicle sounds like, as numbers: the revs its engine turns at (`Revs`) and pure functions
 * from how it moves to how loud and how high each other layer of its sound is. `vehicleaudio.ts`
 * turns them into Web Audio nodes, and `sound/enginesynth.ts` makes the engine's own sound.
 */

/** How a kind's engine is heard (each kind's is in `kinds/`). */
export type Engine = { name: 'none' } | Motor

/**
 * An engine: how its revs follow the throttle and the wheels (`name`), and how it sounds (`synth`).
 * - `gears`: pistons through an automatic gearbox, `shifts` being where each gear starts as a fraction of `top`.
 * - `cvt`: a belt drive (snowmobile, sled): the revs leap to where it pulls and hold there as the speed builds.
 * - `electric`: a motor on one gear, its revs in step with the wheels and silent at a standstill.
 * - `marine`: a boat's engine or motor turning its propeller or waterjet: the throttle sets its revs, the
 *   speed through the water barely, so a hull rocking on the waves does not wobble its note.
 * - `rotary`: a plane's engine and propeller, the revs rising with the throttle and the airspeed.
 * - `rotor`: a helicopter's governed turbine, at nearly the same speed whatever the collective asks.
 * - `props`: a drone's four motors, working harder to climb and go.
 * - `flat`: flat out all the time (a loitering drone's two-stroke, a missile's motor).
 */
export interface Motor {
  name: 'gears' | 'cvt' | 'electric' | 'marine' | 'rotary' | 'rotor' | 'props' | 'flat'
  synth: SynthSpec
  /** Revs at idle and at the redline, rpm. */
  idle: number
  redline: number
  /** Seconds the engine takes to rev from idle to the redline out of gear: a tank's heavy flywheel is slow, a two-stroke snaps. */
  spin: number
  /** Where each gear starts as a fraction of `top` (`gears` only; `[0]` for one). */
  shifts: readonly number[]
  /** Top speed, m/s. */
  top: number
  /** Loudness of the engine, 0..1. */
  level: number
  /** It runs on tracks, which clatter. */
  tracks?: true
}

/** Revs just after an upshift, as a share of the range above idle. */
const AFTER_SHIFT = 0.45
/** Seconds the throttle is lifted for a shift. */
const SHIFT_TIME = 0.22
/** Below this share of a car's first gear the clutch still slips: the revs are the throttle's, not the wheels'. */
const LAUNCH = 0.35
/** A tyre slipping this much spins free of the ground: the revs run away from the speed. */
const SPINNING = 0.45
/** How long the rev limiter cuts the sparks, and how often, seconds. */
const LIMITER_CUT = 0.04
const LIMITER_EVERY = 0.11

const clamp = (value: number, low = 0, high = 1) => Math.min(high, Math.max(low, value))

/** How a vehicle is moving and what state it is in; everything the sound depends on. */
export interface SoundInput {
  kind: number
  /** Speed along the nose, m/s, signed. */
  speed: number
  /** Acceleration along the nose, m/s². */
  accel: number
  /** Rate of climb, m/s. */
  climb: number
  /** How hard the driver works the engine, -1..1 (`Steer.throttle`; a helicopter's collective). */
  throttle: number
  /** Someone drives it and it is not wrecked. */
  powered: boolean
  hp: number
  maxHp: number
  /** Worst tyre slip, 0..1. */
  slip: number
  /** What its wheels or tracks touch, or the water a hull is in. */
  ground: Ground
  /** On its wheels or tracks, or floating: not flying. */
  grounded: boolean
  /** Wholly under the water: no waves to slap it, only the rush of the water past. */
  submerged?: boolean
  /** A tracked vehicle turning, rad/s about its own up axis. */
  yawRate: number
  /** Per-wheel damage (FL, FR, RL, RR), or null while the vehicle is whole. */
  wheels: readonly number[] | null
  /** A plane's blip switch is cutting the engine. */
  cut?: boolean
  /** A Roadster's `THRUST_*` nozzles firing: a roar of escaping air. */
  jets?: number
  /**
   * A helicopter's rotor speed, a share of its governed speed (`Thrust.air`): its blades beat with it,
   * the engine running or not, so it is heard spinning up, drooping, spinning down and autorotating.
   */
  rotor?: number
}

/** Where an engine's revs are: rpm, how hard it pulls (0..1), and whether it is between gears. */
export interface RevState {
  rpm: number
  load: number
  gear: number
}

/**
 * How the share of the engine a driver asks for, from their throttle while the vehicle goes `speed`
 * m/s: pulling the stick against the way it rolls is the brake, not the gas.
 */
export function gasOf(throttle: number, speed: number): number {
  if (Math.abs(speed) > 1 && throttle * speed < 0) return 0
  return clamp(Math.abs(throttle))
}

/**
 * One engine's revs, followed from frame to frame: the gearbox shifts, the clutch slips at a launch,
 * spinning wheels and a car in the air let it rev free, and the limiter bounces it off the redline.
 */
export class Revs {
  rpm = 0
  gear = 0
  private shifting = 0
  private limiter = 0

  /** Steps the revs `dt` seconds on for `motor` moving as `input` says. */
  step(motor: Motor, input: SoundInput, dt: number): RevState {
    const { idle, redline } = motor
    const range = redline - idle
    if (!input.powered || input.hp <= 0) {
      // Switched off it runs down.
      this.rpm *= Math.exp(-dt / 0.4)
      this.shifting = 0
      return { rpm: this.rpm, load: 0, gear: this.gear }
    }
    if (this.rpm < idle * 0.5 && motor.name !== 'electric') this.rpm = idle
    const frac = clamp(Math.abs(input.speed) / motor.top)
    let gas = gasOf(input.throttle, input.speed)
    // A load on the engine: a hill to climb.
    const climbing = input.grounded ? clamp(input.climb / 6, 0, 0.3) : 0
    let target = idle
    let follow = range / motor.spin
    let load = gas
    switch (motor.name) {
      case 'gears': {
        const { shifts } = motor
        const start = (g: number) => shifts[g] ?? 1
        if (this.gear + 1 < shifts.length && frac >= start(this.gear + 1)) {
          this.gear++
          this.shifting = SHIFT_TIME
        } else if (this.gear > 0 && frac < start(this.gear) - 0.03) {
          this.gear--
        }
        const low = start(this.gear)
        const along = clamp((frac - low) / (start(this.gear + 1) - low))
        const from = this.gear === 0 ? 0 : AFTER_SHIFT
        const coupled = idle + range * (from + (1 - from) * along)
        const free = idle + range * gas
        const launching = this.gear === 0 && frac < start(1) * LAUNCH
        const loose = !input.grounded || input.slip > SPINNING
        if (loose) target = Math.max(coupled, free)
        else if (launching) target = Math.max(coupled, idle + range * gas * 0.55)
        else {
          // In gear the wheels hold the engine to the road's speed.
          target = coupled
          follow *= 4
        }
        load = clamp(gas + climbing)
        break
      }
      case 'cvt': {
        // The belt grips once the revs are up, then pulls the sled along at the revs that give its power.
        const engage = idle + range * 0.5
        target = gas > 0.08 ? engage + (redline - engage) * clamp(0.45 * gas + 0.55 * frac * gas + (input.slip > SPINNING ? 0.3 : 0)) : idle + range * 0.1 * frac
        load = clamp(gas + climbing)
        break
      }
      case 'electric': {
        target = redline * frac
        if (!input.grounded || input.slip > SPINNING) target = Math.max(target, redline * gas * 0.6)
        follow *= 3
        break
      }
      case 'marine': {
        target = idle + range * (0.08 + 0.92 * gas) * (0.85 + 0.15 * frac)
        break
      }
      case 'rotary': {
        // A plane's stick is at cruise in the middle, full ahead forward, the blip switch back.
        gas = clamp(0.55 + 0.45 * input.throttle)
        const air = clamp(Math.abs(input.speed) / motor.top)
        target = idle + range * (0.2 + 0.5 * gas + 0.3 * air)
        load = input.cut ? 0 : clamp(gas + input.climb / 20)
        if (input.cut) target = idle + (target - idle) * 0.8
        break
      }
      case 'rotor': {
        // A governed turbine holds its speed; the collective changes how hard it works.
        load = clamp(0.45 + 0.4 * input.throttle + Math.abs(input.speed) / 60 + Math.max(0, input.climb) / 12)
        target = redline * (0.92 + 0.08 * load)
        break
      }
      case 'props': {
        load = clamp(0.3 + Math.abs(input.climb) / 10 + Math.abs(input.speed) / 30 + 0.2 * gas)
        target = idle + range * load
        follow *= 3
        break
      }
      case 'flat': {
        target = redline * (0.9 + 0.1 * frac)
        load = 1
        break
      }
    }
    target = Math.min(target, redline)
    if (this.shifting > 0) {
      // Off the throttle for the shift; the revs fall to the next gear's.
      this.shifting = Math.max(0, this.shifting - dt)
      load = 0
    }
    const step = follow * dt
    this.rpm += clamp(target - this.rpm, -step * 1.3, step * (0.35 + 0.65 * Math.max(gas, 0.3)))
    // Held against the redline, the limiter cuts the sparks in a stutter.
    if (gas > 0.6 && this.rpm >= redline * 0.985 && motor.name !== 'electric' && motor.name !== 'marine' && motor.name !== 'rotor' && motor.name !== 'flat') {
      this.limiter += dt
      if (this.limiter % LIMITER_EVERY < LIMITER_CUT) {
        load = 0
        this.rpm -= range * 0.03
      }
    } else this.limiter = 0
    return { rpm: this.rpm, load, gear: this.gear }
  }
}

export interface SoundMix {
  /** The engine's revs, rpm, and how hard it pulls, 0..1, for `EngineSynth`. */
  rpm: number
  load: number
  /** Level of the engine layer, 0..1. */
  engine: number
  /** Beats per second of the blades or the tracks, and how loud they are. */
  chop: number
  chopLevel: number
  /** The tone of a track's clatter as a share of the kind's (`KindSound.chop`). */
  chopTone: number
  /** Chance per second the engine drops out for an instant. */
  stutter: number
  /** Tyre squeal level and centre frequency. */
  squeal: number
  squealHz: number
  /** Rolling noise of the ground under the tyres, level and cutoff. */
  ground: number
  groundHz: number
  /** The crunch of what it rolls over. */
  grit: Grit
  /** Steam-like hiss of a nearly dead engine. */
  hiss: number
  /** Rattle of a bent wheel or the grind of a lost one. */
  rattle: number
  /** A wreck is burning. */
  fire: boolean
  /** A plane's ignition is cut: the engine falls silent but for the odd blip. */
  cut: boolean
}

/** Squeal level for `slip` on `ground` at `speed`: loud on tarmac, a scrub on gravel, nothing on ice or in water. */
export function squealGain(slip: number, ground: Ground, speed: number): number {
  const hard = clamp((slip - 0.15) / 0.6)
  return hard * rolling(ground, speed, slip, false).squeal * clamp(Math.abs(speed) / 6)
}

/** Whether a drop through the air of `fall` m/s is worth a thud, and how loud, 0..1. */
export function landingThud(fall: number): number {
  return fall < 3 ? 0 : clamp((fall - 3) / 12) * 0.8 + 0.2
}

/** 0 bump, 1 crunch, 2 smash: how a crash at `speed` m/s sounds. */
export function crashSize(speed: number): 0 | 1 | 2 {
  return speed < 8 ? 0 : speed < 16 ? 1 : 2
}

/** Whether a vehicle with `engine` rolls on wheels or tracks, which squeal and rumble on the ground. */
const rolls = (engine: Engine) => engine.name === 'gears' || engine.name === 'cvt' || engine.name === 'electric' || engine.name === 'rotary'

const QUIET_GRIT: Grit = { rate: 0, level: 0, hz: 1000, ring: 0.6, length: 4 }

/** What a vehicle with `engine` sounds like, moving as `input` says with its engine at `revs`. */
export function mixOf(input: SoundInput, engine: Engine, revs: RevState): SoundMix {
  const { speed, powered, hp, maxHp } = input
  const alive = hp > 0
  const quiet: SoundMix = {
    rpm: revs.rpm, load: 0, engine: 0, chop: 0, chopLevel: 0, chopTone: 1, stutter: 0, squeal: 0, squealHz: 1200, ground: 0, groundHz: 600, grit: QUIET_GRIT, hiss: 0, rattle: 0, fire: !alive, cut: false,
  }
  if (!alive) return quiet
  // Legs have no engine to hum and no tyres on the ground: their feet are heard by the footfalls.
  if (engine.name === 'none') return quiet
  const lost = input.wheels ? input.wheels.filter((wear) => wear >= 255).length : 0
  const bent = input.wheels ? input.wheels.filter((wear) => wear >= 128 && wear < 255).length : 0
  const mix: SoundMix = { ...quiet }
  const moving = Math.abs(speed)
  const rollingNow = input.grounded ? clamp(moving / 10) : 0
  mix.rattle = Math.min(0.8, lost * 0.4 * rollingNow + bent * 0.1 * rollingNow)
  const tracked = engine.tracks === true
  const under = rolling(input.ground, speed, input.slip, tracked)
  if (rolls(engine) && input.grounded) {
    mix.squeal = squealGain(input.slip, input.ground, speed)
    mix.squealHz = 900 + 700 * clamp(input.slip) + 4 * moving
    mix.ground = under.roll
    mix.groundHz = under.rollHz
    mix.grit = under.grit
  }
  // A hull in the water washes and slaps, however it is driven.
  if (!rolls(engine) && input.ground === 'water') {
    mix.ground = under.roll
    mix.groundHz = under.rollHz
    mix.grit = under.grit
  }
  if (input.submerged) {
    // Under the surface there are no waves to slap it: the water rushes past, low and dull.
    mix.squeal = 0
    mix.grit = QUIET_GRIT
    mix.ground = 0.25 * clamp(moving / 8)
    mix.groundHz = 180 + 10 * moving
  }
  if (tracked && input.grounded) {
    // The links strike the sprockets and the road wheels, in time with the speed; a track turning on
    // the spot squeals as it is dragged sideways.
    mix.chop = 5 + 1.5 * moving
    mix.chopLevel = 0.9 * clamp(moving / 4) * under.clank
    mix.chopTone = under.clankHz
    if (moving < 3 && Math.abs(input.yawRate) > 0.25 && input.ground !== 'water') {
      mix.squeal = Math.max(mix.squeal, 0.35 * clamp((Math.abs(input.yawRate) - 0.25) / 0.5) * under.squeal + 0.08)
      mix.squealHz = 1900
    }
  }
  if (engine.name === 'rotary' && !input.grounded) {
    // In the air the wind in the wires takes the place of the ground under the wheels.
    mix.ground = 0.6 * clamp(moving / engine.top) ** 1.5
    mix.groundHz = 250 + 30 * moving
    mix.grit = QUIET_GRIT
  }
  if (engine.name === 'rotor' && input.rotor !== undefined) {
    // The blades beat as fast as they turn, and harder the more lift they make.
    mix.chop = input.rotor * (9 + 6 * (powered ? revs.load : 0.3))
    mix.chopLevel = clamp(input.rotor * 1.2)
    mix.chopTone = 1
  }
  if (!powered) return mix

  mix.rpm = revs.rpm
  mix.load = revs.load
  mix.engine = engine.level
  mix.cut = input.cut === true && engine.name === 'rotary'
  if (engine.name === 'rotor') {
    // The blades beat with the rotor's speed, or failing that the turbine's.
    mix.chop = (input.rotor ?? revs.rpm / engine.redline) * (9 + 6 * revs.load)
    mix.chopLevel = 1
    mix.chopTone = 1
  }
  if (engine.name === 'rotary') {
    // The propeller's two blades pass twice a turn.
    mix.chop = (2 * revs.rpm) / 60
    mix.chopLevel = 0.3
    mix.chopTone = 1
  }
  const health = maxHp > 0 ? hp / maxHp : 1
  if (health < 0.4) mix.stutter = ((0.4 - health) / 0.4) * 6
  if (health < 0.15) mix.hiss = 0.12 * (1 - health / 0.15)
  // Cold gas leaving the nozzles is a loud, broad hiss: louder with both sets open.
  if (input.jets) mix.hiss = Math.max(mix.hiss, input.jets === 3 ? 0.5 : 0.35)
  return mix
}
