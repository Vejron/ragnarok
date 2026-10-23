/**
 * How this client presents a vehicle kind: one `VehicleKind` per kind, in `kinds/<kind>.ts`, holding
 * its model, how it is ridden (camera, HUD, prompt), how it sounds and what it leaves on the ground.
 * What a kind is in the game (health, seats, guns, physics) is its Rust definition, which reaches the
 * client as `vehicleDef`.
 */

import type { VehicleModel } from '@/render/car'
import type { RideView, SeatView } from '@/seats'
import type { Engine } from '@/sim/engine'
import type { WheelLayout } from '@/sim/slip'

/** One of an engine's oscillators: its wave, its multiple of the fundamental and its level. */
export type Voice = readonly [type: OscillatorType, ratio: number, level: number]

export interface KindSound {
  voices: readonly Voice[]
  /** Resonance of the engine's tone filter: a rotary's buzz has a nasal peak on its harmonics. */
  q: number
  /** The filter on the noise its blades or tracks chop. */
  chop: { type: BiquadFilterType; hz: number }
  /** How its engine and wheels are heard as it moves (`mixOf`). */
  engine: Engine
}

export interface KindFx {
  /** Its tyres, which mark, dust and spray where they touch. None for a rotor, which blows dust up when low. */
  wheels: WheelLayout | null
  /** The wheels from this index on push it, and so spin on a hard launch; none for a plane, which its propeller pulls. */
  drivenFrom: number | null
  /** What comes out of it while it is driven: exhaust puffs, a rotary's burnt castor oil, or nothing for an electric motor. */
  exhaust: 'pipe' | 'castor' | null
  /** Backfires when the driver lifts off at speed. */
  backfire?: true
  /** Where its engine is, in the hull's frame: where a hurt one smokes from. */
  bay: readonly [number, number, number]
}

export interface VehicleKind {
  kind: number
  /** Builds the model it is drawn as. */
  model: (id: number) => VehicleModel
  /** Fetches what its models are built from, before any is made. */
  load?: () => Promise<unknown>
  ride: RideView
  sound: KindSound
  fx: KindFx
}

/** Behind the vehicle, the chase camera's. */
export const DRIVE = { name: 'drive' } as const
/** A car's passenger, who fires their own guns from over their shoulder. */
export const RIDE_ALONG: SeatView = { rig: { name: 'passenger' }, role: 'passenger', board: 'E: ride along' }

/** The engine sound most kinds start from: a petrol engine's sawtooth and its sub-octave. */
export const PETROL: readonly Voice[] = [['sawtooth', 1, 0.5], ['square', 0.5, 0.35], ['sawtooth', 1.5, 0.15]]
/** The tone filter's resonance, and the chopped noise's filter, for a kind with no blades or tracks to speak of. */
export const PLAIN = { q: 1.2, chop: { type: 'lowpass', hz: 260 } } as const
