import { describe, expect, it } from 'vitest'
import { Presenters, type Frame } from '@/entities/presenter'
import { Pickups } from '@/entities/presenters/pickups'
import { Soldiers } from '@/entities/presenters/people'
import { Ghosts } from '@/entities/presenters/ghosts'
import { GHOST_BLOCK_ENEMY, GHOST_FORM_TICKS, STANCE_FORM, STANCE_GHOST, STANCE_SWIM, TICK_MS } from '@/sim/consts'
import { Projectiles, type ProjectileWorld } from '@/entities/presenters/projectiles'
import { EntityStore } from '@/entities/store'
import type { Components } from '@/net/components.generated'
import { FLAG_GROUNDED, KIND_CIVILIAN } from '@/net/codec'

const frame = (tick: number): Frame => ({ tick, dt: 1 / 60, now: tick * 16, localId: null })
const rocket = (x: number): Components => ({ projectile: { kind: 1 }, place: { x: x * 100, y: 500, z: 0 }, velocity: { vx: 3000, vy: 0, vz: 0 } })
const item = (x: number): Components => ({ pickup: { item: 3 }, place: { x: x * 100, y: 50, z: 0 } })

/** Stands in for what the projectiles draw into, and records what they are told. */
function projectileWorld() {
  const log: string[] = []
  /** What was heard: launches and the projectiles in flight. */
  const heard: string[] = []
  const world = {
    sounds: {
      launch: (_at: unknown, what: string) => heard.push(`launch ${what}`),
      beginFlights() {},
      flight: (id: number) => heard.push(`flight ${id}`),
      endFlights() {},
      dropFlight: (id: number) => heard.push(`drop ${id}`),
    },
    models: { place: (id: number, _kind: number, [x]: readonly number[]) => log.push(`place ${id} ${x}`), remove: (id: number) => log.push(`remove ${id}`) },
    trails: { follow: (_: string, id: number) => log.push(`trail ${id}`), end: (_: string, id: number) => log.push(`end trail ${id}`) },
    glow: { spawn: () => {} },
    lights: { hold: () => {} },
    fx: () => ({}),
    blips: { fired: (shooter: number, x: number) => log.push(`fired ${shooter} ${x}`) },
  } as unknown as ProjectileWorld
  return { log, heard, world }
}

describe('the projectile presenter', () => {
  it('hears a new one leave once, hears it in flight each frame, and lets its sound go when it ends', () => {
    const store = new EntityStore()
    const presenters = new Presenters<Frame>()
    const { heard, world } = projectileWorld()
    presenters.add(new Projectiles(world))
    store.push(10, new Map([[9, rocket(0)]]), true)
    store.push(12, new Map([[9, rocket(2)]]), true)
    store.push(14, new Map(), true)
    for (const tick of [10, 11, 12, 15]) presenters.run(store.sampleAll(tick), frame(tick))
    // Kind 1 is a grenade: thrown, then heard (or not) in flight, then gone.
    expect(heard.filter((h) => h.startsWith('launch'))).toEqual(['launch grenade'])
    expect(heard.filter((h) => h.startsWith('flight')).length).toBeGreaterThanOrEqual(2)
    expect(heard[heard.length - 1]).toBe('drop 9')
  })

  it('draws a shell in flight, marks it on the map once, and ends it where the picture reaches its end', () => {
    const store = new EntityStore()
    const presenters = new Presenters<Frame>()
    const { log, world } = projectileWorld()
    presenters.add(new Projectiles(world))
    store.push(10, new Map([[9, rocket(0)]]), true)
    store.push(12, new Map([[9, rocket(2)]]), true)
    store.push(14, new Map(), true)
    for (const tick of [11, 12, 13.9, 14]) presenters.run(store.sampleAll(tick), frame(tick))
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
    const presenters = new Presenters<Frame>()
    const drawn: number[][] = []
    const models = { set: (items: { id: number }[]) => drawn.push(items.map((i) => i.id)), update: () => {} }
    const lying = presenters.add(new Pickups(models as never))
    store.push(10, new Map([[4, item(1)], [5, item(2)], [6, rocket(0)]]), true)
    presenters.run(store.sampleAll(10), frame(10))
    expect(lying.lying).toEqual([{ id: 4, item: 3, x: 1, y: 0.5, z: 0 }, { id: 5, item: 3, x: 2, y: 0.5, z: 0 }])
    // Picked up: gone once the picture gets to the snapshot without it.
    store.push(12, new Map([[5, item(2)]]), true)
    presenters.run(store.sampleAll(12), frame(12))
    expect(drawn).toEqual([[4, 5], [5]])
  })
})

describe('the soldiers presenter', () => {
  /** Stands in for a soldier: records what it was last told. */
  const body = () => ({ dead: false, held: 0, pose: [] as unknown[], update(...args: unknown[]) { this.pose = args }, takeSteps: () => [] })

  it('draws everyone else on foot as their flags say, and not the local player', () => {
    const bodies = new Map<number, ReturnType<typeof body>>()
    const avatars = { draw: (id: number) => bodies.get(id) ?? bodies.set(id, body()).get(id)! }
    const shadows: number[][] = []
    const presenters = new Presenters<Frame>()
    const soldiers = presenters.add(new Soldiers({ avatars: avatars as never, blobFoot: (x, y, z) => shadows.push([x, y, z]), rewave: () => 0, current: () => [0, 0], wade: () => {}, skiRun: () => {}, headlamp: () => {}, footfall: () => {}, zombie: () => {} }))
    const store = new EntityStore()
    // Flags: grounded | crouching | holding item 2; lying down. And a dead one, and the local player.
    const crouched: Components = { kind: { kind: 0 }, place: { x: 100, y: 90, z: 0 }, heading: { yaw: 500 }, flags: { flags: 1 | 4 | (2 << 3), stance: 1 }, motion: { vy: 0, knock_x: 900, knock_z: 0 } }
    const dead: Components = { ...crouched, place: { x: 300, y: 90, z: 0 }, flags: { flags: 2, stance: 0 } }
    store.push(10, new Map([[3, crouched], [4, dead], [5, crouched], [6, { ...crouched, kind: { kind: 5 } }]]), true)
    presenters.run(store.sampleAll(10), { ...frame(10), localId: 5 })
    expect(soldiers.inView).toEqual([3, 4])
    expect(soldiers.nearby).toEqual([{ id: 3, x: 1, y: 0.9, z: 0 }, { id: 4, x: 3, y: 0.9, z: 0 }])
    expect(bodies.get(3)).toMatchObject({ dead: false, held: 2 })
    // Position, yaw, velocity, grounded, dt, pitch, crouching, sliding (knocked faster than a slide), prone, chute, vertical speed.
    expect(bodies.get(3)!.pose).toEqual([[1, 0.9, 0], 0.5, 0, 0, true, 1 / 60, 0, true, true, true, false, 0])
    expect(bodies.get(4)!.dead).toBe(true)
    expect(shadows).toEqual([[1, 0.9, 0]])
  })
})

describe('a driver of the traffic', () => {
  it('drawn as a civilian is remembered as one, and a soldier is not', () => {
    const presenters = new Presenters<Frame>()
    const soldiers = presenters.add(new Soldiers({ avatars: { draw: () => ({ update() {}, takeSteps: () => [] }) } as never, blobFoot: () => {}, rewave: () => 0, current: () => [0, 0], wade: () => {}, skiRun: () => {}, headlamp: () => {}, footfall: () => {}, zombie: () => {} }))
    const person = (kind: number): Components => ({ kind: { kind }, place: { x: 0, y: 90, z: 0 }, heading: { yaw: 0 }, flags: { flags: FLAG_GROUNDED, stance: 0 } })
    const store = new EntityStore()
    store.push(10, new Map([[3, person(KIND_CIVILIAN)], [4, person(0)]]), true)
    presenters.run(store.sampleAll(10), frame(10))
    expect([...soldiers.civilians]).toEqual([3])
  })
})

describe('a swimmer in a current', () => {
  it('strokes, and stirs the water, as fast as they go through it, not as fast as it carries them', () => {
    const poses = new Map<number, unknown[]>()
    const avatars = { draw: (id: number) => ({ update(...args: unknown[]) { poses.set(id, args) }, takeSteps: () => [] }) }
    const wakes = new Map<number, number>()
    const presenters = new Presenters<Frame>()
    presenters.add(new Soldiers({ avatars: avatars as never, blobFoot: () => {}, rewave: () => 0, current: () => [2, 0.5], wade: (id, _x, _y, _z, speed) => wakes.set(id, speed), skiRun: () => {}, headlamp: () => {}, footfall: () => {}, zombie: () => {} }))
    // The same move east, once swimming in the river and once walking on its bank.
    const at = (x: number, stance: number): Components => ({ kind: { kind: 0 }, place: { x, y: 0, z: 0 }, heading: { yaw: 0 }, flags: { flags: 0, stance }, motion: { vy: 0, knock_x: 0, knock_z: 0 } })
    const store = new EntityStore()
    store.push(10, new Map([[3, at(0, STANCE_SWIM)], [4, at(0, 0)]]), true)
    store.push(16, new Map([[3, at(50, STANCE_SWIM)], [4, at(50, 0)]]), true)
    presenters.run(store.sampleAll(13), frame(13))
    const [walkX, walkZ] = poses.get(4)!.slice(2, 4) as number[]
    const [swimX, swimZ] = poses.get(3)!.slice(2, 4) as number[]
    expect(walkX).toBeGreaterThan(1)
    expect(swimX).toBeCloseTo(walkX! - 2)
    expect(swimZ).toBeCloseTo(walkZ! - 0.5)
    expect(wakes.get(3)).toBeCloseTo(Math.hypot(swimX!, swimZ!))
  })
})

describe('the ghosts presenter', () => {
  it('draws a ghost as a cloud and not as a soldier, the local one from its prediction, and one taking shape as a swirl', () => {
    const log: string[] = []
    const presenters = new Presenters<Frame>()
    const soldiers = presenters.add(new Soldiers({ avatars: { draw: () => ({ update() {} }) } as never, blobFoot: () => {}, rewave: () => 0, current: () => [0, 0], wade: () => {}, skiRun: () => {}, headlamp: () => {}, footfall: () => {}, zombie: () => {} }))
    presenters.add(new Ghosts({
      draw: {
        flying: ([x], _velocity, _yaw, own, blocked) => log.push(`fly ${x} ${own ? 'own' : 'other'}${blocked ? ' blocked' : ''}`),
        forming: ([x], done) => log.push(`form ${x} ${done.toFixed(2)}`),
      },
      own: () => ({ at: [9, 2, 0], velocity: [0, 0, 0], yaw: 0 }),
      ownBlock: () => GHOST_BLOCK_ENEMY,
    }))
    const ghost = (x: number, stance: number): Components => ({ kind: { kind: 0 }, place: { x: x * 100, y: 300, z: 0 }, heading: { yaw: 0 }, flags: { flags: 2, stance }, motion: { vy: 0, knock_x: 0, knock_z: 0 } })
    const store = new EntityStore()
    store.push(10, new Map([[3, ghost(1, STANCE_GHOST)], [4, ghost(2, STANCE_FORM)], [5, ghost(3, STANCE_GHOST)]]), true)
    presenters.run(store.sampleAll(10), { ...frame(10), localId: 5, now: 1000 })
    expect(soldiers.inView).toEqual([])
    presenters.run(store.sampleAll(10), { ...frame(10), localId: 5, now: 1000 + (GHOST_FORM_TICKS * TICK_MS) / 2 })
    expect(log).toEqual(['fly 1 other', 'form 2 0.00', 'fly 9 own blocked', 'fly 1 other', 'form 2 0.50', 'fly 9 own blocked'])
  })
})
