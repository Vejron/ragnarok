import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Group, Vec3 } from '@ragnarok/engine'
import { beforeAll, describe, expect, it } from 'vitest'
import { Presenters, type Frame } from '@/entities/presenter'
import { Headlights } from '@/entities/presenters/headlights'
import { Seating } from '@/entities/presenters/seating'
import { Vehicles, type VehicleWorld } from '@/entities/presenters/vehicles'
import { EntityStore } from '@/entities/store'
import { FLAG_LAMP, KIND_AIR_AMBULANCE, KIND_HELI, KIND_JEEP, KIND_MORTAR, KIND_SPORTS, KIND_TANK } from '@/net/codec'
import type { Components } from '@/net/components.generated'
import { parseAirAmbulanceModel } from '@/render/airambulance'
import { parseCarModel } from '@/render/car'
import { parseHeliModel } from '@/render/heli'
import { DAY_SECONDS } from '@/render/daynight'
import { Particles } from '@/render/particles'
import { Skidmarks } from '@/render/skidmarks'
import { parseTankModel } from '@/render/tank'
import { TICK_RATE } from '@/sim/consts'

beforeAll(async () => {
  for (const name of ['jeep', 'sports', 'tank', 'heli', 'air_ambulance'] as const) {
    const bytes = readFileSync(resolve(process.cwd(), `public/models/${name}.glb`))
    const copy = new Uint8Array(bytes.length)
    copy.set(bytes)
    if (name === 'tank') await parseTankModel(copy.buffer)
    else if (name === 'air_ambulance') await parseAirAmbulanceModel(copy.buffer)
    else if (name === 'heli') await parseHeliModel(copy.buffer)
    else await parseCarModel(name, copy.buffer)
  }
})

/** Ticks from the server's start (noon) to midnight and to the next noon. */
const MIDNIGHT = (DAY_SECONDS / 2) * TICK_RATE
const NOON = DAY_SECONDS * TICK_RATE

/** A vehicle of `kind` at `x` metres, `hp` left, its front `front` damaged, its lamps switched on or not. */
const vehicle = (kind: number, x: number, { hp = 100, front = 0, lit = true } = {}): Components => ({
  kind: { kind }, place: { x: x * 100, y: 100, z: 0 }, orientation: { qx: 0, qy: 0, qz: 0 }, steer: { steer: 0, throttle: 0, spin: 0 }, health: { hp },
  ...(front ? { damage: { parts: [front, 0, 0, 0, 0, 0, 0, 0] } } : {}),
  ...(lit ? { flags: { flags: FLAG_LAMP, stance: 0 } } : {}),
})
const rider = (vehicle: number, seat: number): Components => ({ kind: { kind: 0 }, seated: { vehicle, seat, step: 0 } })

function harness() {
  const held: { x: number; radius: number; intensity: number; direction?: { x: number; y: number; z: number } }[] = []
  const fx = { smoke: new Particles(50), glow: new Particles(10, true), skids: new Skidmarks(10), groundAt: () => 0, surfaceAt: () => 0, camera: new Vec3(0, 5, 10) }
  const world: VehicleWorld = {
    root: new Group(),
    sounds: { begin() {}, end() {}, engine() {}, drop() {}, thud() {}, footstep() {} } as never,
    effects: {} as never,
    rounds: {} as never,
    debris: {} as never,
    lights: { hold: (x: number, _y: number, _z: number, light: { radius: number; intensity: number; direction?: { x: number; y: number; z: number } }) =>
      held.push({ x, ...light, direction: light.direction && { ...light.direction } }) } as never,
    fx: () => fx as never,
    castRay: () => null,
    driven: () => null,
    seated: () => null,
    listen() {},
    ownShot() {},
  }
  const store = new EntityStore()
  const machines = new Presenters<Frame>()
  const vehicles = machines.add(new Vehicles(world, machines.add(new Seating())))
  machines.add(new Headlights(world, vehicles))
  /** Draws a second at `tick`, so the lights finish fading; what is held in its last frame. */
  const settle = (tick: number) => {
    for (let i = 0; i < 60; i++) {
      held.length = 0
      machines.run(store.sampleAll(tick), { tick, dt: 1 / 60, now: 0, localId: null })
    }
    return held.slice()
  }
  /** One frame at `tick`; what it holds. */
  const draw = (tick: number) => {
    held.length = 0
    machines.run(store.sampleAll(tick), { tick, dt: 1 / 60, now: 0, localId: null })
    return held.slice()
  }
  return { store, settle, draw }
}

describe('headlights', () => {
  it('shine at night from every vehicle whose driver switched them on, and from nothing else', () => {
    const { store, settle } = harness()
    store.push(MIDNIGHT, new Map([
      [1, vehicle(KIND_JEEP, 0)], [2, vehicle(KIND_SPORTS, 20, { lit: false })], [3, vehicle(KIND_TANK, 60)], [4, vehicle(KIND_MORTAR, 80)],
      [11, rider(1, 0)], [13, rider(3, 0)],
    ]), true)
    const lights = settle(MIDNIGHT).sort((a, b) => a.x - b.x)
    expect(lights).toHaveLength(2)
    expect(lights[0]).toMatchObject({ intensity: 5, radius: 40 })
    expect(lights[0]!.x).toBeCloseTo(0, 0)
    // A tank has no lamps in its model: it shines from the front of its hull, along the nose.
    expect(lights[1]).toMatchObject({ intensity: 5, radius: 40 })
    expect(lights[1]!.x).toBeCloseTo(60, 0)
    expect(lights[1]!.direction!.z).toBeLessThan(-0.9)
  })

  it('are dim by day', () => {
    const { store, settle } = harness()
    store.push(NOON, new Map([[1, vehicle(KIND_JEEP, 0)], [11, rider(1, 0)]]), true)
    expect(settle(NOON)[0]!.intensity).toBeCloseTo(5 * 0.15)
  })

  it('go out when the lamps are smashed or the vehicle is wrecked', () => {
    const { store, settle } = harness()
    store.push(MIDNIGHT, new Map([
      [1, vehicle(KIND_JEEP, 0, { front: 200 })], [2, vehicle(KIND_JEEP, 20, { hp: 0 })], [11, rider(1, 0)], [12, rider(2, 0)],
    ]), true)
    expect(settle(MIDNIGHT)).toEqual([])
  })

  it('fade in over a quarter second when switched on', () => {
    const { store, draw } = harness()
    store.push(MIDNIGHT, new Map([[1, vehicle(KIND_JEEP, 0)], [11, rider(1, 0)]]), true)
    // One frame of a quarter second's fade: a fifteenth of full strength.
    expect(draw(MIDNIGHT)[0]!.intensity).toBeCloseTo(5 / 15)
    for (let i = 0; i < 14; i++) draw(MIDNIGHT)
    expect(draw(MIDNIGHT)[0]!.intensity).toBe(5)
  })

  it('shine a helicopter\'s searchlight twice as far, along its nose gun', () => {
    const { store, settle } = harness()
    // The gun's angles go over the wire in milliradians.
    const heli = (yaw: number, lit = true): Components => ({
      kind: { kind: KIND_HELI }, place: { x: 0, y: 3000, z: 0 }, orientation: { qx: 0, qy: 0, qz: 0 }, steer: { steer: 0, throttle: 0, spin: 0 }, health: { hp: 100 },
      turret: { yaw: yaw * 1000, pitch: -300, shots: 0, range: 0, heat: 0, flags: 0 },
      ...(lit ? { flags: { flags: FLAG_LAMP, stance: 0 } } : {}),
    })
    store.push(MIDNIGHT, new Map([[1, heli(0)], [2, { ...heli(0, false), place: { x: 5000, y: 3000, z: 0 } }], [11, rider(1, 0)]]), true)
    const ahead = settle(MIDNIGHT)
    expect(ahead).toHaveLength(1)
    expect(ahead[0]).toMatchObject({ radius: 80, intensity: 5 })
    // Along the nose (-z) and dipped with the gun.
    expect(ahead[0]!.direction!.z).toBeLessThan(-0.9)
    expect(ahead[0]!.direction!.y).toBeLessThan(-0.2)

    // The gunner swings the gun to the left: the beam turns with it.
    store.push(MIDNIGHT + 1, new Map([[1, heli(0.6)], [11, rider(1, 0)]]), true)
    const turned = settle(MIDNIGHT + 1)
    expect(turned[0]!.direction!.x).toBeCloseTo(-Math.sin(0.6) * Math.cos(0.3), 2)
  })

  it('shine an air ambulance\'s searchlight as far, fixed under its nose', () => {
    const { store, settle } = harness()
    store.push(MIDNIGHT, new Map([[1, vehicle(KIND_AIR_AMBULANCE, 0)], [11, rider(1, 0)]]), true)
    const lights = settle(MIDNIGHT)
    expect(lights).toHaveLength(1)
    expect(lights[0]).toMatchObject({ radius: 80, intensity: 5 })
    expect(lights[0]!.direction!.z).toBeLessThan(-0.9)
    expect(lights[0]!.direction!.y).toBeLessThan(-0.2)
  })
})
