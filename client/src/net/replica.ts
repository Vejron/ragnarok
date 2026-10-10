import { applyDelta, type WireDelta, type WireScene } from './codec'
import { BASELINE_WINDOW_CLIENT } from '@/sim/consts'

const EMPTY: WireScene = { entities: [], vehicles: [] }

/**
 * The client's half of delta compression. Snapshots list only what changed since a baseline, so
 * the client keeps the scenes it has rebuilt and applies each new delta to the one it names. The
 * newest tick held is sent back with every input, and the server diffs against that.
 */
export class Replica {
  private readonly held = new Map<number, WireScene>()
  private newest = 0

  /** The tick of the newest snapshot rebuilt, to acknowledge: 0 until there is one. */
  get baseline() {
    return this.newest
  }

  /**
   * Rebuilds the scene of snapshot `tick` from `delta` on top of snapshot `baseline`, and keeps it.
   * Returns null when it names a baseline this client does not hold (it was never received, or is
   * too old), in which case the snapshot is dropped and the server, which sees no acknowledgement
   * of it, will fall back to a whole one. `latest` says whether this is the newest snapshot so
   * far, as datagrams can arrive out of order.
   */
  receive(tick: number, baseline: number, delta: WireDelta): { scene: WireScene; latest: boolean } | null {
    const base = baseline === 0 ? EMPTY : this.held.get(baseline)
    if (!base) return null
    const scene = applyDelta(base, delta)
    this.held.set(tick, scene)
    const latest = tick > this.newest
    if (latest) this.newest = tick
    while (this.held.size > BASELINE_WINDOW_CLIENT) {
      let oldest = Infinity
      for (const held of this.held.keys()) oldest = Math.min(oldest, held)
      this.held.delete(oldest)
    }
    return { scene, latest }
  }

  /** Forgets everything, for a new connection: the server's ticks and baselines start over. */
  reset() {
    this.held.clear()
    this.newest = 0
  }
}
