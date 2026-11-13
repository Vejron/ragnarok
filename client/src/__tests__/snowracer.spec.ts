import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { kindView } from '@/kinds'
import { beforeAll, describe, expect, it } from 'vitest'
import { CarModel, parseCarModel } from '@/render/car'
import { KIND_SNOWRACER } from '@/net/codec'
import { SNOWRACER_AXLE_Z, SNOWRACER_RIDE_HEIGHT, SNOWRACER_SEATS } from '@/sim/consts'
import { vehicleDef } from '@/sim/vehicles'
import { triangleCount } from './helpers'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/snowracer.glb'))
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseCarModel('snowracer', copy.buffer)
})

describe('the snowracer model', () => {
  it('has a seat marker where the server puts each rider\'s head, the passenger behind the rider', () => {
    const car = kindView(KIND_SNOWRACER).model(1) as CarModel
    const heads = vehicleDef(KIND_SNOWRACER).seatHeads
    expect(car.seatMarkers).toHaveLength(SNOWRACER_SEATS)
    car.seatMarkers.forEach((marker, seat) => {
      expect([marker.x, marker.y, marker.z].map((v) => Math.round(v * 1000) / 1000)).toEqual([...heads[seat]!])
    })
    expect(car.seatMarkers[1]!.z).toBeGreaterThan(car.seatMarkers[0]!.z)
    car.dispose()
  })

  it('steers its one ski under the nose, has nothing that rolls, and stays low-poly', () => {
    const car = kindView(KIND_SNOWRACER).model(1) as CarModel
    car.update([0, 0, 0], IDENTITY, 1)
    const ski = car.root.getObjectByName('pivot_FL')!
    expect(ski.rotation.y).toBeLessThan(-0.3)
    expect(ski.position.x).toBeCloseTo(0)
    expect(ski.position.z).toBeCloseTo(-SNOWRACER_AXLE_Z)
    expect(car.root.getObjectByName('pivot_FR')).toBeFalsy()
    expect(car.root.getObjectByName('wheel_RL')).toBeFalsy()
    expect(triangleCount(car.root)).toBeLessThan(2500)
    car.dispose()
  })

  it('lights the way ahead from its nose', () => {
    const car = kindView(KIND_SNOWRACER).model(1) as CarModel
    car.update([0, 0, 0], IDENTITY, 0)
    const beam = car.headlight()!
    expect(beam.at.z).toBeLessThan(-SNOWRACER_AXLE_Z)
    expect(beam.at.y).toBeGreaterThan(-SNOWRACER_RIDE_HEIGHT)
    expect(beam.direction.z).toBeLessThan(-0.9)
    car.dispose()
  })
})
