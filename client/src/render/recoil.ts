/**
 * Down a scope, the share of a gun's recoil that goes into the aim itself: the shooter pulls this much
 * back down for the next shot. The rest only throws the view and settles back.
 */
export const CLIMB_SHARE = 0.2

/** The time constant of a shot's climb reaching the aim, seconds: 95% of it is there in three of them, so the aim rises rather than jumps. */
const CLIMB_SECONDS = 0.025

/**
 * A gun's climb on its way into the aim: each shot adds to it, and every frame takes a share of what
 * is left and hands it to the input (`InputController.climb`). The aim the server is sent rises with
 * it, so a burst walks up and to the left unless the shooter holds it down.
 */
export class GunClimb {
  private yaw = 0
  private pitch = 0

  /** A shot: `pitch` radians up and `yaw` radians to the left, still to come. */
  add(yaw: number, pitch: number) {
    this.yaw += yaw
    this.pitch += pitch
  }

  /** How much of the climb reaches the aim in the `dt` seconds just passed, radians: to the left and up. */
  step(dt: number): { yaw: number; pitch: number } {
    const share = 1 - Math.exp(-dt / CLIMB_SECONDS)
    const now = { yaw: this.yaw * share, pitch: this.pitch * share }
    this.yaw -= now.yaw
    this.pitch -= now.pitch
    return now
  }

  /** Forgets what was still to come: the soldier died, or put the gun away. */
  reset() {
    this.yaw = 0
    this.pitch = 0
  }
}

/** Seconds the view takes to snap up to the top of its kick. */
const RISE_SECONDS = 0.05
/** Seconds of the critically damped settle back from the top: most of the way back in about four of them. */
const SETTLE_SECONDS = 0.1

/**
 * A scope's view thrown up by a shot and settling back: an offset added to where it looks, which
 * leaves the aim alone. It snaps up in a few hundredths of a second and comes back over about half a
 * second, so the shooter loses the mark and finds it again.
 */
export class ScopeRecoil {
  private pitch = 0
  private yaw = 0
  /** Seconds since the last kick; Infinity before any. */
  private age = Infinity
  /** Where the kick started from and how far it throws, radians. */
  private from = { pitch: 0, yaw: 0 }
  private to = { pitch: 0, yaw: 0 }

  /** A shot: `rise` radians up and `side` radians to one side (positive turns the view left). */
  kick(rise: number, side: number) {
    this.from = { pitch: this.pitch, yaw: this.yaw }
    this.to = { pitch: this.pitch + rise, yaw: this.yaw + side }
    this.age = 0
  }

  /** The offset `dt` seconds on, radians: how far above and to the left of the aim the view looks. */
  step(dt: number): { pitch: number; yaw: number } {
    this.age += dt
    const t = this.age
    if (t < RISE_SECONDS) {
      const up = Math.sin((t / RISE_SECONDS) * (Math.PI / 2))
      this.pitch = this.from.pitch + (this.to.pitch - this.from.pitch) * up
      this.yaw = this.from.yaw + (this.to.yaw - this.from.yaw) * up
    } else {
      const u = (t - RISE_SECONDS) / SETTLE_SECONDS
      const left = Number.isFinite(u) ? (1 + u) * Math.exp(-u) : 0
      this.pitch = this.to.pitch * left
      this.yaw = this.to.yaw * left
    }
    return { pitch: this.pitch, yaw: this.yaw }
  }

  /** Back to still: off the scope, or the gun put away. */
  reset() {
    this.pitch = 0
    this.yaw = 0
    this.age = Infinity
    this.from = { pitch: 0, yaw: 0 }
    this.to = { pitch: 0, yaw: 0 }
  }
}
