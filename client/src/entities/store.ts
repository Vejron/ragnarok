/**
 * Every replicated entity the client has been told of, as the components it has in the game's units,
 * with the recent snapshots of each so it can be drawn between them at the render tick. Fed every scene
 * `Replica` rebuilds; replaces protocol v1's interpolator per family.
 *
 * An entity the newest scene no longer has (it went out of range, burst, or was picked up) ends at that
 * scene's tick: it is drawn until the render tick gets there, so it goes when the picture reaches the
 * moment it went, as a rocket must burst where it was last seen.
 */

import { TICK_RATE } from '@/sim/consts'
import type { Scene } from '@/net/replicated'
import type { Components } from '@/net/components.generated'
import { blend, toGame, type Entity } from './units'

/**
 * Converted entities, by the wire object they came from. An entity that did not change is the same
 * object from one rebuilt scene to the next, so each is converted once however many scenes hold it.
 */
const converted = new WeakMap<Components, Entity>()

function inGameUnits(wire: Components): Entity {
  let entity = converted.get(wire)
  if (!entity) converted.set(wire, (entity = toGame(wire)))
  return entity
}

/** Snapshot history per entity. At 30 Hz this is about a second. */
const MAX_HISTORY = 32
/** Entities missing from snapshots for this many ticks are dropped. */
const STALE_TICKS = 120

interface Track {
  /** Oldest first. */
  samples: { tick: number; entity: Entity }[]
  /** The tick of the first newest scene without it, once there is one. */
  ends: number | null
}

/** An entity drawn at a render tick: its state there, and how it moves across the ground. */
export interface Sampled {
  entity: Entity
  /** Horizontal velocity `[x, z]`, m/s, between the snapshots either side; zero outside them. */
  velocity: [number, number]
}

const STILL: [number, number] = [0, 0]

export class EntityStore {
  private readonly tracks = new Map<number, Track>()
  private newestTick = -Infinity

  /**
   * Takes scene `tick`, as `Replica` rebuilt it. `latest` says it is the newest so far (datagrams can
   * arrive out of order): then whatever it does not have ends at `tick`.
   */
  push(tick: number, scene: Scene, latest: boolean) {
    if (tick <= this.newestTick - STALE_TICKS) return
    this.newestTick = Math.max(this.newestTick, tick)
    for (const [id, wire] of scene) {
      let track = this.tracks.get(id)
      if (!track) this.tracks.set(id, (track = { samples: [], ends: null }))
      const samples = track.samples
      let index = samples.length
      while (index > 0 && samples[index - 1]!.tick > tick) index--
      if (samples[index - 1]?.tick === tick) continue
      samples.splice(index, 0, { tick, entity: inGameUnits(wire) })
      if (samples.length > MAX_HISTORY) samples.shift()
      // Back in range before it was dropped: it carries on.
      if (latest) track.ends = null
    }
    for (const [id, track] of this.tracks) {
      if (latest && track.ends === null && !scene.has(id)) track.ends = tick
      if (track.samples[track.samples.length - 1]!.tick < this.newestTick - STALE_TICKS) this.tracks.delete(id)
    }
  }

  /** Forgets entity `id` at once: a player who left. */
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

  /** The newest state of entity `id`, as the newest snapshot that has it says. */
  newest(id: number): Entity | null {
    const samples = this.tracks.get(id)?.samples
    return samples?.[samples.length - 1]?.entity ?? null
  }

  /** Horizontal velocity `[x, z]` in m/s between the two newest snapshots, or null with fewer. */
  latestVelocity(id: number): [number, number] | null {
    const samples = this.tracks.get(id)?.samples
    if (!samples || samples.length < 2) return null
    const a = samples[samples.length - 2]!
    const b = samples[samples.length - 1]!
    return velocity(a.entity, b.entity, (b.tick - a.tick) / TICK_RATE)
  }

  /** Entity `id` at fractional server tick `tick`, holding its first or newest state outside them. */
  sample(id: number, tick: number): Sampled | null {
    const track = this.tracks.get(id)
    return track ? sampleTrack(track, tick) : null
  }

  /** `sampleAll`'s answer, reused from frame to frame. */
  private readonly sampled = new Map<number, Sampled>()

  /**
   * Every entity at render tick `tick`, by id, ending the ones whose end the picture has reached: they
   * are forgotten and not listed. The map is the store's, and is refilled by the next call.
   */
  sampleAll(tick: number): ReadonlyMap<number, Sampled> {
    const out = this.sampled
    out.clear()
    for (const [id, track] of this.tracks) {
      if (track.ends !== null && tick >= track.ends) {
        this.tracks.delete(id)
        continue
      }
      const sampled = sampleTrack(track, tick)
      if (sampled) out.set(id, sampled)
    }
    return out
  }
}

function velocity(a: Entity, b: Entity, seconds: number): [number, number] {
  if (!a.place || !b.place || seconds <= 0) return [0, 0]
  return [(b.place.x - a.place.x) / seconds, (b.place.z - a.place.z) / seconds]
}

function sampleTrack({ samples }: Track, tick: number): Sampled | null {
  if (samples.length === 0) return null
  const first = samples[0]!
  const last = samples[samples.length - 1]!
  if (tick <= first.tick) return { entity: first.entity, velocity: STILL }
  if (tick >= last.tick) return { entity: last.entity, velocity: STILL }
  let i = samples.length - 1
  while (samples[i - 1]!.tick > tick) i--
  const a = samples[i - 1]!
  const b = samples[i]!
  return { entity: blend(a.entity, b.entity, (tick - a.tick) / (b.tick - a.tick)), velocity: velocity(a.entity, b.entity, (b.tick - a.tick) / TICK_RATE) }
}
