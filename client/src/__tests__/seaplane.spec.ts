import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Vector3 } from '@ragnarok/engine'
import { beforeAll, describe, expect, it } from 'vitest'
import { boxOf, triangleCount } from './helpers'
import { cockpit } from '@/gauges'
import { kindView } from '@/kinds'
import { KIND_SEAPLANE } from '@/net/codec'
import { PlaneModel } from '@/render/plane'
import { parseSeaplaneModel, SeaplaneModel } from '@/render/seaplane'
import type { VehicleFx } from '@/render/tyres'
import {
  PLANE_BOMBS, PLANE_REAR_LENGTH, PLANE_STALL_SPEED, SEAPLANE_BOMB_PIVOT, SEAPLANE_FLOAT_HALF_TRACK, SEAPLANE_GUN_PIVOTS, SEAPLANE_HALF_EXTENTS, SEAPLANE_KEEL, SEAPLANE_REAR_PIVOT,
  SEAPLANE_RIDE_HEIGHT, SEAPLANE_SEATS, SEAPLANE_STALL_SPEED, SEAPLANE_WING_CENTRE, SEAPLANE_WING_HALF_EXTENTS,
} from '@/sim/consts'
import { vehicleDef } from '@/sim/vehicles'
import type { Arms } from '@/entities/units'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]
const ARMS: Arms = { shots: 0, heat: 0, rear_heat: 0, bombs: PLANE_BOMBS, flags: 0 }

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/seaplane.glb'))
  // A copy made here, so it is an ArrayBuffer of the realm the loader checks against.
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseSeaplaneModel(copy.buffer)
})

function named(plane: PlaneModel, name: string) {
  return plane.root.getObjectByName(name)!
}

describe('SeaplaneModel', () => {
  it('is drawn as a plane is, with its moving parts on the game’s pivots', () => {
    const plane = new SeaplaneModel(1)
    expect(plane).toBeInstanceOf(PlaneModel)
    for (const name of ['prop', 'elevator', 'rudder', 'aileron_L', 'aileron_R', ...Array.from({ length: PLANE_BOMBS }, (_, k) => `bomb_${k + 1}`)]) {
      expect(named(plane, name), name).toBeTruthy()
    }
    SEAPLANE_GUN_PIVOTS.forEach((pivot, side) => {
      const gun = named(plane, side === 0 ? 'gun_L' : 'gun_R')
      pivot.forEach((v, i) => expect(gun.getWorldPosition(new Vector3()).toArray()[i]).toBeCloseTo(v, 3))
    })
    const ring = named(plane, 'turret')
    SEAPLANE_REAR_PIVOT.forEach((v, i) => expect(ring.position.toArray()[i]).toBeCloseTo(v, 4))
    expect(named(plane, 'gun').parent).toBe(ring)
    expect(plane.bombPivot).toEqual(SEAPLANE_BOMB_PIVOT)
    plane.dispose()
  })

  it('stays low-poly', () => {
    const plane = new SeaplaneModel(1)
    expect(triangleCount(plane.root)).toBeLessThan(3000)
    plane.dispose()
  })

  it('stands on its wheels at the ride height, with its wings where the physics has them', () => {
    const plane = new SeaplaneModel(1)
    const all = boxOf(plane.root)
    // The tyres reach the road, a little below the pontoons' undersides.
    expect(all.min.y).toBeCloseTo(-SEAPLANE_RIDE_HEIGHT, 1)
    expect(all.min.y).toBeLessThan(-SEAPLANE_KEEL)
    const hull = boxOf(named(plane, 'hull'))
    // Wide enough to reach the pontoons, and no wider or taller than the wing box.
    expect(hull.max.x).toBeGreaterThan(SEAPLANE_FLOAT_HALF_TRACK + 0.2)
    expect(hull.max.x).toBeLessThanOrEqual(SEAPLANE_WING_HALF_EXTENTS[0] + 0.01)
    expect(hull.max.x).toBeGreaterThan(SEAPLANE_WING_HALF_EXTENTS[0] - 0.3)
    expect(hull.min.z).toBeGreaterThan(-SEAPLANE_HALF_EXTENTS[2] - 0.3)
    expect(hull.max.z).toBeLessThan(SEAPLANE_HALF_EXTENTS[2] + 0.6)
    expect(hull.max.y).toBeGreaterThan(SEAPLANE_WING_CENTRE[1] + SEAPLANE_WING_HALF_EXTENTS[1] - 0.2)
    plane.dispose()
  })

  it('fires the forward guns along the nose and swings the ring gun right round', () => {
    const plane = new SeaplaneModel(1)
    plane.update([10, 2, 5], IDENTITY, 0, [3, 4], { yaw: Math.PI, pitch: 0.5, shots: 0, range: 0, heat: 0, flags: 0 }, ARMS)
    const left = plane.forwardGun(0)
    expect(left.direction.z).toBeCloseTo(-1, 4)
    expect(left.origin.toArray()).toEqual([10 + SEAPLANE_GUN_PIVOTS[0][0], 2 + SEAPLANE_GUN_PIVOTS[0][1], 5 + SEAPLANE_GUN_PIVOTS[0][2]].map((v) => expect.closeTo(v, 3)))
    const { origin, direction } = plane.barrel()
    expect(direction.z).toBeGreaterThan(0.8)
    expect(direction.y).toBeCloseTo(Math.sin(0.5), 3)
    expect(plane.muzzle().distanceTo(origin)).toBeCloseTo(PLANE_REAR_LENGTH, 4)
    expect(plane.gunner).toBe(4)
    plane.dispose()
  })

  it('shows the bombs still on the rack', () => {
    const plane = new SeaplaneModel(1)
    plane.update([0, 0, 0], IDENTITY, 0, [0, 0], null, { ...ARMS, bombs: 1 })
    expect([1, 2, 3, 4].map((k) => named(plane, `bomb_${k}`).visible)).toEqual([true, false, false, false])
    plane.dispose()
  })
})

describe('the seaplane kind', () => {
  it('seats a pilot with the cockpit and the bomb arc, and a gunner on the ring', () => {
    const view = kindView(KIND_SEAPLANE)
    const def = vehicleDef(KIND_SEAPLANE)
    expect(def.name).toBe('Seaplane')
    expect(def.gun && def.arms).toBe(true)
    expect(view.ride.seats).toHaveLength(SEAPLANE_SEATS)
    expect(view.ride.seats.map((seat) => seat.role)).toEqual(['aviator', 'gunner'])
    expect(view.ride.seats[0]!.arc).toBe('bombs')
    expect(view.ride.seats[1]!.rig).toMatchObject({ name: 'gun' })
  })

  it('meets the water on its pontoons, and warns of its own stall speed', () => {
    const view = kindView(KIND_SEAPLANE)
    expect(view.fx.keel).toBe(SEAPLANE_KEEL)
    expect(view.fx.wheels?.contacts).toHaveLength(4)
    expect(view.ride.stall).toBe(SEAPLANE_STALL_SPEED)
    // Between the triplane's stall speed and its own, in the air: only the seaplane's pilot is warned.
    const speed = (PLANE_STALL_SPEED * 1.1 + SEAPLANE_STALL_SPEED * 1.1) / 2
    const motion = { grounded: false, forwardSpeed: speed } as VehicleFx
    expect(cockpit(ARMS, true, motion, view.ride.stall).stall).toBe(true)
    expect(cockpit(ARMS, true, motion).stall).toBe(false)
    expect(cockpit(ARMS, true, { grounded: true, forwardSpeed: 0 } as VehicleFx, view.ride.stall).stall).toBe(false)
  })
})
