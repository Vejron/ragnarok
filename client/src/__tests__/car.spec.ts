import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Matrix4, Mesh, Vector3 } from '@ragnarok/engine'
import { kindView } from '@/kinds'
import { beforeAll, describe, expect, it } from 'vitest'
import { CarModel, parseCarModel } from '@/render/car'
import {
  BUS_AXLE_Z, BUS_RIDE_HEIGHT, BUS_SEATS, BUS_TRACK_HALF, BUS_WHEEL_RADIUS, EPA_AXLE_Z, EPA_RIDE_HEIGHT, EPA_SEATS, EPA_TRACK_HALF, EPA_WHEEL_RADIUS,
  JEEP_AXLE_Z, JEEP_RIDE_HEIGHT, JEEP_TRACK_HALF, JEEP_WHEEL_RADIUS, ROADSTER_AXLE_Z, ROADSTER_HALF_EXTENTS, ROADSTER_RIDE_HEIGHT, ROADSTER_SEATS, ROADSTER_TRACK_HALF,
  ROADSTER_WHEEL_RADIUS, SPORTS_AXLE_Z, SPORTS_RIDE_HEIGHT, SPORTS_SEATS, SPORTS_TRACK_HALF,
  SPORTS_WHEEL_RADIUS, TECHNICAL_AXLE_Z, TECHNICAL_RIDE_HEIGHT, TECHNICAL_SEATS, TECHNICAL_TRACK_HALF, TECHNICAL_WHEEL_RADIUS,
  SAAB_900_AXLE_Z, SAAB_900_RIDE_HEIGHT, SAAB_900_SEATS, SAAB_900_TRACK_HALF, SAAB_900_WHEEL_RADIUS, SAAB_9000_AXLE_Z, SAAB_9000_RIDE_HEIGHT, SAAB_9000_SEATS, SAAB_9000_TRACK_HALF, SAAB_9000_WHEEL_RADIUS, VOLVO_240_AXLE_Z, VOLVO_240_RIDE_HEIGHT, VOLVO_240_SEATS, VOLVO_240_TRACK_HALF, VOLVO_240_WHEEL_RADIUS, VOLVO_740_AXLE_Z, VOLVO_740_RIDE_HEIGHT, VOLVO_740_SEATS, VOLVO_740_TRACK_HALF, VOLVO_740_WHEEL_RADIUS, GTI_AXLE_Z, GTI_RIDE_HEIGHT, GTI_SEATS, GTI_TRACK_HALF, GTI_WHEEL_RADIUS, TRANSPORTER_AXLE_Z, TRANSPORTER_RIDE_HEIGHT, TRANSPORTER_SEATS, TRANSPORTER_TRACK_HALF, TRANSPORTER_WHEEL_RADIUS, KADETT_AXLE_Z, KADETT_RIDE_HEIGHT, KADETT_SEATS, KADETT_TRACK_HALF, KADETT_WHEEL_RADIUS, OMEGA_AXLE_Z, OMEGA_RIDE_HEIGHT, OMEGA_SEATS, OMEGA_TRACK_HALF, OMEGA_WHEEL_RADIUS, SIERRA_AXLE_Z, SIERRA_RIDE_HEIGHT, SIERRA_SEATS, SIERRA_TRACK_HALF, SIERRA_WHEEL_RADIUS, ESCORT_AXLE_Z, ESCORT_RIDE_HEIGHT, ESCORT_SEATS, ESCORT_TRACK_HALF, ESCORT_WHEEL_RADIUS,
  DOOR_SWING, POLICE_CAR_AXLE_Z, POLICE_CAR_RIDE_HEIGHT, POLICE_CAR_SEATS, POLICE_CAR_TRACK_HALF, POLICE_CAR_WHEEL_RADIUS, AMBULANCE_AXLE_Z, AMBULANCE_RIDE_HEIGHT, AMBULANCE_SEATS, AMBULANCE_TRACK_HALF, AMBULANCE_WHEEL_RADIUS,
} from '@/sim/consts'
import { KIND_BUS, KIND_EPA, KIND_JEEP, KIND_ROADSTER, KIND_SPORTS, KIND_TECHNICAL, KIND_SAAB_900, KIND_SAAB_9000, KIND_VOLVO_240, KIND_VOLVO_740, KIND_GTI, KIND_TRANSPORTER, KIND_KADETT, KIND_OMEGA, KIND_SIERRA, KIND_ESCORT, KIND_POLICE_CAR, KIND_AMBULANCE, THRUST_BOOST, THRUST_LIFT } from '@/net/codec'
import { triangleCount } from './helpers'
import { vehicleDef } from '@/sim/vehicles'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]

/** The period cars of the traffic (VEHICLES-XXIII.md) besides the jeep and the sports car. */
const PERIOD = ['saab_900', 'saab_9000', 'volvo_240', 'volvo_740', 'gti', 'transporter', 'kadett', 'omega', 'sierra', 'escort', 'police_car', 'ambulance'] as const

async function load(kind: 'jeep' | 'sports' | 'roadster' | 'technical' | 'bus' | 'epa' | (typeof PERIOD)[number]) {
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
  await load('technical')
  await load('bus')
  await load('epa')
  for (const kind of PERIOD) await load(kind)
})

const triangles = (car: CarModel) => triangleCount(car.root)

describe.each([
  ['jeep', KIND_JEEP, JEEP_TRACK_HALF, JEEP_AXLE_Z, JEEP_WHEEL_RADIUS, JEEP_RIDE_HEIGHT, 4],
  ['sports car', KIND_SPORTS, SPORTS_TRACK_HALF, SPORTS_AXLE_Z, SPORTS_WHEEL_RADIUS, SPORTS_RIDE_HEIGHT, SPORTS_SEATS],
  ['Roadster', KIND_ROADSTER, ROADSTER_TRACK_HALF, ROADSTER_AXLE_Z, ROADSTER_WHEEL_RADIUS, ROADSTER_RIDE_HEIGHT, ROADSTER_SEATS],
  ['technical', KIND_TECHNICAL, TECHNICAL_TRACK_HALF, TECHNICAL_AXLE_Z, TECHNICAL_WHEEL_RADIUS, TECHNICAL_RIDE_HEIGHT, TECHNICAL_SEATS],
  ['bus', KIND_BUS, BUS_TRACK_HALF, BUS_AXLE_Z, BUS_WHEEL_RADIUS, BUS_RIDE_HEIGHT, BUS_SEATS],
  ['EPA traktor', KIND_EPA, EPA_TRACK_HALF, EPA_AXLE_Z, EPA_WHEEL_RADIUS, EPA_RIDE_HEIGHT, EPA_SEATS],
  ['Saab 900 Turbo', KIND_SAAB_900, SAAB_900_TRACK_HALF, SAAB_900_AXLE_Z, SAAB_900_WHEEL_RADIUS, SAAB_900_RIDE_HEIGHT, SAAB_900_SEATS],
  ['Saab 9000', KIND_SAAB_9000, SAAB_9000_TRACK_HALF, SAAB_9000_AXLE_Z, SAAB_9000_WHEEL_RADIUS, SAAB_9000_RIDE_HEIGHT, SAAB_9000_SEATS],
  ['Volvo 240', KIND_VOLVO_240, VOLVO_240_TRACK_HALF, VOLVO_240_AXLE_Z, VOLVO_240_WHEEL_RADIUS, VOLVO_240_RIDE_HEIGHT, VOLVO_240_SEATS],
  ['Volvo 740', KIND_VOLVO_740, VOLVO_740_TRACK_HALF, VOLVO_740_AXLE_Z, VOLVO_740_WHEEL_RADIUS, VOLVO_740_RIDE_HEIGHT, VOLVO_740_SEATS],
  ['Golf GTI', KIND_GTI, GTI_TRACK_HALF, GTI_AXLE_Z, GTI_WHEEL_RADIUS, GTI_RIDE_HEIGHT, GTI_SEATS],
  ['VW Transporter', KIND_TRANSPORTER, TRANSPORTER_TRACK_HALF, TRANSPORTER_AXLE_Z, TRANSPORTER_WHEEL_RADIUS, TRANSPORTER_RIDE_HEIGHT, TRANSPORTER_SEATS],
  ['Opel Kadett', KIND_KADETT, KADETT_TRACK_HALF, KADETT_AXLE_Z, KADETT_WHEEL_RADIUS, KADETT_RIDE_HEIGHT, KADETT_SEATS],
  ['Opel Omega', KIND_OMEGA, OMEGA_TRACK_HALF, OMEGA_AXLE_Z, OMEGA_WHEEL_RADIUS, OMEGA_RIDE_HEIGHT, OMEGA_SEATS],
  ['Ford Sierra', KIND_SIERRA, SIERRA_TRACK_HALF, SIERRA_AXLE_Z, SIERRA_WHEEL_RADIUS, SIERRA_RIDE_HEIGHT, SIERRA_SEATS],
  ['Ford Escort', KIND_ESCORT, ESCORT_TRACK_HALF, ESCORT_AXLE_Z, ESCORT_WHEEL_RADIUS, ESCORT_RIDE_HEIGHT, ESCORT_SEATS],
  ['police car', KIND_POLICE_CAR, POLICE_CAR_TRACK_HALF, POLICE_CAR_AXLE_Z, POLICE_CAR_WHEEL_RADIUS, POLICE_CAR_RIDE_HEIGHT, POLICE_CAR_SEATS],
  ['ambulance', KIND_AMBULANCE, AMBULANCE_TRACK_HALF, AMBULANCE_AXLE_Z, AMBULANCE_WHEEL_RADIUS, AMBULANCE_RIDE_HEIGHT, AMBULANCE_SEATS],
] as const)('the %s model', (_name, kind, trackHalf, axleZ, radius, ride, seats) => {
  it('has its wheels where the physics has them, touching the ground at its ride height', () => {
    const car = (kindView(kind).model(1) as CarModel)
    car.update([0, 0, 0], IDENTITY, 0)
    car.root.updateMatrixWorld(true)
    for (const [tag, x, z] of [['FL', -trackHalf, -axleZ], ['FR', trackHalf, -axleZ], ['RL', -trackHalf, axleZ], ['RR', trackHalf, axleZ]] as const) {
      const wheel = car.root.getObjectByName(`wheel_${tag}`)!
      const at = wheel.getWorldPosition(new Vector3())
      expect(at.x).toBeCloseTo(x, 3)
      expect(at.z).toBeCloseTo(z, 3)
      // The wheel's lowest point is on the ground, `ride` below the chassis centre.
      expect(at.y - radius).toBeCloseTo(-ride, 3)
    }
    car.dispose()
  })

  it('hangs its front doors on the hinges the server has, swings them open outwards and throws one off', () => {
    const doors = vehicleDef(kind).doors
    const car = (kindView(kind).model(1) as CarModel)
    if (doors.length === 0) {
      expect(car.root.getObjectByName('door_FL')).toBeUndefined()
      car.dispose()
      return
    }
    expect(doors).toHaveLength(2)
    doors.forEach((door, n) => {
      const node = car.root.getObjectByName(n === 0 ? 'door_FL' : 'door_FR')!
      expect([node.position.x, node.position.y, node.position.z].map((v) => Math.round(v * 1000) / 1000)).toEqual([...door.hinge])
    })
    car.update([0, 0, 0], IDENTITY, 0)
    const rearEdge = () => {
      car.root.updateMatrixWorld(true)
      const node = car.root.getObjectByName('door_FL')!
      return new Vector3(0, 0, doors[0]!.length).applyMatrix4(new Matrix4().fromArray(node.matrixWorld))
    }
    const shut = rearEdge()
    // It eases towards how far open the server says, between snapshots, and gets there.
    car.swingDoors(1, 0, 0, 1 / 60, null, new Vector3())
    expect(car.doorsOpen[0]).toBeGreaterThan(0.1 * DOOR_SWING)
    expect(car.doorsOpen[0]).toBeLessThan(0.6 * DOOR_SWING)
    car.swingDoors(1, 0, 0, 1, null, new Vector3())
    expect(car.doorsOpen).toEqual([DOOR_SWING, 0])
    const open = rearEdge()
    expect(open.x).toBeLessThan(shut.x - 0.8 * doors[0]!.length * Math.sin(DOOR_SWING))
    expect(open.z).toBeLessThan(shut.z)
    // Off it comes, and is not drawn on the car.
    car.swingDoors(1, 0, 0b10, 1 / 60, null, new Vector3())
    expect(car.root.getObjectByName('door_FR')!.visible).toBe(false)
    expect(car.root.getObjectByName('door_FL')!.visible).toBe(true)
    car.dispose()
  })

  it('has a seat marker where the server puts each rider\'s head', () => {
    const car = (kindView(kind).model(1) as CarModel)
    const heads = vehicleDef(kind).seatHeads
    expect(car.seatMarkers).toHaveLength(seats)
    expect(heads).toHaveLength(seats)
    car.seatMarkers.forEach((marker, seat) => {
      expect([marker.x, marker.y, marker.z].map((v) => Math.round(v * 1000) / 1000)).toEqual([...heads[seat]!])
    })
    car.dispose()
  })

  it('shines its headlights from the front lamps, along the nose and dipped toward the road', () => {
    const car = (kindView(kind).model(1) as CarModel)
    // Turned a quarter to the left: the nose points along -x.
    car.update([10, 0, 0], [0, Math.SQRT1_2, 0, Math.SQRT1_2], 0)
    const beam = car.headlight()!
    expect(beam.at.x).toBeLessThan(10 - axleZ)
    expect(Math.abs(beam.at.z)).toBeLessThan(0.05)
    expect(beam.at.y).toBeGreaterThan(-ride)
    expect(beam.at.y).toBeLessThan(1.5)
    expect(beam.direction.x).toBeLessThan(-0.99)
    expect(beam.direction.y).toBeLessThan(0)
    car.dispose()
  })

  it('steers the front wheels only and stays low-poly', () => {
    const car = (kindView(kind).model(1) as CarModel)
    car.update([0, 0, 0], IDENTITY, 1)
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

describe('the EPA traktor model', () => {
  const glassOf = (car: CarModel) => {
    const panes: number[] = []
    car.root.traverse((node) => {
      if (node instanceof Mesh && node.material.name === 'glass') panes.push(node.material.opacity)
    })
    return panes
  }

  it('is seen through its glass, so its driver and passenger show in the cab, where the bus is not', () => {
    const epa = kindView(KIND_EPA).model(1) as CarModel
    const bus = kindView(KIND_BUS).model(1) as CarModel
    expect(glassOf(epa).length).toBeGreaterThan(0)
    expect(glassOf(epa).every((opacity) => opacity < 0.6)).toBe(true)
    expect(glassOf(bus).every((opacity) => opacity === 1)).toBe(true)
    epa.dispose()
    bus.dispose()
  })
})

describe('the Roadster model', () => {
  it('has four rear nozzles blowing back and six lift nozzles blowing down, at the body', () => {
    const car = (kindView(KIND_ROADSTER).model(1) as CarModel)
    car.update([10, 2, -5], IDENTITY, 0)
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
    car.update([0, 0, 0], [0, 1, 0, 0], 0)
    expect(car.jets(THRUST_BOOST)[0]!.direction.z).toBeCloseTo(-1)
    car.dispose()
  })
})
