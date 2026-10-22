// What the client's per-kind if-chains said before each kind's presentation became its file in
// `kinds/`, kept here to hold the files to them (`kinds.spec.ts`).

import { KIND_DRONE, KIND_HELI, KIND_HEN, KIND_LOITER, KIND_MORTAR, KIND_PLANE, KIND_ROADSTER, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_WALKER } from '@/net/codec'
import { JEEP_MAX_SPEED, ROADSTER_MAX_SPEED, SPORTS_MAX_SPEED, STRV_MAX_SPEED, TANK_MAX_SPEED } from '@/sim/consts'
import { groundGain, squealGain, type SoundInput, type SoundMix } from '@/sim/engine'
import { JEEP_WHEELS, PLANE_WHEELS, SPORTS_WHEELS, TRACK_WHEELS } from '@/sim/slip'
import { SURFACE_ASPHALT } from '@/sim/surface'
import { isTracked } from '@/sim/vehicles'

/** Where each gear starts, as a fraction of top speed. A tank has the one, and so has the Roadster's electric motor. */
const SHIFTS: Record<number, readonly number[]> = {
  [KIND_SPORTS]: [0, 0.16, 0.32, 0.5, 0.72],
  [KIND_ROADSTER]: [0],
  [KIND_TANK]: [0],
  [KIND_STRV]: [0],
}
const JEEP_SHIFTS = [0, 0.22, 0.45, 0.7]

/** Fundamental of the engine at idle revs, Hz, and how far it climbs to the redline. */
const BASE_HZ: Record<number, number> = { [KIND_SPORTS]: 62, [KIND_ROADSTER]: 150, [KIND_TANK]: 30, [KIND_STRV]: 36, [KIND_HELI]: 40, [KIND_PLANE]: 85 }
const JEEP_HZ = 44
/** Revs just after a shift, as a fraction of the range (idle is 0.25). */
const AFTER_SHIFT = 0.5

function topSpeed(kind: number) {
  return kind === KIND_SPORTS ? SPORTS_MAX_SPEED : kind === KIND_ROADSTER ? ROADSTER_MAX_SPEED : kind === KIND_TANK ? TANK_MAX_SPEED : kind === KIND_STRV ? STRV_MAX_SPEED : JEEP_MAX_SPEED
}

function gearing(kind: number, speed: number) {
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

/** A drone's props at hover, Hz. */
const DRONE_HZ = 190
/** A loitering drone's two-stroke at cruise, Hz: the moped buzz. */
const LOITER_HZ = 80
/** Its top speed, m/s, for the pitch. */
const LOITER_TOP = 45
const BASE_LEVEL: Record<number, number> = { [KIND_SPORTS]: 0.2, [KIND_ROADSTER]: 0.1, [KIND_TANK]: 0.3, [KIND_STRV]: 0.3, [KIND_HELI]: 0.3, [KIND_PLANE]: 0.28 }
/** A plane's top speed, m/s, for the pitch of its engine and the rush of the wind. */
const PLANE_TOP = 45
/** A propeller's two blades pass this many times per firing stroke of the nine-cylinder rotary. */
const PROP_BEAT = 2 / 4.5

const clamp = (value: number, low = 0, high = 1) => Math.min(high, Math.max(low, value))

/** `mixOf` as it was. */
export function mix(input: SoundInput): SoundMix {
  const { kind, speed, accel, climb, powered, hp, maxHp } = input
  const alive = hp > 0
  const quiet: SoundMix = {
    pitch: 40, engine: 0, tone: 400, chop: 0, chopLevel: 0, stutter: 0, squeal: 0, squealHz: 1200, ground: 0, groundHz: 600, hiss: 0, rattle: 0, fire: !alive, cut: false,
  }
  if (!alive) return quiet
  // A walker or a hen has no engine to hum, no tyres on the ground: its feet are heard by the footfalls. A mortar is towed, with no engine at all.
  if (kind === KIND_WALKER || kind === KIND_HEN || kind === KIND_MORTAR) return quiet
  const lost = input.wheels ? input.wheels.filter((wear) => wear >= 255).length : 0
  const bent = input.wheels ? input.wheels.filter((wear) => wear >= 128 && wear < 255).length : 0
  const mix: SoundMix = { ...quiet }
  const rolling = input.grounded ? clamp(Math.abs(speed) / 10) : 0
  mix.rattle = Math.min(0.8, lost * 0.4 * rolling + bent * 0.1 * rolling)
  if (kind !== KIND_HELI && kind !== KIND_DRONE && kind !== KIND_LOITER) {
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
  } else if (kind === KIND_DRONE) {
    // Four small props: a thin, high whine that climbs with the work they do.
    const load = clamp(0.3 + Math.abs(climb) / 10 + Math.abs(speed) / 30)
    mix.pitch = DRONE_HZ * (0.85 + 0.6 * load)
    mix.engine = 0.14 * (0.6 + 0.4 * load)
    mix.tone = 2200 + 2000 * load
  } else if (kind === KIND_LOITER) {
    // One small engine flat out all the time: its note rises a little with the airspeed and the climb.
    const load = clamp(0.6 + climb / 15 + accel / 10)
    mix.pitch = LOITER_HZ * (0.8 + 0.45 * clamp(Math.abs(speed) / LOITER_TOP) + 0.15 * load)
    mix.engine = 0.3 * (0.7 + 0.3 * load)
    mix.tone = 1200 + 1200 * load
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
  // Cold gas leaving the nozzles is a loud, broad hiss: louder with both sets open.
  if (input.jets) mix.hiss = Math.max(mix.hiss, input.jets === 3 ? 0.5 : 0.35)
  return mix
}

/** The engine's oscillators, its tone's resonance and the chopped noise's filter, as `Voice` built them. */
export const voices = (kind: number): [OscillatorType, number, number][] =>
  kind === KIND_HELI ? [['sawtooth', 1, 0.5], ['square', 0.5, 0.3]]
  : kind === KIND_PLANE ? [['square', 1, 0.4], ['sawtooth', 2, 0.22], ['square', 0.5, 0.3], ['sawtooth', 4.02, 0.08]]
  : kind === KIND_TANK ? [['sawtooth', 1, 0.5], ['square', 0.5, 0.4], ['sawtooth', 1.51, 0.12]]
  : kind === KIND_STRV ? [['sawtooth', 1, 0.5], ['square', 0.5, 0.3], ['sawtooth', 2.01, 0.15]]
  : kind === KIND_SPORTS ? [['sawtooth', 1, 0.42], ['square', 0.5, 0.25], ['triangle', 3.02, 0.2]]
  : kind === KIND_ROADSTER ? [['sine', 1, 0.5], ['triangle', 2, 0.25], ['sine', 7.03, 0.06]]
  : kind === KIND_DRONE ? [['sawtooth', 1, 0.35], ['sawtooth', 1.013, 0.3], ['square', 2.01, 0.12]]
  : kind === KIND_LOITER ? [['sawtooth', 1, 0.5], ['sawtooth', 1.021, 0.35], ['square', 2, 0.2], ['square', 0.5, 0.25]]
  : [['sawtooth', 1, 0.5], ['square', 0.5, 0.35], ['sawtooth', 1.5, 0.15]]
export const q = (kind: number) => (kind === KIND_PLANE ? 2.5 : 1.2)
export const chop = (kind: number) => ({
  type: kind === KIND_TANK || kind === KIND_STRV || kind === KIND_PLANE ? 'bandpass' : 'lowpass',
  hz: kind === KIND_TANK || kind === KIND_STRV ? 650 : kind === KIND_PLANE ? 220 : 260,
})

/** `wheelLayout`. */
export function wheels(kind: number) {
  if (kind === KIND_HELI) return null
  if (kind === KIND_PLANE) return PLANE_WHEELS
  if (isTracked(kind)) return TRACK_WHEELS
  if (kind === KIND_SPORTS || kind === KIND_ROADSTER) return SPORTS_WHEELS
  return JEEP_WHEELS
}
/** `VehicleFx`: which wheels are driven, what comes out of it, and the backfire. */
export const driven = (kind: number, wheel: number) => kind !== KIND_PLANE && (kind !== KIND_SPORTS || wheel >= 2)
export const exhaust = (kind: number) => (kind === KIND_HELI ? null : kind === KIND_PLANE ? 'castor' : kind === KIND_ROADSTER ? null : 'pipe')
export const backfire = (kind: number) => kind === KIND_SPORTS
/** `engineBay`. */
export function bay(kind: number): [number, number, number] {
  if (kind === KIND_TANK) return [0, 1.3, 1.6]
  if (kind === KIND_STRV) return [0, 1.1, 1.8]
  if (kind === KIND_HELI) return [0, 1.6, 0.4]
  if (kind === KIND_PLANE) return [0, 0.5, -1.8]
  return [0, 0.75, -1.35]
}
