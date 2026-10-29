import { PROJECTILE_BULLET } from '@/net/codec'
import { BULLET_SPEED } from '@/sim/consts'
import type { Rockets } from './rockets'
import type { ProjectileTrails } from './trails'

/** Anything with x, y and z: three's Vector3 or the engine's. */
type V = { x: number; y: number; z: number }

interface Round {
  from: [number, number, number]
  direction: [number, number, number]
  /** How far it goes before it stops: the first thing it meets, or the end of the gun's range. */
  length: number
  /** How far it has flown, m. */
  flown: number
  /** It stops on something solid, and sparks there. */
  hit: boolean
}

/** Mesh ids for rounds, below every server projectile's and bullet's, which are never negative. */
const FIRST_ID = -1

/**
 * A machine gun's rounds in flight, drawn as a rifle's bullet is (the same slug and ribbon, at
 * `BULLET_SPEED`), though the server settled each as a hitscan when it was fired. A round flies from
 * the muzzle to where the client's own ray says it stops; `struck` cuts one short where the server
 * says it hit something the ray does not see, such as a soldier.
 */
export class Rounds {
  private readonly live = new Map<number, Round>()
  private next = FIRST_ID

  constructor(private readonly models: Rockets, private readonly trails: ProjectileTrails, private readonly spark: (at: V) => void) {}

  fire(from: V, to: V, hit: boolean) {
    const d = [to.x - from.x, to.y - from.y, to.z - from.z]
    const length = Math.hypot(d[0]!, d[1]!, d[2]!)
    if (length < 0.01) return
    const id = this.next
    this.next = this.next <= -0x7fffffff ? FIRST_ID : this.next - 1
    const round: Round = { from: [from.x, from.y, from.z], direction: [d[0]! / length, d[1]! / length, d[2]! / length], length, flown: 0, hit }
    this.live.set(id, round)
    // The ribbon starts at the muzzle, so it grows out of the barrel.
    this.trails.follow('m', id, PROJECTILE_BULLET, from.x, from.y, from.z)
  }

  /**
   * The server says a round hit at `at`: the live round whose path passes closest to it, within a
   * metre, stops there and sparks on arrival. With none, it sparks now.
   */
  struck(at: V) {
    let best: Round | null = null
    let bestMiss = 1
    for (const round of this.live.values()) {
      const [fx, fy, fz] = round.from
      const [dx, dy, dz] = round.direction
      const t = (at.x - fx) * dx + (at.y - fy) * dy + (at.z - fz) * dz
      if (t < round.flown - 1 || t > round.length + 1) continue
      const miss = Math.hypot(fx + dx * t - at.x, fy + dy * t - at.y, fz + dz * t - at.z)
      if (miss < bestMiss) {
        bestMiss = miss
        best = round
        best.length = Math.max(t, best.flown)
      }
    }
    if (best) best.hit = true
    else this.spark(at)
  }

  update(dt: number) {
    for (const [id, round] of this.live) {
      round.flown = Math.min(round.length, round.flown + BULLET_SPEED * dt)
      const [fx, fy, fz] = round.from
      const [dx, dy, dz] = round.direction
      const at: [number, number, number] = [fx + dx * round.flown, fy + dy * round.flown, fz + dz * round.flown]
      if (round.flown >= round.length) {
        this.trails.follow('m', id, PROJECTILE_BULLET, ...at)
        this.trails.end('m', id)
        this.models.remove(id)
        this.live.delete(id)
        if (round.hit) this.spark({ x: at[0], y: at[1], z: at[2] })
        continue
      }
      this.models.place(id, PROJECTILE_BULLET, at, round.direction)
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
