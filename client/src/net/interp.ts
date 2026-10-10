import type { EntityState, RocketState, VehicleState } from './codec'
import { wrapAngle } from './codec'
import { TICK_RATE } from '@/sim/consts'

/** Snapshot history per entity. At 30 Hz this is about a second. */
const MAX_HISTORY = 32
/** Entities missing from snapshots for this many ticks are dropped. */
const STALE_TICKS = 120

/** What the interpolator needs to know about a state; the rest is the blend function's business. */
export interface Tracked {
  id: number
  x: number
  y: number
  z: number
}

interface Sample<T> {
  tick: number
  state: T
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t
/** Shortest-arc angle interpolation. */
const lerpAngle = (a: number, b: number, t: number) => a + wrapAngle(b - a) * t

export function blendEntity(a: EntityState, b: EntityState, t: number): EntityState {
  return {
    ...b,
    x: lerp(a.x, b.x, t),
    y: lerp(a.y, b.y, t),
    z: lerp(a.z, b.z, t),
    yaw: lerpAngle(a.yaw, b.yaw, t),
    vy: lerp(a.vy, b.vy, t),
    knockX: lerp(a.knockX, b.knockX, t),
    knockZ: lerp(a.knockZ, b.knockZ, t),
    grounded: t < 0.5 ? a.grounded : b.grounded,
    dead: t < 0.5 ? a.dead : b.dead,
  }
}

export function blendRocket(a: RocketState, b: RocketState, t: number): RocketState {
  return {
    id: b.id,
    x: lerp(a.x, b.x, t),
    y: lerp(a.y, b.y, t),
    z: lerp(a.z, b.z, t),
    vx: lerp(a.vx, b.vx, t),
    vy: lerp(a.vy, b.vy, t),
    vz: lerp(a.vz, b.vz, t),
  }
}

/** Blends orientations by normalised lerp, which is plenty for two snapshots 33 ms apart. */
export function blendVehicle(a: VehicleState, b: VehicleState, t: number): VehicleState {
  // q and -q are the same rotation: take the short way round.
  const flip = a.qx * b.qx + a.qy * b.qy + a.qz * b.qz + a.qw * b.qw < 0 ? -1 : 1
  const qx = lerp(a.qx, b.qx * flip, t)
  const qy = lerp(a.qy, b.qy * flip, t)
  const qz = lerp(a.qz, b.qz * flip, t)
  const qw = lerp(a.qw, b.qw * flip, t)
  const length = Math.hypot(qx, qy, qz, qw) || 1
  return {
    ...b,
    x: lerp(a.x, b.x, t),
    y: lerp(a.y, b.y, t),
    z: lerp(a.z, b.z, t),
    qx: qx / length, qy: qy / length, qz: qz / length, qw: qw / length,
    steer: lerp(a.steer, b.steer, t),
    hp: t < 0.5 ? a.hp : b.hp,
    turret: a.turret && b.turret
      ? {
          yaw: lerpAngle(a.turret.yaw, b.turret.yaw, t),
          pitch: lerp(a.turret.pitch, b.turret.pitch, t),
          // A shot is an event, not a quantity: take the nearer snapshot's counter.
          shots: t < 0.5 ? a.turret.shots : b.turret.shots,
          range: t < 0.5 ? a.turret.range : b.turret.range,
        }
      : b.turret,
  }
}

/**
 * Snapshot interpolation for remote entities: states are buffered by server tick and drawn at
 * a render tick slightly in the past, so there are (almost) always two snapshots to blend.
 */
export class Interpolator<T extends Tracked> {
  private tracks = new Map<number, Sample<T>[]>()
  private newestTick = -Infinity

  constructor(private readonly blend: (a: T, b: T, t: number) => T) {}

  push(tick: number, entities: readonly T[]) {
    if (tick <= this.newestTick - STALE_TICKS) return
    this.newestTick = Math.max(this.newestTick, tick)
    for (const state of entities) {
      let track = this.tracks.get(state.id)
      if (!track) this.tracks.set(state.id, (track = []))
      // Datagrams can arrive out of order: insert sorted, ignore duplicates.
      let index = track.length
      while (index > 0 && track[index - 1]!.tick > tick) index--
      if (track[index - 1]?.tick === tick) continue
      track.splice(index, 0, { tick, state })
      if (track.length > MAX_HISTORY) track.shift()
    }
    for (const [id, track] of this.tracks) {
      if (track[track.length - 1]!.tick < this.newestTick - STALE_TICKS) this.tracks.delete(id)
    }
  }

  remove(id: number) {
    this.tracks.delete(id)
  }

  clear() {
    this.tracks.clear()
    this.newestTick = -Infinity
  }

  ids() {
    return [...this.tracks.keys()]
  }

  /** Horizontal velocity `[x, z]` in m/s between the two newest snapshots, or null with fewer. */
  latestVelocity(id: number): [number, number] | null {
    const track = this.tracks.get(id)
    if (!track || track.length < 2) return null
    const a = track[track.length - 2]!
    const b = track[track.length - 1]!
    const seconds = (b.tick - a.tick) / TICK_RATE
    return [(b.state.x - a.state.x) / seconds, (b.state.z - a.state.z) / seconds]
  }

  /** The entity's state at a fractional server tick. Holds the newest state past the end. */
  sample(id: number, tick: number): (T & { speed: number }) | null {
    const track = this.tracks.get(id)
    if (!track || track.length === 0) return null
    const first = track[0]!
    const last = track[track.length - 1]!
    if (tick <= first.tick) return { ...first.state, speed: 0 }
    if (tick >= last.tick) return { ...last.state, speed: 0 }
    let i = track.length - 1
    while (track[i - 1]!.tick > tick) i--
    const a = track[i - 1]!
    const b = track[i]!
    const t = (tick - a.tick) / (b.tick - a.tick)
    const speed = Math.hypot(b.state.x - a.state.x, b.state.z - a.state.z) / ((b.tick - a.tick) / TICK_RATE)
    return { ...this.blend(a.state, b.state, t), speed }
  }
}
