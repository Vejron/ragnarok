import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { beforeAll, describe, expect, it } from 'vitest'
import { type Node, Vector3 } from '@ragnarok/engine'
import { KIND_AMV } from '@/net/codec'
import { kindView } from '@/kinds'
import { AmvModel } from '@/render/amv'
import { parseCarModel } from '@/render/car'
import { isLobGun } from '@/render/mortar'
import { rideView } from '@/seats'
import { AMV_MAX_STEER, AMV_AXLES_Z, AMV_COOLDOWN_TICKS, AMV_GUN_PIVOT, AMV_RELOAD_TICKS, AMV_RIDE_HEIGHT, AMV_SEATS, AMV_TRACK_HALF, AMV_TUBE_X, AMV_WHEEL_RADIUS } from '@/sim/consts'
import { AMV_LOB } from '@/sim/lob'
import { AMV_STEER_SHARES } from '@/sim/slip'
import { crewGun, seatHead } from '@/sim/vehicles'
import { triangleCount } from './helpers'

const IDENTITY = [0, 0, 0, 1] as const
const named = (root: Node, name: string) => root.getObjectByName(name)!
const gun = (yaw: number, pitch: number, flags = 0) => ({ yaw, pitch, shots: 0, range: 0, heat: 0, flags })
const round = (v: Vector3) => v.toArray().map((x) => Math.round(x * 1000) / 1000)

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/amv.glb'))
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseCarModel('amv', copy.buffer)
})

describe('the AMV', () => {
  it('is drawn as a mortar carrier, its tubes in its turret, and is laid with the mortar dial', () => {
    const amv = kindView(KIND_AMV).model(1)
    expect(amv).toBeInstanceOf(AmvModel)
    expect(named(amv.root, 'gun').parent).toBe(named(amv.root, 'turret'))
    expect(isLobGun(amv) && amv.lob).toBe(AMV_LOB)
    expect(triangleCount(amv.root)).toBeLessThan(2500)
    amv.dispose()
  })

  it('stands on eight wheels where the physics has them, and steers the front two axles, the second less', () => {
    const amv = new AmvModel(1)
    amv.update([0, 0, 0], IDENTITY, 1)
    amv.root.updateMatrixWorld(true)
    const axles: [string, number][] = [['F', AMV_AXLES_Z[0]!], ['F2', AMV_AXLES_Z[1]!], ['R2', AMV_AXLES_Z[2]!], ['R', AMV_AXLES_Z[3]!]]
    for (const [axle, z] of axles) {
      for (const [side, x] of [['L', -AMV_TRACK_HALF], ['R', AMV_TRACK_HALF]] as const) {
        const tag = axle[0] + side + axle.slice(1)
        const at = named(amv.root, `wheel_${tag}`).getWorldPosition(new Vector3())
        expect(at.x).toBeCloseTo(x, 3)
        expect(at.z).toBeCloseTo(z, 3)
        expect(at.y - AMV_WHEEL_RADIUS).toBeCloseTo(-AMV_RIDE_HEIGHT, 3)
        const turned = named(amv.root, `pivot_${tag}`).rotation.y
        expect(turned).toBeCloseTo(axle === 'F' ? -AMV_MAX_STEER : axle === 'F2' ? -AMV_MAX_STEER * AMV_STEER_SHARES[1]! : 0)
      }
    }
    amv.dispose()
  })

  it('points its tubes where the server aims them, and fires them in turn from either side of the pivot', () => {
    const amv = new AmvModel(1)
    amv.update([0, 0, 0], IDENTITY, 0, [0, 0, 0], gun(0, 0))
    expect(round(amv.barrel().origin)).toEqual([...AMV_GUN_PIVOT])
    expect(round(amv.barrel(-1).origin)).toEqual([AMV_GUN_PIVOT[0] - AMV_TUBE_X, AMV_GUN_PIVOT[1], AMV_GUN_PIVOT[2]])
    const [yaw, pitch] = [2.0, 1.1]
    amv.update([0, 0, 0], IDENTITY, 0, [0, 0, 0], gun(yaw, pitch))
    const { direction } = amv.barrel()
    expect(direction.x).toBeCloseTo(-Math.sin(yaw) * Math.cos(pitch))
    expect(direction.y).toBeCloseTo(Math.sin(pitch))
    expect(direction.z).toBeCloseTo(-Math.cos(yaw) * Math.cos(pitch))
    amv.dispose()
  })

  it('seals its crew in: a driver, a mortar operator with the range dial and the landing mark, and a rider', () => {
    const seats = kindView(KIND_AMV).ride.seats
    expect(seats).toHaveLength(AMV_SEATS)
    expect(seats.map((seat) => seat.role)).toEqual(['driver', 'mortar', 'passenger'])
    expect(seats[1]).toMatchObject({ rig: { name: 'mortar' }, gauges: ['range'], arc: 'mortar' })
    expect([0, 1, 2].map((seat) => seatHead(KIND_AMV, seat) ?? crewGun(KIND_AMV, seat))).toEqual([false, false, false])
  })

  it('turns its second axle as far as the server does: 2.45 m from the rear pair against the first axle at 3.95', () => {
    expect(AMV_STEER_SHARES).toHaveLength(2)
    expect(AMV_STEER_SHARES[0]).toBe(1)
    expect(AMV_STEER_SHARES[1]).toBeCloseTo(2.45 / 3.95, 5)
  })

  it('shows a quick reload between the tubes and the long one once both have fired', () => {
    expect(rideView(KIND_AMV)).toMatchObject({ reload: AMV_COOLDOWN_TICKS, refill: AMV_RELOAD_TICKS })
  })
})
