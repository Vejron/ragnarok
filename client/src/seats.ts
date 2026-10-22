/**
 * What sitting in each seat of each vehicle means on this client: where the camera goes, what the HUD
 * calls the seat and shows, what E says to board it, and what the seat aims. One row per vehicle kind,
 * in seat order, so a new vehicle is one row here rather than a branch in the camera, the HUD and the
 * prompt. What a seat does in the game (who drives, who fires what) is the server's, in `VehicleDef`.
 */

import { KIND_DRONE, KIND_HELI, KIND_HEN, KIND_JEEP, KIND_LOITER, KIND_MORTAR, KIND_PLANE, KIND_ROADSTER, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_WALKER } from '@/net/codec'
import type { HudStats } from '@/hud'
import { DRIVING_DISTANCE, DRONE_DISTANCE, HELI_DISTANCE, HEN_DISTANCE, LOITER_DISTANCE, MORTAR_DISTANCE, PLANE_DISTANCE, STRV_DISTANCE, TANK_DISTANCE, WALKER_DISTANCE } from '@/render/scene'
import {
  HELI_GUN_COOLDOWN_TICKS, HELI_GUN_PITCH_MAX, HELI_GUN_PITCH_MIN, HELI_GUN_YAW_LIMIT, MORTAR_COOLDOWN_TICKS, PLANE_REAR_PITCH_MAX, PLANE_REAR_PITCH_MIN,
  SHELL_COOLDOWN_TICKS, TANK_BARREL_MAX, TANK_BARREL_MIN, WALKER_GUN_COOLDOWN_TICKS,
} from '@/sim/consts'
import { DRONE_RANGE, LOITER_RANGE } from '@/sim/items'

/** A loitering drone's look is kept between these pitches, radians: steep enough to dive, not to loop. */
const LOITER_PITCH: readonly [number, number] = [-1.2, 0.6]
/** An FPV drone's. */
const DRONE_PITCH: readonly [number, number] = [-1.4, 1.0]

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
  gauges?: readonly ('air' | 'eggs' | 'cockpit' | 'range')[]
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

const DRIVE: Rig = { name: 'drive' }
const RIDE_ALONG: SeatView = { rig: { name: 'passenger' }, role: 'passenger', board: 'E: ride along' }

/** Every vehicle kind's seats, as this client presents them. */
const RIDES: Record<number, RideView> = {
  [KIND_JEEP]: { distance: DRIVING_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the jeep', chase: true }, RIDE_ALONG, RIDE_ALONG, RIDE_ALONG] },
  [KIND_SPORTS]: { distance: DRIVING_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the sports car', chase: true }, RIDE_ALONG] },
  [KIND_ROADSTER]: { distance: DRIVING_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'roadster', board: 'E: drive the Roadster', chase: true, gauges: ['air'] }, RIDE_ALONG] },
  [KIND_TANK]: {
    distance: TANK_DISTANCE, reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the tank', chase: true }, { rig: { name: 'gun', level: [TANK_BARREL_MIN, TANK_BARREL_MAX] }, role: 'gunner', board: 'E: man the gun', sight: true }],
  },
  [KIND_HELI]: {
    distance: HELI_DISTANCE, reload: HELI_GUN_COOLDOWN_TICKS, downwash: { reach: 18, radius: 6 },
    seats: [{ rig: DRIVE, role: 'pilot', board: 'E: fly' }, { rig: { name: 'gun', hull: [HELI_GUN_YAW_LIMIT, HELI_GUN_PITCH_MIN, HELI_GUN_PITCH_MAX] }, role: 'gunner', board: 'E: man the nose gun' }],
  },
  [KIND_PLANE]: {
    distance: PLANE_DISTANCE, reload: null,
    seats: [
      { rig: { name: 'aviator' }, role: 'aviator', board: 'E: fly the triplane', gauges: ['cockpit'], arc: 'bombs' },
      { rig: { name: 'gun', hull: [Math.PI, PLANE_REAR_PITCH_MIN, PLANE_REAR_PITCH_MAX] }, role: 'gunner', board: 'E: man the rear gun', gauges: ['cockpit'] },
    ],
  },
  [KIND_WALKER]: { distance: WALKER_DISTANCE, reload: WALKER_GUN_COOLDOWN_TICKS, seats: [{ rig: { name: 'walker' }, role: 'walker', board: 'E: drive the walker', chase: true }] },
  [KIND_STRV]: { distance: STRV_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: { name: 'tanker' }, role: 'strv', board: 'E: drive the Strv S', chase: true, sight: true }] },
  [KIND_MORTAR]: { distance: MORTAR_DISTANCE, reload: MORTAR_COOLDOWN_TICKS, seats: [{ rig: { name: 'mortar' }, role: 'mortar', board: 'E: man the mortar', gauges: ['range'], arc: 'mortar' }] },
  [KIND_HEN]: { distance: HEN_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'hen', board: 'E: drive the hen', chase: true, gauges: ['air', 'eggs'] }] },
  [KIND_DRONE]: { distance: DRONE_DISTANCE, reload: SHELL_COOLDOWN_TICKS, downwash: { reach: 4, radius: 1.5 }, seats: [{ rig: { name: 'fpv', pitch: DRONE_PITCH, link: DRONE_RANGE }, role: 'drone' }] },
  [KIND_LOITER]: { distance: LOITER_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: { name: 'fpv', pitch: LOITER_PITCH, link: LOITER_RANGE, belly: true }, role: 'loiter' }] },
}

/** How a vehicle of `kind` is presented. Unknown kinds are jeeps. */
export const rideView = (kind: number): RideView => RIDES[kind] ?? RIDES[KIND_JEEP]!

/** How seat `index` of a vehicle of `kind` is presented: its own row, or the last seat's. */
export function seatView(kind: number, index: number): SeatView {
  const seats = rideView(kind).seats
  return seats[Math.min(index, seats.length - 1)]!
}
