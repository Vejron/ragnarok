import { KIND_HELI, KIND_MORTAR, KIND_PLANE, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_WALKER } from '@/net/codec'
import { JEEP_MAX_SPEED, SPORTS_MAX_SPEED, STRV_MAX_SPEED, TANK_MAX_SPEED } from './consts'
import { SURFACE_ASPHALT, SURFACE_GRASS, SURFACE_WATER } from './surface'

/**
 * What a vehicle sounds like, as numbers: pure functions from how it moves to how loud and how high
 * each layer of its sound is. `vehicleaudio.ts` turns them into Web Audio nodes.
 */

/** Where each gear starts, as a fraction of top speed. A tank has the one. */
const SHIFTS: Record<number, readonly number[]> = {
  [KIND_SPORTS]: [0, 0.16, 0.32, 0.5, 0.72],
  [KIND_TANK]: [0],
  [KIND_STRV]: [0],
}
const JEEP_SHIFTS = [0, 0.22, 0.45, 0.7]

/** Fundamental of the engine at idle revs, Hz, and how far it climbs to the redline. */
const BASE_HZ: Record<number, number> = { [KIND_SPORTS]: 62, [KIND_TANK]: 30, [KIND_STRV]: 36, [KIND_HELI]: 40, [KIND_PLANE]: 85 }
const JEEP_HZ = 44
/** Revs just after a shift, as a fraction of the range (idle is 0.25). */
const AFTER_SHIFT = 0.5

function topSpeed(kind: number) {
  return kind === KIND_SPORTS ? SPORTS_MAX_SPEED : kind === KIND_TANK ? TANK_MAX_SPEED : kind === KIND_STRV ? STRV_MAX_SPEED : JEEP_MAX_SPEED
}

export interface Gearing {
  gear: number
  /** Engine speed, 0 idle to 1 redline. */
  revs: number
}

/**
 * The gear a car is in at `speed` m/s (either direction) and the engine speed in it. Revs climb
 * within a gear and fall back a step at each shift, which is what makes acceleration sound quick.
 */
export function gearing(kind: number, speed: number): Gearing {
  const shifts = SHIFTS[kind] ?? JEEP_SHIFTS
  const fraction = Math.min(1, Math.abs(speed) / topSpeed(kind))
  let gear = 0
  while (gear + 1 < shifts.length && fraction >= shifts[gear + 1]!) gear++
  const low = shifts[gear]!
  const high = shifts[gear + 1] ?? 1
  const along = Math.min(1, Math.max(0, (fraction - low) / (high - low)))
  const start = gear === 0 ? 0.25 : AFTER_SHIFT
  return { gear, revs: start + (1 - start) * along }
}

/** How a vehicle is moving and what state it is in; everything the sound depends on. */
export interface SoundInput {
  kind: number
  /** Speed along the nose, m/s, signed. */
  speed: number
  /** Acceleration along the nose, m/s². */
  accel: number
  /** Rate of climb, m/s. */
  climb: number
  /** Someone drives it and it is not wrecked. */
  powered: boolean
  hp: number
  maxHp: number
  /** Worst tyre slip, 0..1. */
  slip: number
  surface: number
  grounded: boolean
  /** Per-wheel damage (FL, FR, RL, RR), or null while the vehicle is whole. */
  wheels: readonly number[] | null
  /** A plane's blip switch is cutting the engine. */
  cut?: boolean
}

export interface SoundMix {
  /** Fundamental, Hz, before Doppler. */
  pitch: number
  /** Level of the engine layer, 0..1. */
  engine: number
  /** Cutoff of the engine's low-pass, Hz. */
  tone: number
  /** Beats per second of the blades or the tracks, and how loud they are. */
  chop: number
  chopLevel: number
  /** Chance per second the engine drops out for an instant. */
  stutter: number
  /** Tyre squeal level and centre frequency. */
  squeal: number
  squealHz: number
  /** Rolling noise of the ground under the tyres, level and cutoff. */
  ground: number
  groundHz: number
  /** Steam-like hiss of a nearly dead engine. */
  hiss: number
  /** Rattle of a bent wheel or the grind of a lost one. */
  rattle: number
  /** A wreck is burning. */
  fire: boolean
  /** A plane's ignition is cut: the engine falls silent but for the odd blip. */
  cut: boolean
}

const BASE_LEVEL: Record<number, number> = { [KIND_SPORTS]: 0.2, [KIND_TANK]: 0.3, [KIND_STRV]: 0.3, [KIND_HELI]: 0.3, [KIND_PLANE]: 0.28 }
/** A plane's top speed, m/s, for the pitch of its engine and the rush of the wind. */
const PLANE_TOP = 45
/** A propeller's two blades pass this many times per firing stroke of the nine-cylinder rotary. */
const PROP_BEAT = 2 / 4.5

const clamp = (value: number, low = 0, high = 1) => Math.min(high, Math.max(low, value))

/** Squeal level for `slip` on `surface` at `speed`: loud on tarmac, a scrub on dirt, nothing in water. */
export function squealGain(slip: number, surface: number, speed: number): number {
  if (surface === SURFACE_WATER) return 0
  const hard = clamp((slip - 0.15) / 0.6)
  const factor = surface === SURFACE_ASPHALT ? 1 : surface === SURFACE_GRASS ? 0.15 : 0.35
  return hard * factor * clamp(Math.abs(speed) / 6)
}

/** Level of the ground's rolling noise: gravel crunch off-road, a faint roar on tarmac, a swish in water. */
export function groundGain(speed: number, surface: number): number {
  const s = Math.abs(speed)
  if (surface === SURFACE_ASPHALT) return 0.1 * clamp(s / 40)
  if (surface === SURFACE_WATER) return 0.3 * clamp(s / 10)
  return 0.7 * clamp(s / 20)
}

/** Whether a drop through the air of `fall` m/s is worth a thud, and how loud, 0..1. */
export function landingThud(fall: number): number {
  return fall < 3 ? 0 : clamp((fall - 3) / 12) * 0.8 + 0.2
}

/** 0 bump, 1 crunch, 2 smash: how a crash at `speed` m/s sounds. */
export function crashSize(speed: number): 0 | 1 | 2 {
  return speed < 8 ? 0 : speed < 16 ? 1 : 2
}

export function mixOf(input: SoundInput): SoundMix {
  const { kind, speed, accel, climb, powered, hp, maxHp } = input
  const alive = hp > 0
  const quiet: SoundMix = {
    pitch: 40, engine: 0, tone: 400, chop: 0, chopLevel: 0, stutter: 0, squeal: 0, squealHz: 1200, ground: 0, groundHz: 600, hiss: 0, rattle: 0, fire: !alive, cut: false,
  }
  if (!alive) return quiet
  // A walker has no engine to hum, no tyres on the ground: its feet are heard by the footfalls. A mortar is towed, with no engine at all.
  if (kind === KIND_WALKER || kind === KIND_MORTAR) return quiet
  const lost = input.wheels ? input.wheels.filter((wear) => wear >= 255).length : 0
  const bent = input.wheels ? input.wheels.filter((wear) => wear >= 128 && wear < 255).length : 0
  const mix: SoundMix = { ...quiet }
  const rolling = input.grounded ? clamp(Math.abs(speed) / 10) : 0
  mix.rattle = Math.min(0.8, lost * 0.4 * rolling + bent * 0.1 * rolling)
  if (kind !== KIND_HELI) {
    mix.squeal = input.grounded ? squealGain(input.slip, input.surface, speed) : 0
    mix.squealHz = 900 + 700 * clamp(input.slip) + 4 * Math.abs(speed)
    mix.ground = input.grounded ? groundGain(speed, input.surface) : 0
    mix.groundHz = input.surface === SURFACE_ASPHALT ? 400 : 900 + 40 * Math.abs(speed)
  }
  if (kind === KIND_PLANE && !input.grounded) {
    // In the air the wind in the wires takes the place of the ground under the wheels.
    mix.ground = 0.6 * clamp(Math.abs(speed) / PLANE_TOP) ** 1.5
    mix.groundHz = 250 + 30 * Math.abs(speed)
  }
  if (!powered) return mix

  const health = maxHp > 0 ? hp / maxHp : 1
  if (kind === KIND_PLANE) {
    // A rotary has no gears: its note rises with the airspeed and the throttle, and sags when the blip
    // switch cuts it and the propeller windmills.
    const cut = input.cut === true
    const air = clamp(Math.abs(speed) / PLANE_TOP)
    const load = cut ? 0 : clamp(0.55 + accel / 6 + climb / 20)
    mix.pitch = BASE_HZ[KIND_PLANE]! * (0.7 + 0.35 * air + 0.2 * load) * (cut ? 0.85 : 1)
    mix.chop = mix.pitch * PROP_BEAT
    mix.chopLevel = 0.35
    mix.engine = BASE_LEVEL[KIND_PLANE]! * (0.5 + 0.5 * load)
    mix.tone = 900 + 1800 * load
    mix.cut = cut
  } else if (kind === KIND_HELI) {
    const load = clamp(0.35 + Math.abs(climb) / 6 + Math.abs(speed) / 40)
    mix.pitch = BASE_HZ[KIND_HELI]! * (0.85 + 0.5 * load)
    mix.chop = 9 + 6 * load
    mix.chopLevel = 1
    mix.engine = BASE_LEVEL[KIND_HELI]! * (0.5 + 0.5 * load)
    mix.tone = 500 + 800 * load
  } else {
    const { revs } = gearing(kind, speed)
    // Gas held when accelerating or holding speed, off when braking hard or lifting.
    const load = accel < -2 ? 0.05 : clamp(0.3 + accel / 4)
    const base = BASE_HZ[kind] ?? JEEP_HZ
    mix.pitch = base * (0.7 + 1.9 * revs)
    mix.engine = (BASE_LEVEL[kind] ?? 0.22) * (0.4 + 0.6 * load)
    mix.tone = 350 + 1500 * load + 700 * revs
    if (kind === KIND_TANK || kind === KIND_STRV) {
      mix.chop = 5 + 1.5 * Math.abs(speed)
      mix.chopLevel = 0.9 * clamp(Math.abs(speed) / 4)
    }
  }
  if (health < 0.4) mix.stutter = ((0.4 - health) / 0.4) * 6
  if (health < 0.15) mix.hiss = 0.12 * (1 - health / 0.15)
  return mix
}
