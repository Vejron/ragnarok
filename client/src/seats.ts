/**
 * What sitting in each seat of a vehicle means on this client: where the camera goes, what the HUD
 * calls the seat and shows, what E says to board it, and what the seat aims. Each kind's seats, in seat
 * order, are its `ride` in `kinds/`, so a new vehicle is its rows there rather than a branch in the
 * camera, the HUD and the prompt. What a seat does in the game (who drives, who fires what) is the
 * server's, in `VehicleDef`.
 */

import type { HudStats } from '@/hud'
import { kindView } from '@/kinds'

/** How the camera follows from a seat (`cameras.ts`). */
export type Rig =
  /** Behind the vehicle, easing round behind it when the mouse rests. */
  | { name: 'drive' }
  /** Behind a mounted gun along its line, its travel kept in the hull's frame. */
  | { name: 'gun'; hull: readonly [yawLimit: number, pitchMin: number, pitchMax: number] }
  /** Behind a gun on a turret's roof, its travel kept within `limit` either side of the turret. */
  | { name: 'pintle'; limit: number; pitch: readonly [number, number] }
  /** Through a drone's camera, flown from afar within `link` metres; a loitering drone also has one under its belly. */
  | { name: 'fpv'; pitch: readonly [number, number]; link: number; belly?: true }
  /** Behind a plane, following the mouse's aim at an easy pace. */
  | { name: 'aviator' }
  /** Behind a helicopter, looking where the mouse looks, which its pedals turn the nose to meet. */
  | { name: 'pilot' }
  /**
   * Behind a gun fixed in the hull, which the hull aims and which only elevates: a Strv S's, or with
   * `pitch` and the camera's `eye` (back, up) a Stridsbåt 90's machine guns.
   */
  | { name: 'tanker'; pitch?: readonly [number, number]; eye?: readonly [back: number, up: number] }
  /** Behind a walker's guns or a tupp's eyes, which turn within its torso's or its head's travel. */
  | { name: 'walker' }
  /** Behind and above a Titan's shoulders, looking along the look, which its fists follow. */
  | { name: 'titan' }
  /** Behind a field mortar, looking where the mouse sets its bearing. */
  | { name: 'mortar' }
  /** Over the shoulder from where they sit, as on foot: a car's passenger, who fires their own guns. */
  | { name: 'passenger' }
  /** Through a submarine's periscope, from its head over the sail, turning all the way round. */
  | { name: 'periscope' }

export interface SeatView {
  rig: Rig
  /** What the HUD calls it, which picks its hints. */
  role: NonNullable<HudStats['seat']>
  /** What E says to board into it; none for a seat nobody boards (a drone is flown from afar). */
  board?: string
  /** The chase camera's lean, kick and wider view at speed. */
  chase?: true
  /** Holding aim looks down the gun's sight. */
  sight?: true
  /** Its occupant stands rather than sits: a technical's gunner, on their feet in the bed behind the gun. */
  stand?: true
  /**
   * Its occupant works a collective lever (`CollectiveLever`): jump raises it and run lowers it, and it
   * stays where it is let go. The input carries it as `collective`.
   */
  lever?: true
  /** It is inside a closed hull, cabin or cockpit: the world outside is heard muffled from it (`setMuffle`). */
  enclosed?: true
  /** The gauges the HUD shows from it. */
  gauges?: readonly ('air' | 'boost' | 'eggs' | 'cockpit' | 'range' | 'ammo' | 'pintle' | 'heat' | 'wind' | 'tubes' | 'rotor')[]
  /**
   * With this seat empty, its occupant works that seat's gun: a driver alone in a turreted hull aims and
   * fires the turret, and sees and is told what its gunner would be (`MountDef::stand_in`).
   */
  standIn?: number
  /** The arc drawn ahead of what it fires: a mortar's bomb, or where a plane's bomb would land. */
  arc?: 'mortar' | 'bombs'
}

export interface RideView {
  /** How far behind it the camera sits, metres. */
  distance: number
  /** Ticks between shots of the gun its crew fires, for the HUD's reload ring; null for guns that never reload. */
  reload: number | null
  /**
   * Ticks a gun loaded all at once takes to load again after its last round (`Refill::Reload`), for the
   * reload ring while the turret shows `TURRET_EMPTY`.
   */
  refill?: number
  seats: readonly SeatView[]
  /** An aeroplane's stall speed, m/s, for its pilot's stall warning; the triplane's when left out. */
  stall?: number
  /** Rotors that flatten the grass under them: gone past `reach` metres up, over `radius` metres at the ground and wider higher up. */
  downwash?: { reach: number; radius: number }
}

/** How a vehicle of `kind` is presented. Unknown kinds are jeeps. */
export const rideView = (kind: number): RideView => kindView(kind).ride

/** How seat `index` of a vehicle of `kind` is presented: its own row, or the last seat's. */
export function seatView(kind: number, index: number): SeatView {
  const seats = rideView(kind).seats
  return seats[Math.min(index, seats.length - 1)]!
}

/**
 * How seat `index` of a vehicle of `kind` is presented with its crew as it is (`seats`, a rider's id per
 * seat, 0 for an empty one): a driver alone in a turreted hull works the gun, from its gunner's view.
 */
export function crewedSeatView(kind: number, index: number, seats: readonly number[]): SeatView {
  const own = seatView(kind, index)
  if (own.standIn === undefined || (seats[own.standIn] ?? 0) !== 0) return own
  const gun = seatView(kind, own.standIn)
  return { ...gun, role: 'tankDriver', board: own.board, gauges: [...(own.gauges ?? []), ...(gun.gauges ?? [])] }
}
