import { describe, expect, it } from 'vitest'
import { Presenters, type Frame, type Presenter } from '@/entities/presenter'
import { EntityStore, type Sampled } from '@/entities/store'
import type { Entity } from '@/entities/units'
import type { Components } from '@/net/components.generated'

/** A stickman at `x` metres facing `yaw` radians, as the wire has it. */
const person = (x: number, yaw = 0): Components => ({
  kind: { kind: 0 }, place: { x: Math.round(x * 100), y: 100, z: 0 }, heading: { yaw: Math.round(yaw * 1000) }, flags: { flags: 1, stance: 0 },
})
const scene = (entries: [number, Components][]) => new Map(entries)
const x = (sampled: Sampled | null) => sampled!.entity.place!.x

describe('EntityStore', () => {
  it('blends between the bracketing snapshots', () => {
    const store = new EntityStore()
    store.push(10, scene([[7, person(0)]]), true)
    store.push(12, scene([[7, person(2)]]), true)
    expect(x(store.sample(7, 11))).toBeCloseTo(1)
    expect(store.sample(7, 11)!.velocity[0]).toBeCloseTo(60)
  })

  it('reports the velocity between the two newest snapshots', () => {
    const store = new EntityStore()
    expect(store.latestVelocity(7)).toBeNull()
    store.push(10, scene([[7, person(0)]]), true)
    store.push(12, scene([[7, person(2)]]), true)
    expect(store.latestVelocity(7)![0]).toBeCloseTo(60)
  })

  it('accepts out-of-order snapshots', () => {
    const store = new EntityStore()
    store.push(10, scene([[7, person(0)]]), true)
    store.push(14, scene([[7, person(4)]]), true)
    store.push(12, scene([[7, person(1)]]), false)
    expect(x(store.sample(7, 13))).toBeCloseTo(2.5)
  })

  it('holds the newest state past the end', () => {
    const store = new EntityStore()
    store.push(10, scene([[7, person(0)]]), true)
    store.push(12, scene([[7, person(2)]]), true)
    expect(x(store.sample(7, 20))).toBe(2)
    expect(store.newest(7)!.place!.x).toBe(2)
  })

  it('takes the short way round for yaw', () => {
    const store = new EntityStore()
    store.push(10, scene([[7, person(0, 3.0)]]), true)
    store.push(12, scene([[7, person(0, -3.0)]]), true)
    const yaw = store.sample(7, 11)!.entity.heading!.yaw
    expect(Math.abs(Math.abs(yaw) - Math.PI)).toBeLessThan(0.01)
  })

  it('ends what the newest scene no longer has when the picture gets there', () => {
    const store = new EntityStore()
    store.push(10, scene([[7, person(0)], [8, person(5)]]), true)
    store.push(12, scene([[7, person(1)]]), true)
    // An older scene arriving late says nothing about what is gone.
    store.push(11, scene([[7, person(0.5)]]), false)
    expect([...store.sampleAll(11.9).keys()]).toEqual([7, 8])
    expect([...store.sampleAll(12).keys()]).toEqual([7])
    expect(store.ids()).toEqual([7])
  })

  it('carries on with what comes back before the picture reached its end', () => {
    const store = new EntityStore()
    store.push(10, scene([[7, person(0)]]), true)
    store.push(12, scene([]), true)
    store.push(14, scene([[7, person(4)]]), true)
    expect(store.sampleAll(13).has(7)).toBe(true)
    expect(store.sampleAll(20).has(7)).toBe(true)
  })

  it('converts an entity once, however many scenes hold it unchanged', () => {
    const store = new EntityStore()
    const standing = person(3)
    store.push(10, scene([[7, standing]]), true)
    store.push(12, scene([[7, standing]]), true)
    expect(store.sample(7, 10)!.entity).toBe(store.sample(7, 12)!.entity)
    // And blending it with itself is no work at all.
    expect(store.sample(7, 11)!.entity).toBe(store.sample(7, 10)!.entity)
  })

  it('drops entities that stop appearing for long', () => {
    const store = new EntityStore()
    store.push(10, scene([[7, person(0)]]), true)
    store.push(500, scene([]), true)
    expect(store.ids()).toEqual([])
  })
})

/** Records what it is told. */
class Recorder implements Presenter<Frame> {
  readonly log: string[] = []
  constructor(private readonly want: (e: Entity) => boolean) {}
  wants(e: Entity) {
    return this.want(e)
  }
  update(id: number) {
    this.log.push(`update ${id}`)
  }
  remove(id: number) {
    this.log.push(`remove ${id}`)
  }
}

describe('Presenters', () => {
  const frame = (tick: number): Frame => ({ tick, dt: 1 / 60, now: 0, localId: null })

  it('draw what each is for, and let go once when it goes or stops being what they are for', () => {
    const store = new EntityStore()
    const presenters = new Presenters<Frame>()
    const standing = presenters.add(new Recorder((e) => e.flags!.flags === 1))
    const everyone = presenters.add(new Recorder(() => true))
    store.push(10, scene([[7, person(0)], [8, person(1)]]), true)
    presenters.run(store.sampleAll(10), frame(10))
    // 8 falls over, and 7 goes out of range.
    store.push(12, scene([[8, { ...person(1), flags: { flags: 0, stance: 0 } }]]), true)
    presenters.run(store.sampleAll(12), frame(12))
    presenters.run(store.sampleAll(13), frame(13))
    expect(standing.log).toEqual(['update 7', 'update 8', 'remove 8', 'remove 7'])
    expect(everyone.log).toEqual(['update 7', 'update 8', 'update 8', 'remove 7', 'update 8'])
    presenters.clear()
    expect(everyone.log[everyone.log.length - 1]).toBe('remove 8')
  })
})

/** A full room's scene `n` snapshots in: 64 stickmen, 20 vehicles and 30 rockets, all moving. */
function room(n: number): Map<number, Components> {
  const entries: [number, Components][] = []
  for (let i = 1; i <= 64; i++) entries.push([i, { ...person(i + n * 0.25, (i + n) * 0.01), motion: { vy: 0, knock_x: 0, knock_z: 0 } }])
  for (let i = 0; i < 20; i++) {
    entries.push([100 + i, {
      kind: { kind: 2 }, place: { x: i * 500 + n * 30, y: 120, z: 0 }, orientation: { qx: 0, qy: 1000 + n * 10, qz: 0 }, steer: { steer: n % 50 },
      turret: { yaw: n * 5, pitch: 100, shots: n, range: 0, flags: 0 }, health: { hp: 200 },
    }])
  }
  for (let i = 0; i < 30; i++) entries.push([200 + i, { projectile: { kind: 0 }, place: { x: i * 100 + n * 800, y: 2000, z: 0 }, velocity: { vx: 24000, vy: 0, vz: 0 } }])
  return new Map(entries)
}

describe('a full room', () => {
  it('costs the store a small share of a second to take in and draw for a second', () => {
    // A second of play: 30 snapshots taken in, and 60 frames drawn between them.
    const scenes = Array.from({ length: 30 }, (_, n) => room(n))
    const second = () => {
      const store = new EntityStore()
      const start = performance.now()
      for (let n = 0; n < 30; n++) {
        store.push(2 * n, scenes[n]!, true)
        store.sampleAll(2 * n - 3.5)
        store.sampleAll(2 * n - 2.5)
      }
      return performance.now() - start
    }
    for (let i = 0; i < 5; i++) second()
    const best = Math.min(...Array.from({ length: 7 }, second))
    console.log(`a full room for a second: store ${best.toFixed(2)} ms`)
    // Built and run by Node alone it is about 0.6 ms, under vitest about 1 (MIGRATION.md, 4.2); the
    // adapter and interpolators it replaced took about 1.0 ms. This catches a real step back.
    expect(best).toBeLessThan(3)
  })
})
