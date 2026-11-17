import { KIND_SOLDIER } from '@/net/codec'
import { GHOST_BLOCK_NONE, GHOST_FORM_TICKS, STANCE_FORM, STANCE_GHOST, TICK_MS } from '@/sim/consts'
import type { Frame, Presenter } from '../presenter'
import type { Sampled } from '../store'
import type { Entity } from '../units'

/** Whether a soldier's `stance` makes them a ghost, flying or taking shape (GHOSTS.md), and not a body. */
export const ghostly = (stance: number) => stance === STANCE_GHOST || stance === STANCE_FORM

/** How a ghost is drawn (`render/ghost.ts`). */
export interface GhostDrawing {
  flying(at: readonly [number, number, number], velocity: readonly [number, number, number], yaw: number, own: boolean, blocked: boolean, dt: number): void
  forming(at: readonly [number, number, number], done: number, dt: number): void
}

export interface GhostWorld {
  draw: GhostDrawing
  /**
   * The local player's ghost as their prediction has it, a frame ahead of the snapshot: where it is,
   * its velocity `[x, y, z]`, m/s, and its yaw; null when no prediction is running.
   */
  own(): { at: [number, number, number]; velocity: [number, number, number]; yaw: number } | null
  /** Why the local player's ghost may not take shape (`GHOST_BLOCK_*`). */
  ownBlock(): number
}

/**
 * Ghosts, everyone's: a churning shadow where a dead player flies, the local player's own from their
 * prediction, and a swirl where one takes shape. The soldier is not drawn meanwhile (`Soldiers` leaves
 * a ghostly stance alone); once it has taken shape the server says they are alive and standing.
 */
export class Ghosts implements Presenter<Frame> {
  /** When each ghost taking shape was first seen doing so, `performance.now()`. */
  private readonly since = new Map<number, number>()

  constructor(private readonly world: GhostWorld) {}

  wants(entity: Entity) {
    return entity.kind?.kind === KIND_SOLDIER && entity.place !== undefined && entity.flags !== undefined && ghostly(entity.flags.stance)
  }

  update(id: number, { entity, velocity }: Sampled, frame: Frame) {
    const { x, y, z } = entity.place!
    const own = id === frame.localId
    if (entity.flags!.stance === STANCE_FORM) {
      const since = this.since.get(id) ?? frame.now
      this.since.set(id, since)
      this.world.draw.forming([x, y, z], Math.min(1, (frame.now - since) / (GHOST_FORM_TICKS * TICK_MS)), frame.dt)
      return
    }
    this.since.delete(id)
    const predicted = own ? this.world.own() : null
    if (predicted) {
      this.world.draw.flying(predicted.at, predicted.velocity, predicted.yaw, true, this.world.ownBlock() !== GHOST_BLOCK_NONE, frame.dt)
    } else {
      this.world.draw.flying([x, y, z], [velocity[0], entity.motion?.vy ?? 0, velocity[1]], entity.heading?.yaw ?? 0, own, false, frame.dt)
    }
  }

  remove(id: number) {
    this.since.delete(id)
  }
}
