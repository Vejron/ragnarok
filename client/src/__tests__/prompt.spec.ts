import { describe, expect, it } from 'vitest'
import type { Placed } from '@/entities/presenters/vehicles'
import { Events, type EventWorld } from '@/events'
import { KIND_DRONE, KIND_HEN, KIND_JEEP, KIND_TANK, WEAPON_BULLET, type BulletSpawn } from '@/net/codec'
import { itemInReach, prompt, type Where } from '@/prompt'
import { ITEM_AK5, ITEM_GRENADE, ITEM_SHOTGUN } from '@/sim/items'
import { WALKER_LEG_LENGTH } from '@/sim/consts'

const standing = (x: number, extra: Partial<Where> = {}): Where => ({ position: [x, 1, 0], ground: 0, alive: true, grounded: true, chute: false, seat: null, ...extra })
const car = (kind: number, x: number, seats: number[], y = 1): Placed => ({
  kind, x, y, z: 0, yaw: 0, rotation: [0, 0, 0, 1], steer: 0, seats, turret: null, arms: null, crew: [], hp: 100, damage: [0, 0, 0, 0, 0, 0, 0, 0], jets: 0, air: null,
})
const cars = (...list: Placed[]) => new Map(list.map((placed, i) => [i + 1, placed]))

describe('the E prompt', () => {
  it('offers the lowest free seat of the nearest vehicle in reach', () => {
    expect(prompt(standing(0), [], cars(car(KIND_TANK, 2, [0, 0])))).toBe('E: drive the tank')
    expect(prompt(standing(0), [], cars(car(KIND_TANK, 2, [7, 0])))).toBe('E: man the gun')
    expect(prompt(standing(0), [], cars(car(KIND_TANK, 2, [7, 8])))).toBeNull()
    expect(prompt(standing(0), [], cars(car(KIND_TANK, 3.5, [0, 0]), car(KIND_JEEP, -1, [7, 0, 0, 0])))).toBe('E: ride along')
    expect(prompt(standing(0), [], cars(car(KIND_JEEP, 9, [0, 0, 0, 0])))).toBeNull()
  })

  it('never offers a drone, which is flown from afar, nor a way out of one', () => {
    expect(prompt(standing(0), [], cars(car(KIND_DRONE, 1, [0])))).toBeNull()
    expect(prompt(standing(0, { seat: { kind: KIND_DRONE, index: 0 } }), [], cars())).toBeNull()
    expect(prompt(standing(0, { seat: { kind: KIND_JEEP, index: 2 } }), [], cars())).toBe('E: get out')
  })

  it('boards a hen from between its feet', () => {
    const hen = car(KIND_HEN, 0, [0], 1 + WALKER_LEG_LENGTH)
    expect(prompt(standing(0.5), [], cars(hen))).toBe('E: drive the hen')
    expect(prompt(standing(0.5, { position: [0.5, 1 + WALKER_LEG_LENGTH, 0] }), [], cars(hen))).toBeNull()
  })

  it('hands over an item before a seat, and offers the parachute before either while falling', () => {
    const lying = [{ id: 9, item: ITEM_GRENADE, x: 0.5, y: 1, z: 0 }]
    expect(prompt(standing(0), lying, cars(car(KIND_JEEP, 1, [0, 0, 0, 0])))).toBe('E: take Grenade')
    const falling = standing(0, { grounded: false, position: [0, 30, 0] })
    expect(prompt(falling, lying, cars())).toBe('G: open the parachute')
    expect(prompt({ ...falling, chute: true }, lying, cars())).toBe('G: cut the parachute')
  })

  it('takes the nearest item within reach and on the same level', () => {
    const lying = [{ id: 1, item: ITEM_AK5, x: 1, y: 1, z: 0 }, { id: 2, item: ITEM_SHOTGUN, x: 0.5, y: 1, z: 0 }, { id: 3, item: ITEM_AK5, x: 0.1, y: 5, z: 0 }]
    expect(itemInReach(lying, [0, 1, 0])?.id).toBe(2)
  })
})

describe('bullets', () => {
  /** Events drawn into stand-ins that record what is placed and ended. */
  function events() {
    const log: string[] = []
    const world = {
      rockets: { place: (id: number) => log.push(`place ${id}`), remove: (id: number) => log.push(`remove ${id}`) },
      trails: { follow() {}, end() {} },
      avatars: { of: () => undefined },
      effects: { pelletStrike() {}, burst() {}, muzzle() {} },
      sounds: { gunshot: () => log.push('bang') },
      blips: { fired() {} },
      surroundings: { watery: () => false },
      view: { chipped() {} },
      localId: () => null,
    } as unknown as EventWorld
    return { log, events: new Events(world) }
  }
  const spawn: BulletSpawn = { id: 4, shooter: 3, item: ITEM_AK5, x: 0, y: 1.5, z: 0, vx: 0, vy: 0, vz: -300, tick: 100 }

  it('start once though each is sent twice, and end where the server says it struck', () => {
    const { log, events: e } = events()
    e.receive(100, [], [spawn])
    e.receive(102, [{ shooter: 4, weapon: WEAPON_BULLET, x: 0, y: 1.5, z: -20 }], [spawn])
    e.drawBullets(101)
    e.play(102)
    e.drawBullets(103)
    // 20 m at 300 m/s is 4 ticks: drawn at 101 and 103, gone by 104.
    e.drawBullets(104.5)
    expect(log).toEqual(['bang', 'place 4', 'place 4', 'remove 4'])
  })
})
