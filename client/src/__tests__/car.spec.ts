import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Mesh } from '@ragnarok/engine'
import * as THREE from 'three'
import { beforeAll, describe, expect, it } from 'vitest'
import { CarModel, parseCarModel } from '@/render/car'
import {
  JEEP_AXLE_Z, JEEP_RIDE_HEIGHT, JEEP_TRACK_HALF, JEEP_WHEEL_RADIUS, ROADSTER_AXLE_Z, ROADSTER_HALF_EXTENTS, ROADSTER_RIDE_HEIGHT, ROADSTER_SEATS, ROADSTER_TRACK_HALF,
  ROADSTER_WHEEL_RADIUS, SPORTS_AXLE_Z, SPORTS_RIDE_HEIGHT, SPORTS_SEATS, SPORTS_TRACK_HALF,
  SPORTS_WHEEL_RADIUS,
} from '@/sim/consts'
import { KIND_JEEP, KIND_ROADSTER, KIND_SPORTS, THRUST_BOOST, THRUST_LIFT } from '@/net/codec'
import { triangleCount } from './helpers'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]

async function load(kind: 'jeep' | 'sports' | 'roadster') {
  const bytes = readFileSync(resolve(process.cwd(), `public/models/${kind}.glb`))
  // A copy made here, so it is an ArrayBuffer of the realm the loader checks against.
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseCarModel(kind, copy.buffer)
}

beforeAll(async () => {
  await load('jeep')
  await load('sports')
  await load('roadster')
})

const triangles = (car: CarModel) => triangleCount(car.root)

describe.each([
  ['jeep', KIND_JEEP, JEEP_TRACK_HALF, JEEP_AXLE_Z, JEEP_WHEEL_RADIUS, JEEP_RIDE_HEIGHT, 4],
  ['sports car', KIND_SPORTS, SPORTS_TRACK_HALF, SPORTS_AXLE_Z, SPORTS_WHEEL_RADIUS, SPORTS_RIDE_HEIGHT, SPORTS_SEATS],
  ['Roadster', KIND_ROADSTER, ROADSTER_TRACK_HALF, ROADSTER_AXLE_Z, ROADSTER_WHEEL_RADIUS, ROADSTER_RIDE_HEIGHT, ROADSTER_SEATS],
] as const)('the %s model', (_name, kind, trackHalf, axleZ, radius, ride, seats) => {
  it('has its wheels where the physics has them, touching the ground at its ride height', () => {
    const car = new CarModel(kind, 1)
    car.update([0, 0, 0], IDENTITY, 0, [])
    car.root.updateMatrixWorld(true)
    for (const [tag, x, z] of [['FL', -trackHalf, -axleZ], ['FR', trackHalf, -axleZ], ['RL', -trackHalf, axleZ], ['RR', trackHalf, axleZ]] as const) {
      const wheel = car.root.getObjectByName(`wheel_${tag}`)!
      const at = wheel.getWorldPosition(new THREE.Vector3())
      expect(at.x).toBeCloseTo(x, 3)
      expect(at.z).toBeCloseTo(z, 3)
      // The wheel's lowest point is on the ground, `ride` below the chassis centre.
      expect(at.y - radius).toBeCloseTo(-ride, 3)
    }
    car.dispose()
  })

  it('shows one head per seat, and no more', () => {
    const car = new CarModel(kind, 1)
    const heads = car.root.children.filter((child) => child instanceof Mesh && child.userData.head)
    expect(heads).toHaveLength(seats)
    car.update([0, 0, 0], IDENTITY, 0, Array(seats).fill(7))
    expect(heads.every((head) => head.visible)).toBe(true)
    car.update([0, 0, 0], IDENTITY, 0, Array(seats).fill(0))
    expect(heads.some((head) => head.visible)).toBe(false)
    car.dispose()
  })

  it('steers the front wheels only and stays low-poly', () => {
    const car = new CarModel(kind, 1)
    car.update([0, 0, 0], IDENTITY, 1, [])
    expect(car.root.getObjectByName('pivot_FL')!.rotation.y).toBeLessThan(-0.3)
    expect(car.root.getObjectByName('pivot_RL')!.rotation.y).toBe(0)
    expect(triangles(car)).toBeLessThan(2500)
    car.dispose()
  })
})

describe('the two cars', () => {
  it('differ in clearance: the jeep sits much higher', () => {
    expect(JEEP_RIDE_HEIGHT).toBeGreaterThan(SPORTS_RIDE_HEIGHT * 2)
    expect(JEEP_WHEEL_RADIUS).toBeGreaterThan(SPORTS_WHEEL_RADIUS * 1.5)
  })
})

describe('the Roadster model', () => {
  it('has four rear nozzles blowing back and six lift nozzles blowing down, at the body', () => {
    const car = new CarModel(KIND_ROADSTER, 1)
    car.update([10, 2, -5], IDENTITY, 0, [])
    expect(car.jets(0)).toHaveLength(0)
    const rear = car.jets(THRUST_BOOST)
    const lift = car.jets(THRUST_LIFT)
    expect(rear).toHaveLength(4)
    expect(lift).toHaveLength(6)
    expect(car.jets(THRUST_BOOST | THRUST_LIFT)).toHaveLength(10)
    const [hx, hy, hz] = ROADSTER_HALF_EXTENTS
    for (const { at, direction } of rear) {
      // Out of the tail (+Z is behind), blowing backwards.
      expect(at.z - -5).toBeGreaterThan(hz)
      expect(direction.z).toBeCloseTo(1)
    }
    for (const { at, direction } of lift) {
      expect(Math.abs(at.x - 10)).toBeLessThan(hx)
      expect(at.y - 2).toBeLessThan(-hy * 0.5)
      expect(direction.y).toBeCloseTo(-1)
    }
    // Turned half round, the rear jets blow the other way.
    car.update([0, 0, 0], [0, 1, 0, 0], 0, [])
    expect(car.jets(THRUST_BOOST)[0]!.direction.z).toBeCloseTo(-1)
    car.dispose()
  })
})
