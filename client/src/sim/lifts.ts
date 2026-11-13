import { LIFT_CHAIR_DROP, LIFT_CHAIR_SPACING, LIFT_GAUGE, LIFT_MAX_SEATS, LIFT_SEAT_PITCH, LIFT_SPEED, LIFT_WHEEL_SEGMENTS, TICK_RATE } from './consts'
import type { Lift } from '@/world'

/**
 * Ski lifts, mirroring `crates/shared/src/lifts.rs`: where each chair of a lift is at a server tick,
 * so the chairs drawn are the ones the server carries riders on. `lifts.spec.ts` holds this copy to
 * the Rust one. The cable loops up the right of the lift's line, round the top station's bull wheel,
 * down the left and round the bottom's, straight from pylon to pylon. LIFTS.md.
 */

/** Which stretch of its loop a chair is on: going up, round the top wheel, coming down, round the bottom. */
export const STRETCH_UP = 0
export const STRETCH_TOP = 1
export const STRETCH_DOWN = 2
export const STRETCH_BOTTOM = 3

/** Where a chair is: its seat's middle (metres), yaw (radians, 0 faces -Z), distance round the loop and stretch. */
export interface Chair {
  x: number
  y: number
  z: number
  yaw: number
  along: number
  stretch: number
}

/** A lift's cable loop, laid out once. */
export class LiftPath {
  /** The cable's corners, `[x, y, z]` metres, flattened: up the right, round the top, down the left, round the bottom. */
  readonly points: number[] = []
  /** Distance round the loop to each corner, metres, and to its end last. */
  readonly corners: number[] = []
  length = 0
  /** The length the clock runs on, metres, rounded to the millimetre like the Rust's. */
  clockLength = 0
  /** Where the stretches end round the loop, metres: going up, round the top, coming down. */
  up = 0
  top = 0
  down = 0
  /** Chairs on the loop, the distance between them (metres) and the riders a chair carries. */
  chairs = 0
  spacing = 0
  seats = 1
  /** Points of the lift's line. */
  readonly count: number

  constructor(lift: Lift) {
    const n = Math.min(lift.points.length, lift.cable.length)
    this.count = n
    if (n < 2) return
    const at = (i: number): [number, number, number] => [lift.points[i]![0], lift.cable[i]!, lift.points[i]![1]]
    const ahead = (i: number): [number, number] => {
      const a = at(Math.max(0, i - 1))
      const b = at(Math.min(n - 1, i + 1))
      const dx = b[0] - a[0]
      const dz = b[2] - a[2]
      const length = Math.max(Math.hypot(dx, dz), 1e-6)
      return [dx / length, dz / length]
    }
    const half = LIFT_GAUGE / 2
    const push = (p: number[]) => this.points.push(p[0]!, p[1]!, p[2]!)
    const side = (i: number, sign: number) => {
      const [fx, fz] = ahead(i)
      const p = at(i)
      push([p[0] - fz * half * sign, p[1], p[2] + fx * half * sign])
    }
    const wheel = (centre: number[], forward: [number, number]) => {
      for (let k = 1; k < LIFT_WHEEL_SEGMENTS; k++) {
        const angle = (Math.PI * k) / LIFT_WHEEL_SEGMENTS
        const across = Math.cos(angle) * half
        const on = Math.sin(angle) * half
        push([centre[0]! - forward[1] * across + forward[0] * on, centre[1]!, centre[2]! + forward[0] * across + forward[1] * on])
      }
    }
    for (let i = 0; i < n; i++) side(i, 1)
    const upCorners = n
    wheel(at(n - 1), ahead(n - 1))
    const topCorners = this.points.length / 3
    for (let i = n - 1; i >= 0; i--) side(i, -1)
    const downCorners = this.points.length / 3
    const back = ahead(0)
    wheel(at(0), [-back[0], -back[1]])
    const corners = this.points.length / 3
    let total = 0
    for (let i = 0; i < corners; i++) {
      this.corners.push(total)
      const j = (i + 1) % corners
      total += Math.hypot(this.points[j * 3]! - this.points[i * 3]!, this.points[j * 3 + 1]! - this.points[i * 3 + 1]!, this.points[j * 3 + 2]! - this.points[i * 3 + 2]!)
    }
    this.corners.push(total)
    this.length = total
    this.up = this.corners[upCorners - 1]!
    this.top = this.corners[topCorners]!
    this.down = this.corners[downCorners - 1]!
    // Rounded to the millimetre, as `clock_length` is: the clock runs on it.
    this.clockLength = Math.round(total * 1000) / 1000
    this.chairs = Math.max(1, Math.floor(this.clockLength / LIFT_CHAIR_SPACING))
    this.spacing = this.clockLength / this.chairs
    this.seats = Math.min(LIFT_MAX_SEATS, Math.max(1, lift.seats))
  }

  /** The stretch of the loop `along` metres round it is on (`STRETCH_*`). */
  stretch(along: number): number {
    return along < this.up ? STRETCH_UP : along < this.top ? STRETCH_TOP : along < this.down ? STRETCH_DOWN : STRETCH_BOTTOM
  }

  /** How far round the loop chair `index` is at server tick `tick` (fractional between ticks), metres. */
  along(index: number, tick: number): number {
    if (this.clockLength <= 0) return 0
    const run = (tick / TICK_RATE) * LIFT_SPEED + index * this.spacing
    return ((((run % this.clockLength) + this.clockLength) % this.clockLength) / this.clockLength) * this.length
  }

  /** Chair `index` at `tick`, into `out`. */
  chair(index: number, tick: number, out: Chair = blank()): Chair {
    return this.at(this.along(index, tick), out)
  }

  /** A chair `along` metres round the loop, into `out`. */
  at(along: number, out: Chair = blank()): Chair {
    const corners = this.points.length / 3
    if (corners === 0) return Object.assign(out, blank())
    // The last corner at or before `along`, by bisection.
    let lo = 0
    let hi = corners - 1
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1
      if (this.corners[mid]! <= along) lo = mid
      else hi = mid - 1
    }
    const i = lo
    const j = (i + 1) % corners
    const span = Math.max(this.corners[i + 1]! - this.corners[i]!, 1e-6)
    const t = Math.min(1, Math.max(0, (along - this.corners[i]!) / span))
    const p = this.points
    const dx = p[j * 3]! - p[i * 3]!
    const dz = p[j * 3 + 2]! - p[i * 3 + 2]!
    out.x = p[i * 3]! + dx * t
    out.y = p[i * 3 + 1]! + (p[j * 3 + 1]! - p[i * 3 + 1]!) * t - LIFT_CHAIR_DROP
    out.z = p[i * 3 + 2]! + dz * t
    out.yaw = Math.atan2(-dx, -dz)
    out.along = along
    out.stretch = this.stretch(along)
    return out
  }

  /** Where rider `seat` (0 on the left) sits on `chair`, `[x, y, z]` metres. */
  place(chair: Chair, seat: number): [number, number, number] {
    const across = (seat - (this.seats - 1) / 2) * LIFT_SEAT_PITCH
    return [chair.x + Math.cos(chair.yaw) * across, chair.y, chair.z - Math.sin(chair.yaw) * across]
  }
}

const blank = (): Chair => ({ x: 0, y: 0, z: 0, yaw: 0, along: 0, stretch: STRETCH_UP })
