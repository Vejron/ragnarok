import {
  JEEP_AXLE_Z, JEEP_MAX_STEER, JEEP_RIDE_HEIGHT, JEEP_TRACK_HALF, SPORTS_AXLE_Z, SPORTS_MAX_STEER, SPORTS_RIDE_HEIGHT, SPORTS_TRACK_HALF,
  TANK_AXLES_Z, TANK_TRACK_HALF, PLANE_AXLES_Z, PLANE_MAX_STEER, PLANE_TRACK_HALF,
} from './consts'
import { KIND_HELI, KIND_PLANE, KIND_SPORTS, KIND_TANK } from '@/net/codec'

/**
 * Where a vehicle's tyres meet the ground and how hard they are sliding, worked out from where the
 * vehicle is drawn. Only effects use it; the physics has its own wheels.
 */

/** Chassis centre above the ground of a settled tank, metres (measured in `vehicle.rs`'s fixture). */
const TANK_RIDE_HEIGHT = 0.94

export interface WheelLayout {
  /** Contact points in the body frame, front first, left before right. */
  contacts: readonly (readonly [number, number, number])[]
  /** How many of them, from the front, steer. */
  steered: number
  /** Front wheel angle at full lock, radians. */
  maxSteer: number
  /** Width of the mark a tyre leaves, metres. */
  width: number
}

function corners(track: number, front: number, back: number, ride: number): (readonly [number, number, number])[] {
  return [[-track, -ride, front], [track, -ride, front], [-track, -ride, back], [track, -ride, back]]
}

const JEEP = { contacts: corners(JEEP_TRACK_HALF, -JEEP_AXLE_Z, JEEP_AXLE_Z, JEEP_RIDE_HEIGHT), steered: 2, maxSteer: JEEP_MAX_STEER, width: 0.32 }
const SPORTS = { contacts: corners(SPORTS_TRACK_HALF, -SPORTS_AXLE_Z, SPORTS_AXLE_Z, SPORTS_RIDE_HEIGHT), steered: 2, maxSteer: SPORTS_MAX_STEER, width: 0.26 }
// A track lays its mark from end to end; the corners stand in for it.
const TANK = { contacts: corners(TANK_TRACK_HALF, TANK_AXLES_Z[0], TANK_AXLES_Z[2], TANK_RIDE_HEIGHT), steered: 0, maxSteer: 0, width: 0.55 }

/** Chassis centre above the ground of a parked plane, metres (`build_plane.py`'s `GROUND`). */
const PLANE_RIDE_HEIGHT = 1.05
// Two wheels on the axle, and the tail skid dragging a narrow mark behind.
const PLANE = {
  contacts: [[-PLANE_TRACK_HALF, -PLANE_RIDE_HEIGHT, PLANE_AXLES_Z[0]], [PLANE_TRACK_HALF, -PLANE_RIDE_HEIGHT, PLANE_AXLES_Z[0]], [-0.05, -PLANE_RIDE_HEIGHT, PLANE_AXLES_Z[1]], [0.05, -PLANE_RIDE_HEIGHT, PLANE_AXLES_Z[1]]] as const,
  steered: 2,
  maxSteer: PLANE_MAX_STEER,
  width: 0.14,
}

/** The tyres of a vehicle kind, or null for one that has none (the helicopter). */
export function wheelLayout(kind: number): WheelLayout | null {
  if (kind === KIND_HELI) return null
  if (kind === KIND_PLANE) return PLANE
  if (kind === KIND_TANK) return TANK
  if (kind === KIND_SPORTS) return SPORTS
  return JEEP
}

/** Sideways speed (m/s) under which a tyre is gripping. */
const GRIP_SPEED = 1.8
/** Sideways speed over `GRIP_SPEED` at which a tyre slides flat out. */
const SLIDE_SPAN = 5
/** Deceleration (m/s²) above which braking locks the wheels, and the speed it needs. */
const LOCK_DECEL = 8
const LOCK_SPEED = 5
/** Acceleration (m/s²) above which a launch spins the wheels, and the speed under which it can. */
const SPIN_ACCEL = 7
const SPIN_SPEED = 14

/**
 * How hard a tyre is sliding, 0 gripping to 1 flat out: `sideways` is the speed of its contact
 * point across the wheel, `speed` the vehicle's along its nose and `accel` how fast that changes.
 * `driven` wheels also spin on a hard launch.
 */
export function tyreSlip(sideways: number, speed: number, accel: number, driven: boolean): number {
  const slide = Math.min(1, Math.max(0, (Math.abs(sideways) - GRIP_SPEED) / SLIDE_SPAN))
  const locked = Math.abs(speed) > LOCK_SPEED && accel * Math.sign(speed) < -LOCK_DECEL ? Math.min(1, (-accel * Math.sign(speed) - LOCK_DECEL) / 6 + 0.4) : 0
  const spun = driven && Math.abs(speed) < SPIN_SPEED && accel > SPIN_ACCEL ? Math.min(1, (accel - SPIN_ACCEL) / 3 + 0.3) : 0
  return Math.max(slide, locked, spun)
}
