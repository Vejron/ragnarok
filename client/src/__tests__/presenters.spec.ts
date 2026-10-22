import { describe, expect, it } from 'vitest'
import { Presenters, type Frame } from '@/entities/presenter'
import { Pickups } from '@/entities/presenters/pickups'
import { Projectiles, type ProjectileWorld } from '@/entities/presenters/projectiles'
import { EntityStore } from '@/entities/store'
import type { Components } from '@/net/components.generated'

const frame = (tick: number): Frame => ({ tick, dt: 1 / 60, now: tick * 16 })
const rocket = (x: number): Components => ({ projectile: { kind: 1 }, place: { x: x * 100, y: 500, z: 0 }, velocity: { vx: 3000, vy: 0, vz: 0 } })
const item = (x: number): Components => ({ pickup: { item: 3 }, place: { x: x * 100, y: 50, z: 0 } })

/** Stands in for what the projectiles draw into, and records what they are told. */
function projectileWorld() {
  const log: string[] = []
  const world = {
    models: { place: (id: number, _kind: number, [x]: readonly number[]) => log.push(`place ${id} ${x}`), remove: (id: number) => log.push(`remove ${id}`) },
    trails: { follow: (_: string, id: number) => log.push(`trail ${id}`), end: (_: string, id: number) => log.push(`end trail ${id}`) },
    glow: { spawn: () => {} },
    lights: { hold: () => {} },
    fx: () => ({}),
    blips: { fired: (shooter: number, x: number) => log.push(`fired ${shooter} ${x}`) },
  } as unknown as ProjectileWorld
  return { log, world }
}

describe('the projectile presenter', () => {
  it('draws a shell in flight, marks it on the map once, and ends it where the picture reaches its end', () => {
    const store = new EntityStore()
    const presenters = new Presenters<Frame>(store)
    const { log, world } = projectileWorld()
    presenters.add(new Projectiles(world))
    store.push(10, new Map([[9, rocket(0)]]), true)
    store.push(12, new Map([[9, rocket(2)]]), true)
    store.push(14, new Map(), true)
    for (const tick of [11, 12, 13.9, 14]) presenters.run(tick, frame(tick))
    expect(log).toEqual([
      'fired 0 1', 'place 9 1', 'trail 9',
      'place 9 2', 'trail 9',
      'place 9 2', 'trail 9',
      'remove 9', 'end trail 9',
    ])
  })
})

describe('the item presenter', () => {
  it('draws what lies about this frame, and lists it for the prompt', () => {
    const store = new EntityStore()
    const presenters = new Presenters<Frame>(store)
    const drawn: number[][] = []
    const models = { set: (items: { id: number }[]) => drawn.push(items.map((i) => i.id)), update: () => {} }
    const lying = presenters.add(new Pickups(models as never))
    store.push(10, new Map([[4, item(1)], [5, item(2)], [6, rocket(0)]]), true)
    presenters.run(10, frame(10))
    expect(lying.lying).toEqual([{ id: 4, item: 3, x: 1, y: 0.5, z: 0 }, { id: 5, item: 3, x: 2, y: 0.5, z: 0 }])
    // Picked up: gone once the picture gets to the snapshot without it.
    store.push(12, new Map([[5, item(2)]]), true)
    presenters.run(12, frame(12))
    expect(drawn).toEqual([[4, 5], [5]])
  })
})
