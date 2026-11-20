/**
 * How this client presents a vehicle kind: one `VehicleKind` per kind, in `kinds/<kind>.ts`, holding
 * its model, how it is ridden (camera, HUD, prompt), how it sounds and what it leaves on the ground.
 * What a kind is in the game (health, seats, guns, physics) is its Rust definition, which reaches the
 * client as `vehicleDef`.
 */

import type { VehicleModel } from '@/render/car'
import type { RoostStyle } from '@/render/snowroost'
import type { RideView, SeatView } from '@/seats'
import type { Engine } from '@/sim/engine'
import type { WheelLayout } from '@/sim/slip'

export interface KindSound {
  /** The filter on the noise its blades or tracks chop. */
  chop: { type: BiquadFilterType; hz: number }
  /** Its engine (`Motor`), how its revs follow the throttle and how it is heard (`Revs`, `mixOf`). */
  engine: Engine
}

export interface KindFx {
  /** Its tyres, which mark, dust and spray where they touch. None for a rotor, which blows dust up when low. */
  wheels: WheelLayout | null
  /** The wheels from this index on push it, and so spin on a hard launch; none for a plane, which its propeller pulls. */
  drivenFrom: number | null
  /** What comes out of it while it is driven: exhaust puffs, a rotary's burnt castor oil, a missile's flame and smoke, or nothing for an electric motor. */
  exhaust: 'pipe' | 'castor' | 'rocket' | null
  /** Backfires when the driver lifts off at speed. */
  backfire?: true
  /** Where its engine is, in the hull's frame: where a hurt one smokes from. */
  bay: readonly [number, number, number]
  /** A track that throws up snow when it digs in, in a winter world (`SnowRoost`). */
  roost?: Roost
  /** A hull that floats, with no wheels: it carves a wake through open water as it goes. */
  floats?: true
  /**
   * What drives a boat through the water: waterjets in its transom, which throw a rooster tail of spray
   * up behind it under way, or an outboard's propeller, which throws a low fan of spray back and out.
   * Either leaves a white wash behind it on the water.
   */
  drive?: 'jets' | 'outboard'
  /**
   * How far below its middle a seaplane's pontoons reach, metres: what meets the water is their
   * undersides, not its hull's, and afloat on them it is down as much as on its wheels.
   */
  keel?: number
}

/** Where a track throws its snow and how hard it must pull to: past the tail of each track, in the hull's frame. */
export interface Roost extends RoostStyle {
  /** (x, z) in the hull's frame, behind the end of each track. */
  at: readonly (readonly [number, number])[]
  /** Acceleration along its nose where the snow starts flying, and where it is at its thickest, m/s². */
  accel: readonly [number, number]
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

/** The chopped noise's filter for a kind with no blades or tracks to speak of. */
export const PLAIN_CHOP = { type: 'lowpass', hz: 260 } as const
/** The sound of a kind with no engine: legs, a towed gun, a ship heard by her guns. */
export const SILENT: KindSound = { chop: PLAIN_CHOP, engine: { name: 'none' } }
