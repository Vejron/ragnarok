import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Group, Vec3 } from '@ragnarok/engine'
import { beforeAll, describe, expect, it } from 'vitest'
import { Presenters, type Frame } from '@/entities/presenter'
import { Turrets } from '@/entities/presenters/guns'
import { Smoke } from '@/entities/presenters/smoke'
import { Seating } from '@/entities/presenters/seating'
import { Vehicles, type Driven, type VehicleWorld } from '@/entities/presenters/vehicles'
import { Wear } from '@/entities/presenters/wear'
import { EntityStore } from '@/entities/store'
import { DAMAGE_TURRET, KIND_TANK, TURRET_GONE } from '@/net/codec'
import type { Components } from '@/net/components.generated'
import { Particles } from '@/render/particles'
import { Skidmarks } from '@/render/skidmarks'
import { parseTankModel, TankModel } from '@/render/tank'

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/tank.glb'))
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseTankModel(copy.buffer)
})

/** A tank as the wire has it: its gun fired `shots` times, `hp` left. */
function tank(x: number, { shots = 0, hp = 250, damage }: { shots?: number; hp?: number; damage?: number[] } = {}): Components {
  return {
    kind: { kind: KIND_TANK }, place: { x: x * 100, y: 120, z: 0 }, orientation: { qx: 0, qy: 0, qz: 0 }, steer: { steer: 0, throttle: 0 },
    turret: { yaw: 0, pitch: 0, shots, range: 0, heat: 0, flags: 0 }, health: { hp },
    ...(damage ? { damage: { parts: damage } } : {}),
  }
}

/** Someone sitting in seat `seat` of vehicle `vehicle`. */
const rider = (vehicle: number, seat: number): Components => ({ kind: { kind: 0 }, seated: { vehicle, seat } })

/** Vehicles drawn into stand-ins that record what is heard, flashed and thrown. */
function harness(driven: Driven | null = null) {
  const log: string[] = []
  /** The heat of each barrel that smoked, frame by frame. */
  const smoking: number[] = []
  const fx = { smoke: new Particles(500), glow: new Particles(100, true), skids: new Skidmarks(100), groundAt: () => 0, surfaceAt: () => 0, camera: new Vec3(0, 5, 10) }
  const world: VehicleWorld = {
    root: new Group(),
    sounds: { begin() {}, end() {}, engine() {}, drop: (id: number) => log.push(`drop ${id}`), thud() {}, footstep() {}, cannon: () => log.push('cannon'), laser() {}, machineGun() {} } as never,
    effects: { gunfire: (_m: unknown, _d: unknown, _g: unknown, fired: number) => log.push(fired === 1 ? 'flash' : `flash of ${fired}`), barrelSmoke: (_m: unknown, _d: unknown, heat: number) => smoking.push(heat), muzzle() {} } as never,
    rounds: { fire() {} } as never,
    debris: { throw: () => log.push('turret thrown') } as never,
    lights: { flash: (_x: number, _y: number, _z: number, light: { color: number }) => log.push(light.color === 0xff7a24 ? 'fire' : 'muzzle light') } as never,
    fx: () => fx as never,
    castRay: () => null,
    driven: () => driven,
    seated: () => null,
    listen() {},
    ownShot: () => log.push('own shot'),
  }
  const store = new EntityStore()
  const machines = new Presenters<Frame>()
  const vehicles = machines.add(new Vehicles(world, machines.add(new Seating())))
  const wear = machines.add(new Wear(world, vehicles))
  machines.add(new Smoke(world, vehicles))
  machines.add(new Turrets(world, vehicles))
  const frame = (tick: number, localId: number | null = 7): Frame => ({ tick, dt: 1 / 60, now: tick * 33, localId })
  const draw = (tick: number) => machines.run(store.sampleAll(tick), frame(tick))
  return { log, smoking, store, vehicles, wear, draw, world, fx }
}

describe('the vehicle presenters', () => {
  it('draw a vehicle at its pose, and the one the local player drives where it is predicted', () => {
    const predicted: Driven = { id: 4, pose: { position: [9, 1, 9], rotation: [0, 0, 0, 1], steer: 0.5 } as never, jets: 0, throttle: 0 }
    const { store, vehicles, draw } = harness(predicted)
    store.push(10, new Map([[3, tank(1)], [4, tank(2)]]), true)
    draw(10)
    expect(vehicles.model(3)).toBeInstanceOf(TankModel)
    expect(vehicles.placed.get(3)).toMatchObject({ kind: KIND_TANK, x: 1, y: 1.2, z: 0, hp: 250, seats: [0, 0, 0] })
    expect(vehicles.placed.get(4)).toMatchObject({ x: 9, y: 1, z: 9, steer: 0.5 })
  })

  it('flash, bang and recoil when a gun’s counter moves, and not at first sight', () => {
    const { log, store, draw } = harness()
    store.push(10, new Map([[3, tank(1, { shots: 5 })], [7, rider(3, 1)]]), true)
    draw(10)
    expect(log).toEqual([])
    store.push(12, new Map([[3, tank(1, { shots: 6 })], [7, rider(3, 1)]]), true)
    draw(12)
    // The local player (7) mans the gun, so the HUD's reload starts.
    expect(log).toEqual(['flash', 'muzzle light', 'cannon', 'own shot'])
  })

  it('a gun that has fired smokes from its barrel for some seconds, less and less, and then stops', () => {
    const { smoking, store, draw } = harness()
    store.push(10, new Map([[3, tank(1, { shots: 5 })]]), true)
    draw(10)
    expect(smoking).toEqual([])
    store.push(12, new Map([[3, tank(1, { shots: 6 })]]), true)
    draw(12)
    expect(smoking).toEqual([1])
    // A tank's barrel smokes for five seconds: 300 frames of a sixtieth.
    for (let frame = 0; frame < 400; frame++) draw(12)
    expect(smoking.length).toBeGreaterThan(290)
    expect(smoking.length).toBeLessThan(310)
    expect(smoking[smoking.length - 1]).toBeLessThan(0.02)
    expect(smoking.every((heat, i) => i === 0 || heat < smoking[i - 1]!)).toBe(true)
  })

  it('a gun whose counter moved by several between two frames fired them all', () => {
    const { log, store, draw } = harness()
    store.push(10, new Map([[3, tank(1, { shots: 254 })]]), true)
    draw(10)
    // The counter is a byte, and wraps.
    store.push(12, new Map([[3, tank(1, { shots: 1 })]]), true)
    draw(12)
    expect(log).toEqual(['flash of 3', 'muzzle light', 'cannon'])
  })

  it('throw a shot-off turret clear once, and smoke and burn as the hull dies', () => {
    const { log, store, draw, wear } = harness()
    const gone = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    gone[DAMAGE_TURRET] = TURRET_GONE
    store.push(10, new Map([[3, tank(1, { hp: 0, damage: gone })]]), true)
    draw(10)
    draw(10.5)
    // The shot event saying so may come too; it is still thrown once.
    wear.shedTurret(3)
    expect(log.filter((line) => line === 'turret thrown')).toHaveLength(1)
    expect(log.filter((line) => line === 'fire')).toHaveLength(2)
  })

  it('let go of a vehicle and all it carried when it goes', () => {
    const { log, store, vehicles, draw } = harness()
    store.push(10, new Map([[3, tank(1)]]), true)
    draw(10)
    const model = vehicles.model(3)!
    store.push(12, new Map(), true)
    draw(12)
    expect(vehicles.model(3)).toBeUndefined()
    expect(model.root.parent).toBeNull()
    expect(log).toContain('drop 3')
  })
})
