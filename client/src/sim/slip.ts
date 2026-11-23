import {
  AMV_AXLES_Z, AMV_MAX_STEER, AMV_RIDE_HEIGHT, AMV_STEER_AXLES, AMV_TRACK_HALF, BUS_AXLE_Z, BUS_MAX_STEER, BUS_RIDE_HEIGHT, BUS_TRACK_HALF, EPA_AXLE_Z, EPA_MAX_STEER, EPA_RIDE_HEIGHT, EPA_TRACK_HALF, GOLF_CART_AXLE_Z, GOLF_CART_MAX_STEER, GOLF_CART_RIDE_HEIGHT, GOLF_CART_TRACK_HALF, TECHNICAL_AXLE_Z, TECHNICAL_MAX_STEER, TECHNICAL_RIDE_HEIGHT, TECHNICAL_TRACK_HALF, JEEP_AXLE_Z, JEEP_MAX_STEER, JEEP_RIDE_HEIGHT, JEEP_TRACK_HALF, SNOWMOBILE_AXLE_Z, SNOWMOBILE_MAX_STEER, SNOWMOBILE_RIDE_HEIGHT, SNOWMOBILE_TRACK_HALF, SNOWRACER_AXLE_Z, SNOWRACER_MAX_STEER, SNOWRACER_RIDE_HEIGHT, SNOWRACER_TRACK_HALF, SPORTS_AXLE_Z, SPORTS_MAX_STEER, SPORTS_RIDE_HEIGHT, SPORTS_TRACK_HALF,
  SAAB_900_AXLE_Z, SAAB_900_MAX_STEER, SAAB_900_RIDE_HEIGHT, SAAB_900_TRACK_HALF, SAAB_9000_AXLE_Z, SAAB_9000_MAX_STEER, SAAB_9000_RIDE_HEIGHT, SAAB_9000_TRACK_HALF, VOLVO_240_AXLE_Z, VOLVO_240_MAX_STEER, VOLVO_240_RIDE_HEIGHT, VOLVO_240_TRACK_HALF, VOLVO_740_AXLE_Z, VOLVO_740_MAX_STEER, VOLVO_740_RIDE_HEIGHT, VOLVO_740_TRACK_HALF, GTI_AXLE_Z, GTI_MAX_STEER, GTI_RIDE_HEIGHT, GTI_TRACK_HALF, TRANSPORTER_AXLE_Z, TRANSPORTER_MAX_STEER, TRANSPORTER_RIDE_HEIGHT, TRANSPORTER_TRACK_HALF, KADETT_AXLE_Z, KADETT_MAX_STEER, KADETT_RIDE_HEIGHT, KADETT_TRACK_HALF, OMEGA_AXLE_Z, OMEGA_MAX_STEER, OMEGA_RIDE_HEIGHT, OMEGA_TRACK_HALF, SIERRA_AXLE_Z, SIERRA_MAX_STEER, SIERRA_RIDE_HEIGHT, SIERRA_TRACK_HALF, ESCORT_AXLE_Z, ESCORT_MAX_STEER, ESCORT_RIDE_HEIGHT, ESCORT_TRACK_HALF,
  PBV_AXLES_Z, PBV_RIDE_HEIGHT, PBV_TRACK_HALF, STRF_AXLES_Z, STRF_RIDE_HEIGHT, STRF_TRACK_HALF, STRV_AXLES_Z, STRV_RIDE_HEIGHT, STRV_TRACK_HALF, TANK_AXLES_Z, TANK_RIDE_HEIGHT, TANK_TRACK_HALF, PLANE_AXLES_Z, PLANE_MAX_STEER, PLANE_TRACK_HALF, SEAPLANE_AXLES_Z, SEAPLANE_FLOAT_HALF_TRACK, SEAPLANE_MAX_STEER, SEAPLANE_RIDE_HEIGHT,
} from './consts'

/**
 * Where a vehicle's tyres meet the ground and how hard they are sliding, worked out from where the
 * vehicle is drawn. Only effects use it; the physics has its own wheels.
 */

export interface WheelLayout {
  /** Contact points in the body frame, front first, left before right. */
  contacts: readonly (readonly [number, number, number])[]
  /** How many of them, from the front, steer. */
  steered: number
  /** Front wheel angle at full lock, radians. */
  maxSteer: number
  /** How far each steered contact turns, as a share of `maxSteer`; all of it when left out. */
  shares?: readonly number[]
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
/**
 * How far each of the first `steered` axles of `axles` (front first) turns, as a share of the front one's
 * lock: the server's `Spec::steer_share`, each in proportion to its distance from the middle of the axles
 * that do not steer.
 */
export function steerShares(axles: readonly number[], steered: number): number[] {
  const fixed = axles.slice(steered)
  if (fixed.length === 0) return axles.slice(0, steered).map(() => 1)
  const pivot = fixed.reduce((sum, z) => sum + z, 0) / fixed.length
  return axles.slice(0, steered).map((z) => (pivot - z) / (pivot - axles[0]!))
}

/** The AMV's front axles' shares of the lock. */
export const AMV_STEER_SHARES = steerShares(AMV_AXLES_Z, AMV_STEER_AXLES)

/** The AMV's eight wheels, two to each of its four axles: the front two pairs steer, as in the physics. */
export const AMV_WHEELS: WheelLayout = {
  contacts: AMV_AXLES_Z.flatMap((z) => [[-AMV_TRACK_HALF, -AMV_RIDE_HEIGHT, z] as const, [AMV_TRACK_HALF, -AMV_RIDE_HEIGHT, z] as const]),
  steered: 2 * AMV_STEER_AXLES,
  shares: AMV_STEER_SHARES.flatMap((share) => [share, share]),
  maxSteer: AMV_MAX_STEER,
  width: 0.4,
}
export const EPA_WHEELS: WheelLayout = { contacts: corners(EPA_TRACK_HALF, -EPA_AXLE_Z, EPA_AXLE_Z, EPA_RIDE_HEIGHT), steered: 2, maxSteer: EPA_MAX_STEER, width: 0.22 }
/** The period cars of the traffic (VEHICLES-XXIII.md). */
export const SAAB_900_WHEELS: WheelLayout = { contacts: corners(SAAB_900_TRACK_HALF, -SAAB_900_AXLE_Z, SAAB_900_AXLE_Z, SAAB_900_RIDE_HEIGHT), steered: 2, maxSteer: SAAB_900_MAX_STEER, width: 0.2 }
export const SAAB_9000_WHEELS: WheelLayout = { contacts: corners(SAAB_9000_TRACK_HALF, -SAAB_9000_AXLE_Z, SAAB_9000_AXLE_Z, SAAB_9000_RIDE_HEIGHT), steered: 2, maxSteer: SAAB_9000_MAX_STEER, width: 0.21 }
export const VOLVO_240_WHEELS: WheelLayout = { contacts: corners(VOLVO_240_TRACK_HALF, -VOLVO_240_AXLE_Z, VOLVO_240_AXLE_Z, VOLVO_240_RIDE_HEIGHT), steered: 2, maxSteer: VOLVO_240_MAX_STEER, width: 0.2 }
export const VOLVO_740_WHEELS: WheelLayout = { contacts: corners(VOLVO_740_TRACK_HALF, -VOLVO_740_AXLE_Z, VOLVO_740_AXLE_Z, VOLVO_740_RIDE_HEIGHT), steered: 2, maxSteer: VOLVO_740_MAX_STEER, width: 0.2 }
export const GTI_WHEELS: WheelLayout = { contacts: corners(GTI_TRACK_HALF, -GTI_AXLE_Z, GTI_AXLE_Z, GTI_RIDE_HEIGHT), steered: 2, maxSteer: GTI_MAX_STEER, width: 0.2 }
export const TRANSPORTER_WHEELS: WheelLayout = { contacts: corners(TRANSPORTER_TRACK_HALF, -TRANSPORTER_AXLE_Z, TRANSPORTER_AXLE_Z, TRANSPORTER_RIDE_HEIGHT), steered: 2, maxSteer: TRANSPORTER_MAX_STEER, width: 0.21 }
export const KADETT_WHEELS: WheelLayout = { contacts: corners(KADETT_TRACK_HALF, -KADETT_AXLE_Z, KADETT_AXLE_Z, KADETT_RIDE_HEIGHT), steered: 2, maxSteer: KADETT_MAX_STEER, width: 0.19 }
export const OMEGA_WHEELS: WheelLayout = { contacts: corners(OMEGA_TRACK_HALF, -OMEGA_AXLE_Z, OMEGA_AXLE_Z, OMEGA_RIDE_HEIGHT), steered: 2, maxSteer: OMEGA_MAX_STEER, width: 0.2 }
export const SIERRA_WHEELS: WheelLayout = { contacts: corners(SIERRA_TRACK_HALF, -SIERRA_AXLE_Z, SIERRA_AXLE_Z, SIERRA_RIDE_HEIGHT), steered: 2, maxSteer: SIERRA_MAX_STEER, width: 0.2 }
export const ESCORT_WHEELS: WheelLayout = { contacts: corners(ESCORT_TRACK_HALF, -ESCORT_AXLE_Z, ESCORT_AXLE_Z, ESCORT_RIDE_HEIGHT), steered: 2, maxSteer: ESCORT_MAX_STEER, width: 0.18 }
export const BUS_WHEELS: WheelLayout = { contacts: corners(BUS_TRACK_HALF, -BUS_AXLE_Z, BUS_AXLE_Z, BUS_RIDE_HEIGHT), steered: 2, maxSteer: BUS_MAX_STEER, width: 0.3 }
/** The snowmobile's two skis, then its track: a wide mark from the middle of the sled to its tail. */
export const SNOWMOBILE_WHEELS: WheelLayout = {
  contacts: [[-SNOWMOBILE_TRACK_HALF, -SNOWMOBILE_RIDE_HEIGHT, -SNOWMOBILE_AXLE_Z], [SNOWMOBILE_TRACK_HALF, -SNOWMOBILE_RIDE_HEIGHT, -SNOWMOBILE_AXLE_Z], [-0.1, -SNOWMOBILE_RIDE_HEIGHT, SNOWMOBILE_AXLE_Z], [0.1, -SNOWMOBILE_RIDE_HEIGHT, SNOWMOBILE_AXLE_Z]],
  steered: 2,
  maxSteer: SNOWMOBILE_MAX_STEER,
  width: 0.2,
}
/** The snowracer's one steering ski in the middle, then its two runners: the physics' front pair both stand for the ski. */
export const SNOWRACER_WHEELS: WheelLayout = {
  contacts: [[0, -SNOWRACER_RIDE_HEIGHT, -SNOWRACER_AXLE_Z], [0, -SNOWRACER_RIDE_HEIGHT, -SNOWRACER_AXLE_Z], [-SNOWRACER_TRACK_HALF, -SNOWRACER_RIDE_HEIGHT, SNOWRACER_AXLE_Z], [SNOWRACER_TRACK_HALF, -SNOWRACER_RIDE_HEIGHT, SNOWRACER_AXLE_Z]],
  steered: 2,
  maxSteer: SNOWRACER_MAX_STEER,
  width: 0.1,
}
/** A track lays its mark from end to end; the corners stand in for it. */
export const TRACK_WHEELS: WheelLayout = { contacts: corners(TANK_TRACK_HALF, TANK_AXLES_Z[0], TANK_AXLES_Z[2], TANK_RIDE_HEIGHT), steered: 0, maxSteer: 0, width: 0.6 }
/** The Strv S's tracks: a shorter run on the ground than the tank's, under a hull that rides higher. */
export const STRV_TRACKS: WheelLayout = { contacts: corners(STRV_TRACK_HALF, STRV_AXLES_Z[0], STRV_AXLES_Z[2], STRV_RIDE_HEIGHT), steered: 0, maxSteer: 0, width: 0.62 }
/** The Strf 90's tracks: shorter and narrower than the tank's. */
export const STRF_TRACKS: WheelLayout = { contacts: corners(STRF_TRACK_HALF, STRF_AXLES_Z[0], STRF_AXLES_Z[2], STRF_RIDE_HEIGHT), steered: 0, maxSteer: 0, width: 0.5 }
/** The Pbv 302's tracks: narrow, under a hull that floats (afloat they mark nothing). */
export const PBV_TRACKS: WheelLayout = { contacts: corners(PBV_TRACK_HALF, PBV_AXLES_Z[0], PBV_AXLES_Z[2], PBV_RIDE_HEIGHT), steered: 0, maxSteer: 0, width: 0.38 }

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
