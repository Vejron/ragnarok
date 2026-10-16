import { describe, expect, it } from 'vitest'
import { blendEntity, blendVehicle, Interpolator } from '@/net/interp'
import type { EntityState, VehicleState } from '@/net/codec'

const at = (x: number, yaw = 0): EntityState => ({ id: 7, kind: 0, x, y: 1, z: 0, yaw, vy: 0, grounded: true, dead: false, crouching: false, prone: false, knockX: 0, knockZ: 0, held: 0, crowing: false })

describe('Interpolator', () => {
  it('blends between the bracketing snapshots', () => {
    const interp = new Interpolator(blendEntity)
    interp.push(10, [at(0)])
    interp.push(12, [at(2)])
    expect(interp.sample(7, 11)!.x).toBeCloseTo(1)
    expect(interp.sample(7, 11)!.speed).toBeCloseTo(60)
  })

  it('reports the velocity between the two newest snapshots', () => {
    const interp = new Interpolator(blendEntity)
    expect(interp.latestVelocity(7)).toBeNull()
    interp.push(10, [at(0)])
    interp.push(12, [at(2)])
    expect(interp.latestVelocity(7)![0]).toBeCloseTo(60)
  })

  it('accepts out-of-order snapshots', () => {
    const interp = new Interpolator(blendEntity)
    interp.push(10, [at(0)])
    interp.push(14, [at(4)])
    interp.push(12, [at(1)])
    expect(interp.sample(7, 13)!.x).toBeCloseTo(2.5)
  })

  it('holds the newest state past the end', () => {
    const interp = new Interpolator(blendEntity)
    interp.push(10, [at(0)])
    interp.push(12, [at(2)])
    expect(interp.sample(7, 20)!.x).toBe(2)
  })

  it('takes the short way round for yaw', () => {
    const interp = new Interpolator(blendEntity)
    interp.push(10, [at(0, 3.0)])
    interp.push(12, [at(0, -3.0)])
    const yaw = interp.sample(7, 11)!.yaw
    expect(Math.abs(Math.abs(yaw) - Math.PI)).toBeLessThan(0.01)
  })

  it('drops entities that stop appearing', () => {
    const interp = new Interpolator(blendEntity)
    interp.push(10, [at(0)])
    interp.push(500, [])
    expect(interp.ids()).toEqual([])
  })

  it('blends vehicle orientation the short way and keeps the seats of the newer snapshot', () => {
    const car = (x: number, angle: number, seats: number[]): VehicleState => ({
      id: 9, kind: 1, x, y: 0.7, z: 0, qx: 0, qy: Math.sin(angle / 2), qz: 0, qw: Math.cos(angle / 2), steer: 0, seats, turret: null, hp: 100, damage: [0, 0, 0, 0, 0, 0, 0, 0], arms: null, thrust: null,
    })
    const interp = new Interpolator<VehicleState>(blendVehicle)
    // 0.2 rad and -0.2 rad about Y; the same rotation as q and -q must not spin the long way.
    const flipped = car(2, -0.2, [4, 0, 0, 0])
    interp.push(10, [car(0, 0.2, [0, 0, 0, 0])])
    interp.push(12, [{ ...flipped, qy: -flipped.qy, qw: -flipped.qw }])
    const mid = interp.sample(9, 11)!
    expect(mid.x).toBeCloseTo(1)
    expect(mid.seats).toEqual([4, 0, 0, 0])
    expect(Math.hypot(mid.qx, mid.qy, mid.qz, mid.qw)).toBeCloseTo(1)
    // Halfway between +0.2 and -0.2 rad is no rotation at all.
    expect(Math.abs(mid.qy)).toBeLessThan(0.01)
    expect(Math.abs(mid.qw)).toBeGreaterThan(0.99)
  })

  it('blends a turret by angle and takes the shot counter from the nearer snapshot', () => {
    const tank = (turret: VehicleState['turret']): VehicleState => ({
      id: 3, kind: 2, x: 0, y: 1, z: 0, qx: 0, qy: 0, qz: 0, qw: 1, steer: 0, seats: [1, 2], turret, hp: 250, damage: [0, 0, 0, 0, 0, 0, 0, 0], arms: null, thrust: null,
    })
    const interp = new Interpolator<VehicleState>(blendVehicle)
    // 3.0 to -3.0 rad is a 0.28 rad turn through ±π, not 6 rad the long way.
    interp.push(10, [tank({ yaw: 3.0, pitch: 0, shots: 4, range: 10, stowed: false })])
    interp.push(12, [tank({ yaw: -3.0, pitch: 0.4, shots: 5, range: 90, stowed: false })])
    const mid = interp.sample(3, 11)!.turret!
    expect(Math.abs(Math.abs(mid.yaw) - Math.PI)).toBeLessThan(0.01)
    expect(mid.pitch).toBeCloseTo(0.2)
    expect(interp.sample(3, 10.5)!.turret).toMatchObject({ shots: 4, range: 10 })
    expect(interp.sample(3, 11.5)!.turret).toMatchObject({ shots: 5, range: 90 })
  })
})
