/**
 * Presenters draw entities: each says which entities it is for, by the components they have, and keeps
 * whatever it needs per entity (a model, a trail, a sound) itself. `Presenters` runs a group of them
 * over the store as sampled at the render tick, and tells each once when an entity it drew has gone,
 * or no longer has what it is for, so that per-entity state is cleaned up in one place.
 *
 * The store is sampled once a frame (`EntityStore.sampleAll`), and the groups run from it at their
 * places in the frame: what is drawn in flight before the lights are updated, people where the
 * contact shadows are gathered, and so on.
 */

import type { Entity } from './units'
import type { Sampled } from './store'

/** What every presenter is told each frame. */
export interface Frame {
  /** The server tick the picture shows, fractional. */
  tick: number
  /** Seconds since the last frame. */
  dt: number
  /** `performance.now()` of this frame. */
  now: number
  /** The local player's id, once the server has given one. */
  localId: number | null
}

export interface Presenter<F> {
  /** Whether it draws `entity`. */
  wants(entity: Entity, id: number, frame: F): boolean
  /** Before the frame's entities, if it needs to. */
  begin?(frame: F): void
  /** Draws entity `id` as it is at the render tick. */
  update(id: number, sampled: Sampled, frame: F): void
  /** After the frame's entities, if it needs to. */
  end?(frame: F): void
  /** Entity `id` is gone, or no longer what it is for: it lets go of whatever it held for it. */
  remove(id: number): void
}

export class Presenters<F> {
  private readonly drawn = new Map<Presenter<F>, Set<number>>()

  /** Adds `presenter`, run after those added before it. */
  add<P extends Presenter<F>>(presenter: P): P {
    this.drawn.set(presenter, new Set())
    return presenter
  }

  /** Draws `sampled`, the store at the render tick, presenter by presenter in the order they were added. */
  run(sampled: ReadonlyMap<number, Sampled>, frame: F) {
    for (const [presenter, drawn] of this.drawn) {
      presenter.begin?.(frame)
      for (const [id, state] of sampled) {
        if (presenter.wants(state.entity, id, frame)) {
          drawn.add(id)
          presenter.update(id, state, frame)
        } else if (drawn.delete(id)) {
          presenter.remove(id)
        }
      }
      for (const id of drawn) {
        if (!sampled.has(id)) {
          drawn.delete(id)
          presenter.remove(id)
        }
      }
      presenter.end?.(frame)
    }
  }

  /** Lets every presenter go of everything: a reconnect. */
  clear() {
    for (const [presenter, drawn] of this.drawn) {
      for (const id of drawn) presenter.remove(id)
      drawn.clear()
    }
  }
}
