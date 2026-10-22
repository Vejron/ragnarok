import { describe, expect, it } from 'vitest'
import { blend, toGame, type Entity } from '../entities/units'
import { v1Scene } from '../net/adapter'
import { sceneStates, type EntityState, type RocketState, type VehicleState } from '../net/codec'
import type { Components } from '../net/components.generated'
import { blendEntity, blendRocket, blendVehicle } from '../net/interp'
import fixture from './adapter.fixture.json'

// The scene with every family and the edge cases of each (`adapter.fixture.json`, written by the Rust
// tests), converted by the units the schema gives each field, must say what v1's hand-written
// converters said of the same scene.
const scene = new Map(fixture.scene as [number, Components][])
const { wire, rockets, pickups } = v1Scene(scene)
const { entities, vehicles } = sceneStates(wire)
const game = (id: number) => toGame(scene.get(id)!)

/** What v1's stickman state says, read from an entity in the game's units. */
const asStickman = (e: Entity) => ({
  x: e.place!.x, y: e.place!.y, z: e.place!.z, yaw: e.heading!.yaw,
  vy: e.motion?.vy ?? 0, knockX: e.motion?.knock_x ?? 0, knockZ: e.motion?.knock_z ?? 0,
})
const fromStickman = (s: EntityState) => ({ x: s.x, y: s.y, z: s.z, yaw: s.yaw, vy: s.vy, knockX: s.knockX, knockZ: s.knockZ })

const asVehicle = (e: Entity) => ({
  x: e.place!.x, y: e.place!.y, z: e.place!.z, ...e.orientation!, steer: e.steer!.steer,
  turret: e.turret ? { yaw: e.turret.yaw, pitch: e.turret.pitch, shots: e.turret.shots, range: e.turret.range } : null,
  arms: e.arms ? { shots: e.arms.shots, heat: e.arms.heat, rearHeat: e.arms.rear_heat, bombs: e.arms.bombs, flags: e.arms.flags } : null,
  air: e.thrust?.air ?? null,
  crew: e.crew?.riders ?? [],
  hp: e.health!.hp,
})
const fromVehicle = (v: VehicleState) => ({
  x: v.x, y: v.y, z: v.z, qx: v.qx, qy: v.qy, qz: v.qz, qw: v.qw, steer: v.steer,
  turret: v.turret ? { yaw: v.turret.yaw, pitch: v.turret.pitch, shots: v.turret.shots, range: v.turret.range } : null,
  arms: v.arms, air: v.thrust?.air ?? null, crew: v.crew, hp: v.hp,
})

const asRocket = (e: Entity) => ({ x: e.place!.x, y: e.place!.y, z: e.place!.z, vx: e.velocity!.vx, vy: e.velocity!.vy, vz: e.velocity!.vz })
const fromRocket = ({ x, y, z, vx, vy, vz }: RocketState) => ({ x, y, z, vx, vy, vz })

/** The same entity a little later: everything that interpolates has moved, and a turn crosses ±π. */
function later(id: number): Components {
  const was = scene.get(id)!
  const moved: Components = { ...was, place: { x: was.place!.x + 37, y: was.place!.y - 12, z: was.place!.z + 250 } }
  if (was.heading) moved.heading = { yaw: 3100 }
  if (was.motion) moved.motion = { vy: 40, knock_x: -10, knock_z: 300 }
  if (was.orientation) moved.orientation = { qx: -was.orientation.qx, qy: 9000, qz: 400 }
  if (was.steer) moved.steer = { steer: -100 }
  if (was.turret) moved.turret = { ...was.turret, yaw: -3100, pitch: was.turret.pitch + 100, shots: was.turret.shots + 1 }
  if (was.crew) moved.crew = { riders: was.crew.riders.map((rider) => ({ ...rider, yaw: rider.yaw + 500 })) }
  if (was.velocity) moved.velocity = { vx: 100, vy: -900, vz: 20 }
  return moved
}

describe('replicated components in the game’s units', () => {
  it('convert as v1’s stickman states did', () => {
    for (const state of entities) expect(asStickman(game(state.id))).toEqual(fromStickman(state))
  })

  it('convert as v1’s vehicle states did', () => {
    for (const state of vehicles) expect(asVehicle(game(state.id))).toEqual(fromVehicle(state))
  })

  it('convert as v1’s rockets and items did', () => {
    for (const rocket of rockets) expect(asRocket(game(rocket.id))).toEqual(fromRocket(rocket))
    for (const pickup of pickups) expect({ item: game(pickup.id).pickup!.item, ...game(pickup.id).place }).toEqual({ item: pickup.item, x: pickup.x, y: pickup.y, z: pickup.z })
  })

  it('keep a component with no units as the wire has it', () => {
    const wire = scene.get(4)!
    expect(toGame(wire).seats).toBe(wire.seats)
    expect(toGame(wire).place).not.toBe(wire.place)
  })

  for (const t of [0, 0.3, 0.7, 1]) {
    it(`blend as v1 did, ${t} of the way`, () => {
      for (const state of entities) {
        const [a, b] = [game(state.id), toGame(later(state.id))]
        const old = blendEntity(state, sceneStates(v1Scene(new Map([[state.id, later(state.id)]])).wire).entities[0]!, t)
        expect(asStickman(blend(a, b, t))).toEqual(fromStickman(old))
      }
      for (const state of vehicles) {
        const [a, b] = [game(state.id), toGame(later(state.id))]
        const old = blendVehicle(state, sceneStates(v1Scene(new Map([[state.id, later(state.id)]])).wire).vehicles[0]!, t)
        const ours = asVehicle(blend(a, b, t))
        const theirs = fromVehicle(old)
        for (const key of ['x', 'y', 'z', 'qx', 'qy', 'qz', 'qw', 'steer'] as const) expect(ours[key]).toBeCloseTo(theirs[key], 12)
        expect(ours.turret).toEqual(theirs.turret)
        expect(ours.crew).toEqual(theirs.crew)
        expect(ours.arms).toEqual(theirs.arms)
      }
      for (const rocket of rockets) {
        const [a, b] = [game(rocket.id), toGame(later(rocket.id))]
        const old = blendRocket(rocket, v1Scene(new Map([[rocket.id, later(rocket.id)]])).rockets[0]!, t)
        expect(asRocket(blend(a, b, t))).toEqual(fromRocket(old))
      }
    })
  }

  it('take a component only one snapshot has from the nearer one', () => {
    const hurt = toGame({ ...scene.get(4)!, damage: { parts: [0, 3, 0, 0, 0, 0, 0, 0] } })
    const whole = game(4)
    expect(blend(whole, hurt, 0.4).damage).toBeUndefined()
    expect(blend(whole, hurt, 0.6).damage?.parts).toEqual([0, 3, 0, 0, 0, 0, 0, 0])
  })
})
