/**
 * What makes the camera behind a car feel fast: the field of view widens with speed, the camera
 * drops back when the car pulls away and closes in when it brakes, and it leans into turns. It
 * shakes only when told to (`kick`), which the game does for real collisions; braking, cornering
 * and bumps never shake it. The rest is worked out from where the car is drawn, frame by frame.
 */

/** Speed at which the field of view is widest, m/s. */
const FULL_SPEED = 45
/** Degrees of extra field of view at `FULL_SPEED`. */
const MAX_FOV_BOOST = 14
/** Extra follow distance at `FULL_SPEED`, as a share of the base distance. */
const SPEED_STRETCH = 0.18
/** Metres the camera drops back per m/s² of acceleration (forward is positive), and its limits. */
const PULL_PER_ACCEL = 0.09
const PULL_MIN = -1.5
const PULL_MAX = 2.0
/** Radians the view leans into a turn per rad/s of yaw rate, and its limit. */
const LEAN_PER_YAW_RATE = 0.12
const LEAN_MAX = 0.18
/** How quickly the smoothed speed, acceleration and lean follow the car, per second. */
const SMOOTHING = 6
/** Trauma lost per second; the shake is trauma², up to `SHAKE_METRES`. */
const TRAUMA_DECAY = 1.6
const SHAKE_METRES = 0.35
/** Frames longer than this are ignored for derivatives (a stall, a tab switch). */
const MAX_DT = 0.1

export interface ChaseView {
  /** Degrees to add to the field of view. */
  fovBoost: number
  /** Metres to add to the follow distance. */
  distance: number
  /** Radians to add to the camera's yaw so it looks into the turn. */
  lean: number
  /** Offset of the camera's position, metres. */
  shake: [number, number, number]
}

export class ChaseCam {
  private last: [number, number, number] | null = null
  private lastYaw = 0
  private speed = 0
  private accel = 0
  private yawRate = 0
  private pull = 0
  private trauma = 0
  private time = 0

  /** Forgets the car followed so far, e.g. on leaving it. */
  reset() {
    this.last = null
    this.speed = this.accel = this.yawRate = this.pull = this.trauma = 0
  }

  /** Shakes the camera, `amount` 0..1 (1 is a heavy crash). */
  kick(amount: number) {
    this.trauma = Math.min(1, this.trauma + amount)
  }

  /** Follows a car drawn at `position` heading `yaw` (0 faces -Z), `dt` seconds after the last call. */
  update(position: readonly [number, number, number], yaw: number, dt: number, baseDistance: number, random: () => number = Math.random): ChaseView {
    this.time += dt
    if (this.last && dt > 1e-4 && dt < MAX_DT) {
      const vx = (position[0] - this.last[0]) / dt
      const vz = (position[2] - this.last[2]) / dt
      // Speed along the heading, so reversing reads as negative.
      const along = -Math.sin(yaw) * vx - Math.cos(yaw) * vz
      const blend = Math.min(1, dt * SMOOTHING)
      const accel = (along - this.speed) / dt
      this.speed += (along - this.speed) * blend
      this.accel += (accel - this.accel) * blend
      let turn = yaw - this.lastYaw
      turn = Math.atan2(Math.sin(turn), Math.cos(turn))
      this.yawRate += (turn / dt - this.yawRate) * blend
      const wanted = Math.max(PULL_MIN, Math.min(PULL_MAX, this.accel * PULL_PER_ACCEL))
      this.pull += (wanted - this.pull) * Math.min(1, dt * 2.5)
    }
    this.last = [position[0], position[1], position[2]]
    this.lastYaw = yaw
    this.trauma = Math.max(0, this.trauma - TRAUMA_DECAY * dt)

    const pace = Math.min(1, Math.abs(this.speed) / FULL_SPEED)
    const shake = this.trauma * this.trauma * SHAKE_METRES
    const wobble = () => (random() * 2 - 1) * shake
    return {
      fovBoost: MAX_FOV_BOOST * Math.pow(pace, 1.3),
      distance: baseDistance * SPEED_STRETCH * pace + this.pull,
      lean: Math.max(-LEAN_MAX, Math.min(LEAN_MAX, this.yawRate * LEAN_PER_YAW_RATE)),
      shake: [wobble(), wobble(), wobble()],
    }
  }
}
