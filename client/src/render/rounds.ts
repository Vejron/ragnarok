import { PROJECTILE_BULLET } from '@/net/codec'
import { BULLET_GRAVITY_SCALE, BULLET_SPEED, GRAVITY } from '@/sim/consts'
import type { Rockets } from './rockets'
import type { ProjectileTrails } from './trails'

/** Anything with x, y and z: three's Vector3 or the engine's. */
type V = { x: number; y: number; z: number }
type CastRay = (origin: [number, number, number], direction: [number, number, number], max: number) => { t: number } | null

/** Mesh ids for rounds, below every server projectile's and bullet's, which are never negative. */
const FIRST_ID = -1
/** A round falls under this, m/s², as a hand gun's bullet and the server's mounted round do. */
const FALL = GRAVITY * BULLET_GRAVITY_SCALE
/** How long each straight piece of the arc is that `arcTime` casts a ray along, seconds. */
const ARC_STEP = 0.05

interface Round {
  from: [number, number, number]
  /** Unit heading it leaves on, at `BULLET_SPEED`. */
  direction: [number, number, number]
  /** How long it flies before it stops: at the first thing it meets, or the end of the gun's range, s. */
  time: number
  /** How long it has flown, s. */
  flown: number
  /** It stops on something solid, and sparks there. */
  hit: boolean
}

/** Where a round from `from` heading along the unit `direction` is `t` seconds on. */
function arcAt(from: readonly number[], direction: readonly number[], t: number): [number, number, number] {
  const run = BULLET_SPEED * t
  return [from[0]! + direction[0]! * run, from[1]! + direction[1]! * run - 0.5 * FALL * t * t, from[2]! + direction[2]! * run]
}

/**
 * How long a round from `from` heading along the unit `direction` flies before it meets the world or
 * has gone `range` metres, as the server flies it, and whether it met something: the arc is cast a
 * short straight piece at a time.
 */
export function arcTime(castRay: CastRay, from: V, direction: V, range: number): { time: number; hit: boolean } {
  const start = [from.x, from.y, from.z]
  const heading = [direction.x, direction.y, direction.z]
  const end = range / BULLET_SPEED
  for (let t = 0; t < end; t += ARC_STEP) {
    const next = Math.min(end, t + ARC_STEP)
    const a = arcAt(start, heading, t)
    const b = arcAt(start, heading, next)
    const d: [number, number, number] = [b[0] - a[0], b[1] - a[1], b[2] - a[2]]
    const length = Math.hypot(...d)
    const met = castRay(a, [d[0] / length, d[1] / length, d[2] / length], length)
    if (met) return { time: t + (next - t) * (met.t / length), hit: true }
  }
  return { time: end, hit: false }
}

/**
 * A machine gun's rounds in flight, drawn as a rifle's bullet is (the same slug and ribbon), on the
 * arc the server flies them on: at `BULLET_SPEED`, falling under `FALL`. A round flies from the muzzle
 * for as long as the client's own rays say it goes (`arcTime`); `struck` cuts one short where the
 * server says it hit something the rays do not see, such as a soldier.
 */
export class Rounds {
  private readonly live = new Map<number, Round>()
  private next = FIRST_ID

  constructor(private readonly models: Rockets, private readonly trails: ProjectileTrails, private readonly spark: (at: V) => void) {}

  /** A round from `from` along the unit `direction`, for `time` seconds, sparking at the end if it `hit` something. */
  fire(from: V, direction: V, time: number, hit: boolean) {
    if (time <= 0) return
    const id = this.next
    this.next = this.next <= -0x7fffffff ? FIRST_ID : this.next - 1
    this.live.set(id, { from: [from.x, from.y, from.z], direction: [direction.x, direction.y, direction.z], time, flown: 0, hit })
    // The ribbon starts at the muzzle, so it grows out of the barrel.
    this.trails.follow('m', id, PROJECTILE_BULLET, from.x, from.y, from.z)
  }

  /**
   * The server says a round hit at `at`: the live round whose arc passes closest to it, within a metre,
   * stops there and sparks on arrival. With none, it sparks now.
   */
  struck(at: V) {
    let best: Round | null = null
    let bestMiss = 1
    let bestTime = 0
    for (const round of this.live.values()) {
      const [dx, , dz] = round.direction
      const across = Math.hypot(dx, dz) * BULLET_SPEED
      // When it is abreast of the point, across the ground (or along the way, for a round straight up or down).
      const [fx, fy, fz] = round.from
      const t = across > 1 ? ((at.x - fx) * dx + (at.z - fz) * dz) / Math.hypot(dx, dz) / across : (at.y - fy) / (round.direction[1] * BULLET_SPEED)
      if (t < round.flown - 1 / BULLET_SPEED || t > round.time + 1 / BULLET_SPEED) continue
      const on = arcAt(round.from, round.direction, t)
      const miss = Math.hypot(on[0] - at.x, on[1] - at.y, on[2] - at.z)
      if (miss < bestMiss) {
        bestMiss = miss
        best = round
        bestTime = t
      }
    }
    if (best) {
      best.time = Math.max(bestTime, best.flown)
      best.hit = true
    } else this.spark(at)
  }

  update(dt: number) {
    for (const [id, round] of this.live) {
      round.flown = Math.min(round.time, round.flown + dt)
      const at = arcAt(round.from, round.direction, round.flown)
      if (round.flown >= round.time) {
        this.trails.follow('m', id, PROJECTILE_BULLET, ...at)
        this.trails.end('m', id)
        this.models.remove(id)
        this.live.delete(id)
        if (round.hit) this.spark({ x: at[0], y: at[1], z: at[2] })
        continue
      }
      const [dx, dy, dz] = round.direction
      const heading: [number, number, number] = [dx * BULLET_SPEED, dy * BULLET_SPEED - FALL * round.flown, dz * BULLET_SPEED]
      this.models.place(id, PROJECTILE_BULLET, at, heading)
      this.trails.follow('m', id, PROJECTILE_BULLET, ...at)
    }
  }

  /** How many are in flight, for tests. */
  get count() {
    return this.live.size
  }

  clear() {
    for (const id of this.live.keys()) {
      this.trails.end('m', id)
      this.models.remove(id)
    }
    this.live.clear()
  }
}
