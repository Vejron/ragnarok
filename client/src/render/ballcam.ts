/**
 * The golf ball camera: after the local player drives, the view leaves them and follows the ball they
 * struck, from behind and a little above it, trailing it as it flies so its flight is seen whole, until
 * it has lain still a moment, is gone, or the player moves on. It eases in and out of the view it takes
 * over.
 */

/** Seconds after the strike before the view leaves the golfer, so the follow-through is seen. */
const DELAY = 0.35
/** Behind the ball and above it, metres. */
const BEHIND = 7
const ABOVE = 2.2
/** How quickly the camera closes on its place behind the ball, and turns to the ball's heading, 1/s. */
const CHASE = 5
const TURN = 3
/** Seconds the ball lies still before the view goes back, and the most it follows one, seconds. */
const SETTLED = 1.5
const LONGEST = 20
/** How quickly the view eases into the ball camera and back out of it, 1/s. */
const BLEND = 3
/** Least height over the ground, metres. */
const CLEARANCE = 1

type Vec = [number, number, number]

export class BallCam {
  private following = false
  private age = 0
  private still = 0
  /** Where the camera stands, where the ball was last frame, and the way it heads (unit, horizontal). */
  private eye: Vec | null = null
  private last: Vec | null = null
  private heading: [number, number] = [0, -1]
  /** How much of the view is the ball camera's, 0..1. */
  private blend = 0

  /** Follows the ball just struck, which heads off along `yaw` (camera yaw: 0 looks down -z). */
  start(yaw: number) {
    this.following = true
    this.age = 0
    this.still = 0
    this.eye = null
    this.last = null
    this.heading = [-Math.sin(yaw), -Math.cos(yaw)]
  }

  /** The player has moved on: the view eases back to them. */
  stop() {
    this.following = false
  }

  get active() {
    return this.blend > 0.001
  }

  /**
   * Moves on `dt` seconds with the ball at `ball` (null once it is gone), and blends the view: `from` is
   * where the camera stands for the player and `fromAt` a point it looks at. Returns the camera's place
   * and the point to look at, or null when the player's own view is the whole of it.
   */
  update(ball: { x: number; y: number; z: number } | null, from: Vec, fromAt: Vec, dt: number, groundAt: (x: number, z: number) => number): { eye: Vec; at: Vec } | null {
    if (!ball) this.following = false
    this.age += dt
    if (this.age > LONGEST) this.following = false
    const live = this.following && this.age >= DELAY
    this.blend += ((live ? 1 : 0) - this.blend) * Math.min(1, dt * BLEND)
    if (!live && this.blend < 0.001) {
      this.blend = 0
      this.eye = null
      return null
    }
    if (ball) this.track([ball.x, ball.y, ball.z], dt, groundAt)
    if (!this.eye || !this.last) return null
    const t = this.blend * this.blend * (3 - 2 * this.blend)
    const mix = (a: Vec, b: Vec): Vec => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
    return { eye: mix(from, this.eye), at: mix(fromAt, this.last) }
  }

  /** Follows the ball to `at`: turns to its heading while it moves, and trails behind it. */
  private track(at: Vec, dt: number, groundAt: (x: number, z: number) => number) {
    const last = this.last
    this.last = at
    if (last && dt > 0) {
      const [dx, dz] = [at[0] - last[0], at[2] - last[2]]
      const moved = Math.hypot(dx, dz)
      if (moved / dt > 0.2) {
        this.still = 0
        const k = Math.min(1, dt * TURN)
        const [hx, hz] = [this.heading[0] + (dx / moved - this.heading[0]) * k, this.heading[1] + (dz / moved - this.heading[1]) * k]
        const length = Math.hypot(hx, hz) || 1
        this.heading = [hx / length, hz / length]
      } else if ((this.still += dt) > SETTLED) this.following = false
    }
    const want: Vec = [at[0] - this.heading[0] * BEHIND, 0, at[2] - this.heading[1] * BEHIND]
    want[1] = Math.max(at[1] + ABOVE, groundAt(want[0], want[2]) + CLEARANCE)
    if (!this.eye) this.eye = want
    else {
      const k = Math.min(1, dt * CHASE)
      this.eye = [this.eye[0] + (want[0] - this.eye[0]) * k, this.eye[1] + (want[1] - this.eye[1]) * k, this.eye[2] + (want[2] - this.eye[2]) * k]
    }
    this.eye[1] = Math.max(this.eye[1], groundAt(this.eye[0], this.eye[2]) + CLEARANCE)
  }
}
