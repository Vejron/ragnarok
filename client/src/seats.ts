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
  /** Behind a mounted gun along its line, its travel kept in the hull's frame (`hull`) or about the vertical (`level`). */
  | { name: 'gun'; hull: readonly [yawLimit: number, pitchMin: number, pitchMax: number] }
  | { name: 'gun'; level: readonly [pitchMin: number, pitchMax: number] }
  /** Through a drone's camera, flown from afar within `link` metres; a loitering drone also has one under its belly. */
  | { name: 'fpv'; pitch: readonly [number, number]; link: number; belly?: true }
  /** Behind a plane, following the mouse's aim at an easy pace. */
  | { name: 'aviator' }
  /** Behind a Strv S's gun, which the hull aims. */
  | { name: 'tanker' }
  /** Behind a walker's guns, which turn within its torso's travel. */
  | { name: 'walker' }
  /** Behind a field mortar, looking where the mouse sets its bearing. */
  | { name: 'mortar' }
  /** Over the shoulder from where they sit, as on foot: a car's passenger, who fires their own guns. */
  | { name: 'passenger' }

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
  /** The gauges the HUD shows from it. */
  gauges?: readonly ('air' | 'eggs' | 'cockpit' | 'range' | 'ammo')[]
  /** The arc drawn ahead of what it fires: a mortar's bomb, or where a plane's bomb would land. */
  arc?: 'mortar' | 'bombs'
}

export interface RideView {
  /** How far behind it the camera sits, metres. */
  distance: number
  /** Ticks between shots of the gun its crew fires, for the HUD's reload ring; null for guns that never reload. */
  reload: number | null
  seats: readonly SeatView[]
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
