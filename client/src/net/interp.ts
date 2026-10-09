import type { EntityState } from './codec'
import { wrapAngle } from './codec'
import { TICK_RATE } from '@/sim/consts'

/** Snapshot history per entity. At 30 Hz this is about a second. */
const MAX_HISTORY = 32
/** Entities missing from snapshots for this many ticks are dropped. */
const STALE_TICKS = 120

interface Sample {
  tick: number
  state: EntityState
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t
/** Shortest-arc angle interpolation. */
const lerpAngle = (a: number, b: number, t: number) => a + wrapAngle(b - a) * t

/**
 * Snapshot interpolation for remote entities: states are buffered by server tick and drawn at
 * a render tick slightly in the past, so there are (almost) always two snapshots to blend.
 */
export class Interpolator {
  private tracks = new Map<number, Sample[]>()
  private newestTick = -Infinity

  push(tick: number, entities: readonly EntityState[]) {
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

  /** The entity's state at a fractional server tick. Holds the newest state past the end. */
  sample(id: number, tick: number): (EntityState & { speed: number }) | null {
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
    return {
      ...b.state,
      x: lerp(a.state.x, b.state.x, t),
      y: lerp(a.state.y, b.state.y, t),
      z: lerp(a.state.z, b.state.z, t),
      yaw: lerpAngle(a.state.yaw, b.state.yaw, t),
      vy: lerp(a.state.vy, b.state.vy, t),
      grounded: t < 0.5 ? a.state.grounded : b.state.grounded,
      speed,
    }
  }
}
