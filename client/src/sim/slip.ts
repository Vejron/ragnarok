import {
  GOLF_CART_AXLE_Z, GOLF_CART_MAX_STEER, GOLF_CART_RIDE_HEIGHT, GOLF_CART_TRACK_HALF, TECHNICAL_AXLE_Z, TECHNICAL_MAX_STEER, TECHNICAL_RIDE_HEIGHT, TECHNICAL_TRACK_HALF, JEEP_AXLE_Z, JEEP_MAX_STEER, JEEP_RIDE_HEIGHT, JEEP_TRACK_HALF, SNOWMOBILE_AXLE_Z, SNOWMOBILE_MAX_STEER, SNOWMOBILE_RIDE_HEIGHT, SNOWMOBILE_TRACK_HALF, SPORTS_AXLE_Z, SPORTS_MAX_STEER, SPORTS_RIDE_HEIGHT, SPORTS_TRACK_HALF,
  STRF_AXLES_Z, STRF_RIDE_HEIGHT, STRF_TRACK_HALF, TANK_AXLES_Z, TANK_TRACK_HALF, PLANE_AXLES_Z, PLANE_MAX_STEER, PLANE_TRACK_HALF, SEAPLANE_AXLES_Z, SEAPLANE_FLOAT_HALF_TRACK, SEAPLANE_MAX_STEER, SEAPLANE_RIDE_HEIGHT,
} from './consts'

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

/** A car's four wheels; the walker, the hen, the drones and the mortar use the jeep's too (`kinds/`). */
export const JEEP_WHEELS: WheelLayout = { contacts: corners(JEEP_TRACK_HALF, -JEEP_AXLE_Z, JEEP_AXLE_Z, JEEP_RIDE_HEIGHT), steered: 2, maxSteer: JEEP_MAX_STEER, width: 0.32 }
export const SPORTS_WHEELS: WheelLayout = { contacts: corners(SPORTS_TRACK_HALF, -SPORTS_AXLE_Z, SPORTS_AXLE_Z, SPORTS_RIDE_HEIGHT), steered: 2, maxSteer: SPORTS_MAX_STEER, width: 0.26 }
export const GOLF_CART_WHEELS: WheelLayout = { contacts: corners(GOLF_CART_TRACK_HALF, -GOLF_CART_AXLE_Z, GOLF_CART_AXLE_Z, GOLF_CART_RIDE_HEIGHT), steered: 2, maxSteer: GOLF_CART_MAX_STEER, width: 0.18 }
export const TECHNICAL_WHEELS: WheelLayout = { contacts: corners(TECHNICAL_TRACK_HALF, -TECHNICAL_AXLE_Z, TECHNICAL_AXLE_Z, TECHNICAL_RIDE_HEIGHT), steered: 2, maxSteer: TECHNICAL_MAX_STEER, width: 0.36 }
/** The snowmobile's two skis, then its track: a wide mark from the middle of the sled to its tail. */
export const SNOWMOBILE_WHEELS: WheelLayout = {
  contacts: [[-SNOWMOBILE_TRACK_HALF, -SNOWMOBILE_RIDE_HEIGHT, -SNOWMOBILE_AXLE_Z], [SNOWMOBILE_TRACK_HALF, -SNOWMOBILE_RIDE_HEIGHT, -SNOWMOBILE_AXLE_Z], [-0.1, -SNOWMOBILE_RIDE_HEIGHT, SNOWMOBILE_AXLE_Z], [0.1, -SNOWMOBILE_RIDE_HEIGHT, SNOWMOBILE_AXLE_Z]],
  steered: 2,
  maxSteer: SNOWMOBILE_MAX_STEER,
  width: 0.2,
}
/** A track lays its mark from end to end; the corners stand in for it. The Strv S's are the tank's. */
export const TRACK_WHEELS: WheelLayout = { contacts: corners(TANK_TRACK_HALF, TANK_AXLES_Z[0], TANK_AXLES_Z[2], TANK_RIDE_HEIGHT), steered: 0, maxSteer: 0, width: 0.55 }
/** The Strf 90's tracks: longer, set wider and a touch narrower than the tank's. */
export const STRF_TRACKS: WheelLayout = { contacts: corners(STRF_TRACK_HALF, STRF_AXLES_Z[0], STRF_AXLES_Z[2], STRF_RIDE_HEIGHT), steered: 0, maxSteer: 0, width: 0.5 }

/** Chassis centre above the ground of a parked plane, metres (`build_plane.py`'s `GROUND`). */
const PLANE_RIDE_HEIGHT = 1.05
/** Two wheels on the axle, and the tail skid dragging a narrow mark behind. */
export const PLANE_WHEELS: WheelLayout = {
  contacts: [[-PLANE_TRACK_HALF, -PLANE_RIDE_HEIGHT, PLANE_AXLES_Z[0]], [PLANE_TRACK_HALF, -PLANE_RIDE_HEIGHT, PLANE_AXLES_Z[0]], [-0.05, -PLANE_RIDE_HEIGHT, PLANE_AXLES_Z[1]], [0.05, -PLANE_RIDE_HEIGHT, PLANE_AXLES_Z[1]]] as const,
  steered: 2,
  maxSteer: PLANE_MAX_STEER,
  width: 0.14,
}

/** The seaplane's small wheels, a pair on each pontoon. */
export const SEAPLANE_WHEELS: WheelLayout = {
  contacts: corners(SEAPLANE_FLOAT_HALF_TRACK, SEAPLANE_AXLES_Z[0], SEAPLANE_AXLES_Z[1], SEAPLANE_RIDE_HEIGHT),
  steered: 2,
  maxSteer: SEAPLANE_MAX_STEER,
  width: 0.12,
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
