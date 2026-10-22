/**
 * Presenters draw entities: each says which entities it is for, by the components they have, and keeps
 * whatever it needs per entity (a model, a trail, a sound) itself. `Presenters` runs them every frame
 * over the store sampled at the render tick, and tells each once when an entity it drew has gone, or no
 * longer has what it is for, so that per-entity state is cleaned up in one place.
 */

import type { Entity } from './units'
import type { EntityStore, Sampled } from './store'

/** What every presenter is told each frame. */
export interface Frame {
  /** The server tick the picture shows, fractional. */
  tick: number
  /** Seconds since the last frame. */
  dt: number
  /** `performance.now()` of this frame. */
  now: number
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

  constructor(private readonly store: EntityStore) {}

  /** Adds `presenter`, run after those added before it. */
  add<P extends Presenter<F>>(presenter: P): P {
    this.drawn.set(presenter, new Set())
    return presenter
  }

  /** Draws everything at render tick `tick`, presenter by presenter in the order they were added. */
  run(tick: number, frame: F) {
    const sampled = this.store.sampleAll(tick)
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
