import { describe, expect, it } from 'vitest'
import { blend, toGame, type Entity } from '../entities/units'
import { ARMS_CUT, FLAG_CROWING, FLAG_GROUNDED, KIND_PLANE, KIND_ROADSTER, KIND_ROOSTER, THRUST_BOOST, TURRET_STOWED } from '../net/codec'
import type { Components } from '../net/components.generated'
import fixture from './scene.fixture.json'

// `scene.fixture.json` is written by the Rust tests: a scene with every family and the edge cases of
// each, as the server replicates it.
const scene = new Map(fixture.scene as [number, Components][])
const game = (id: number) => toGame(scene.get(id)!)
const q = (n: number) => n / 32767

describe('replicated components in the game’s units', () => {
  it('scale each field by its unit: centimetres, milliradians, a share of full, tenths of a metre', () => {
    const person = toGame({ kind: { kind: 0 }, place: { x: 1234, y: -123456, z: 101 }, heading: { yaw: 1571 }, motion: { vy: -500, knock_x: 650, knock_z: -300 }, flags: { flags: 0, stance: 0 } })
    expect(person).toMatchObject({ place: { x: 12.34, y: -1234.56, z: 1.01 }, heading: { yaw: 1.571 }, motion: { vy: -5, knock_x: 6.5, knock_z: -3 } })
    const roadster = toGame({
      kind: { kind: KIND_ROADSTER }, place: { x: -250, y: 80, z: 4100 }, orientation: { qx: 1200, qy: -3000, qz: 15 }, steer: { steer: -64 },
      health: { hp: 100 }, damage: { parts: [0, 0, 40, 0, 0, 0, 255, 3] }, thrust: { air: 51, flags: THRUST_BOOST },
    })
    expect(roadster).toMatchObject({
      place: { x: -2.5, y: 0.8, z: 41 }, steer: { steer: -64 / 127 }, health: { hp: 100 }, damage: { parts: [0, 0, 40, 0, 0, 0, 255, 3] },
      thrust: { air: 0.2, flags: THRUST_BOOST },
    })
    expect(roadster.orientation).toEqual({ qx: q(1200), qy: q(-3000), qz: q(15), qw: expect.closeTo(Math.sqrt(1 - (1200 ** 2 + 3000 ** 2 + 15 ** 2) / 32767 ** 2), 9) })
    const tank = toGame({ kind: { kind: 2 }, place: { x: 0, y: 0, z: 0 }, orientation: { qx: 0, qy: 0, qz: 0 }, turret: { yaw: -700, pitch: 250, shots: 3, range: 1234, flags: TURRET_STOWED } })
    expect(tank.turret).toEqual({ yaw: -0.7, pitch: 0.25, shots: 3, range: 123.4, flags: TURRET_STOWED })
    expect(tank.orientation).toEqual({ qx: 0, qy: 0, qz: 0, qw: 1 })
    const plane = toGame({ kind: { kind: KIND_PLANE }, arms: { shots: 17, heat: 255, rear_heat: 51, bombs: 3, flags: ARMS_CUT } })
    expect(plane.arms).toEqual({ shots: 17, heat: 1, rear_heat: 0.2, bombs: 3, flags: ARMS_CUT })
    const gunner = toGame({ kind: { kind: 0 }, seated: { vehicle: 70000, seat: 1 }, heading: { yaw: 1571 }, look: { pitch: -100 } })
    expect(gunner).toEqual({ kind: { kind: 0 }, seated: { vehicle: 70000, seat: 1 }, heading: { yaw: 1.571 }, look: { pitch: -0.1 } })
  })

  it('keep steering within full lock', () => {
    expect(toGame({ steer: { steer: -128 } }).steer!.steer).toBe(-1)
  })

  it('carry every family of the fixture scene', () => {
    expect(game(1)).toMatchObject({ kind: { kind: 0 }, motion: { knock_x: 1.5 }, flags: { stance: 1 } })
    expect(game(2)).toMatchObject({ kind: { kind: KIND_ROOSTER }, flags: { flags: FLAG_GROUNDED | FLAG_CROWING } })
    expect(game(5).turret!.flags & TURRET_STOWED).toBe(TURRET_STOWED)
    expect(game(6)).toMatchObject({ arms: { bombs: 4 } })
    expect(game(12)).toMatchObject({ seated: { vehicle: 6, seat: 1 }, look: { pitch: -0.04 } })
    expect(game(13)).toMatchObject({ seated: { vehicle: 7, seat: 0 }, place: { x: 0.4 } })
    expect(game(8).health!.hp).toBe(0)
    expect(game(9)).toMatchObject({ projectile: { kind: 0 }, place: { x: 0.01, y: 20, z: 0.03 }, velocity: { vx: -40, vy: 0.5, vz: 0 } })
    expect(game(10)).toMatchObject({ pickup: { item: 7 }, place: { x: -0.05, y: 0.3, z: 0.05 } })
  })

  it('keep a component with no units as the wire has it', () => {
    const wire = scene.get(4)!
    expect(toGame(wire).health).toBe(wire.health)
    expect(toGame(wire).place).not.toBe(wire.place)
  })
})

/** A vehicle `angle` radians about Y, as the game holds it. */
const turned = (x: number, angle: number, extra: Partial<Entity> = {}): Entity => ({
  kind: { kind: 1 }, place: { x, y: 0.7, z: 0 }, orientation: { qx: 0, qy: Math.sin(angle / 2), qz: 0, qw: Math.cos(angle / 2) }, ...extra,
})

describe('blending two snapshots', () => {
  it('moves, and turns the short way round through ±π', () => {
    const a: Entity = { place: { x: 0, y: 1, z: 0 }, heading: { yaw: 3.0 } }
    const b: Entity = { place: { x: 2, y: 1, z: 0 }, heading: { yaw: -3.0 } }
    const mid = blend(a, b, 0.5)
    expect(mid.place!.x).toBeCloseTo(1)
    expect(Math.abs(Math.abs(mid.heading!.yaw) - Math.PI)).toBeLessThan(0.01)
  })

  it('turns a vehicle the short way when its rotation flips sign, and keeps it a unit rotation', () => {
    const b = turned(2, -0.2)
    const flipped = { ...b, orientation: { qx: -b.orientation!.qx, qy: -b.orientation!.qy, qz: -b.orientation!.qz, qw: -b.orientation!.qw } }
    const mid = blend(turned(0, 0.2), flipped, 0.5)
    const { qx, qy, qz, qw } = mid.orientation!
    expect(mid.place!.x).toBeCloseTo(1)
    expect(Math.hypot(qx, qy, qz, qw)).toBeCloseTo(1)
    // Halfway between +0.2 and -0.2 rad is no rotation at all.
    expect(Math.abs(qy)).toBeLessThan(0.01)
    expect(Math.abs(qw)).toBeGreaterThan(0.99)
  })

  it('turns a turret the short way, and takes its counters and the seats from the nearer snapshot', () => {
    const turret = (yaw: number, pitch: number, shots: number, range: number) => ({ yaw, pitch, shots, range, flags: 0 })
    const a = turned(0, 0, { turret: turret(3.0, 0, 4, 10) })
    const b = turned(0, 0, { turret: turret(-3.0, 0.4, 5, 90) })
    const mid = blend(a, b, 0.5).turret!
    expect(Math.abs(Math.abs(mid.yaw) - Math.PI)).toBeLessThan(0.01)
    expect(mid.pitch).toBeCloseTo(0.2)
    expect(blend(a, b, 0.4)).toMatchObject({ turret: { shots: 4, range: 10 } })
    expect(blend(a, b, 0.6)).toMatchObject({ turret: { shots: 5, range: 90 } })
  })

  it('turns a rider’s look, and takes their seat from the nearer snapshot', () => {
    const rider = (seat: number, yaw: number, pitch: number) => ({ seated: { vehicle: 4, seat }, heading: { yaw }, look: { pitch } })
    const a = rider(0, 0, 0)
    const b = rider(1, 1, 0.4)
    expect(blend(a, b, 0.25)).toMatchObject({ seated: { seat: 0 }, heading: { yaw: 0.25 } })
    expect(blend(a, b, 0.25).look!.pitch).toBeCloseTo(0.1)
    expect(blend(a, b, 0.75).seated).toEqual({ vehicle: 4, seat: 1 })
  })

  it('takes a component only one snapshot has from the nearer one', () => {
    const hurt = { ...game(4), damage: { parts: [0, 3, 0, 0, 0, 0, 0, 0] } }
    const whole = game(4)
    expect(blend(whole, hurt, 0.4).damage).toBeUndefined()
    expect(blend(whole, hurt, 0.6).damage?.parts).toEqual([0, 3, 0, 0, 0, 0, 0, 0])
  })

  it('keeps what did not move as it was, rather than making it again', () => {
    const a = game(4)
    const b = { ...a, place: { x: 9, y: 9, z: 9 } }
    const mid = blend(a, b, 0.5)
    expect(mid.orientation).toBe(b.orientation)
    expect(mid.turret).toBe(b.turret)
  })
})
