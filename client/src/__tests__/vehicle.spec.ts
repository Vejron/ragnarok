import RAPIER from '@dimforge/rapier3d-compat'
import { describe, expect, it } from 'vitest'
import { CarSim, CAR_GRAVITY, conditionOf, controlsOf, VehicleDriver, type CarControls } from '@/sim/vehicle'
import { addTileCollider } from '@/sim/physics'
import { heightIn, type TileGround } from '@/sim/terrain'
import { f32 } from '@/sim/movement'
import { TICK_DT, vehicleMaxHp } from '@/sim/consts'
import { KIND_HELI, KIND_JEEP, KIND_SPORTS, KIND_TANK, type DriveState, type InputFrame, type VehicleState } from '@/net/codec'
import type { TileData, WorldData } from '@/world'

const COAST: CarControls = { throttle: 0, steer: 0, handbrake: false, lift: 0, powered: false }
const FORWARD: CarControls = { ...COAST, throttle: 1 }

/** Same as `scripted_controls` in vehicle.rs. */
const scripted = (tick: number): CarControls => {
  switch (Math.floor(tick / 60)) {
    case 0: return COAST
    case 1: case 2: return FORWARD
    case 3: case 4: return { ...COAST, throttle: 1, steer: 0.6 }
    case 5: return { ...COAST, throttle: 1, steer: -1 }
    case 6: return { ...COAST, throttle: -1 }
    default: return { ...COAST, throttle: 0.5, handbrake: true }
  }
}

/** `SCRIPTED_CHECKPOINTS` in vehicle.rs. */
const checkpoints: [number, number, [number, number, number]][] = [
  [KIND_JEEP, 90, [0, 0.8572, -0.9723]],
  [KIND_JEEP, 210, [2.5239, 0.8564, -22.4112]],
  [KIND_JEEP, 300, [30.9153, 0.856, -23.3722]],
  [KIND_JEEP, 480, [64.3589, 0.8568, -59.5452]],
  [KIND_SPORTS, 90, [-0.0019, 0.3975, -1.9154]],
  [KIND_SPORTS, 210, [4.3494, 0.3961, -44.2191]],
  [KIND_SPORTS, 300, [56.9594, 0.3953, -54.7882]],
  [KIND_SPORTS, 480, [125.6157, 0.3959, -128.6126]],
  [KIND_TANK, 90, [0, 0.94, -0.834]],
  [KIND_TANK, 210, [0.5739, 0.94, -17.5632]],
  [KIND_TANK, 300, [11.9063, 0.94, -29.5214]],
  [KIND_TANK, 480, [26.5903, 0.94, -40.3629]],
]

/** Same as `scripted_flight` in vehicle.rs. */
const PILOTED: CarControls = { ...COAST, powered: true }
const flight = (tick: number): CarControls => {
  switch (Math.floor(tick / 60)) {
    case 0: return PILOTED
    case 1: case 2: return { ...PILOTED, lift: 1 }
    case 3: case 4: return { ...PILOTED, throttle: 1 }
    case 5: case 6: return { ...PILOTED, throttle: 1, steer: 0.7 }
    default: return { ...PILOTED, throttle: -0.5, steer: -1, lift: -1 }
  }
}

/** `FLIGHT_CHECKPOINTS` in vehicle.rs: a helicopter started at [0, 3, 0]. */
const flightCheckpoints: [number, [number, number, number]][] = [
  [90, [0, 3.6099, 0]],
  [210, [0, 11.8079, -0.0842]],
  [330, [0.1181, 13.1328, -8.3432]],
  [480, [12.1913, 11.3589, -26.3039]],
]

describe('vehicle prediction', () => {
  it('WASM vehicles drive where the native server does', async () => {
    await RAPIER.init()
    for (const kind of [KIND_JEEP, KIND_SPORTS, KIND_TANK]) {
      const world = new RAPIER.World({ x: 0, y: -CAR_GRAVITY, z: 0 })
      world.timestep = TICK_DT
      world.createCollider(RAPIER.ColliderDesc.cuboid(200, 0.5, 200).setTranslation(0, -0.5, 0))
      const car = new CarSim(world, kind, [0, 1.5, 0], 0)
      for (let tick = 0; tick < 480; tick++) {
        car.drive(scripted(tick), TICK_DT)
        car.step(TICK_DT)
        const expected = checkpoints.find(([k, t]) => k === kind && t === tick + 1)
        if (!expected) continue
        car.state().position.forEach((value, axis) => {
          expect(Math.abs(value - expected[2][axis]!), `kind ${kind} tick ${tick + 1} axis ${axis}`).toBeLessThan(0.01)
        })
      }
      world.free()
    }
  })

  /** `DAMAGED_CHECKPOINTS` in vehicle.rs: a jeep with a bent front left wheel and no rear right one. */
  const damagedCheckpoints: [number, [number, number, number]][] = [
    [90, [0.1147, 0.7626, -0.3659]],
    [300, [9.175, 0.7638, -15.2648]],
    [480, [13.0374, 0.7641, -30.9333]],
  ]

  it('WASM drives a damaged jeep where the native server does', async () => {
    await RAPIER.init()
    const world = new RAPIER.World({ x: 0, y: -CAR_GRAVITY, z: 0 })
    world.timestep = TICK_DT
    world.createCollider(RAPIER.ColliderDesc.cuboid(200, 0.5, 200).setTranslation(0, -0.5, 0))
    const car = new CarSim(world, KIND_JEEP, [0, 1.5, 0], 0)
    car.condition = { power: 0.8, wheels: [180, 0, 0, 255] }
    for (let tick = 0; tick < 480; tick++) {
      car.drive(scripted(tick), TICK_DT)
      car.step(TICK_DT)
      const expected = damagedCheckpoints.find(([t]) => t === tick + 1)
      if (!expected) continue
      car.state().position.forEach((value, axis) => {
        expect(Math.abs(value - expected[1][axis]!), `tick ${tick + 1} axis ${axis}`).toBeLessThan(0.01)
      })
    }
    world.free()
  })

  it('reads a vehicle’s condition off the snapshot like the server', () => {
    expect(conditionOf(KIND_JEEP, 100, [9, 9, 9, 9, 1, 2, 3, 4])).toEqual({ power: 1, wheels: [1, 2, 3, 4] })
    expect(conditionOf(KIND_JEEP, 60, [0, 0, 0, 0, 0, 0, 0, 0]).power).toBe(1)
    expect(conditionOf(KIND_JEEP, 10, [0, 0, 0, 0, 0, 0, 0, 0]).power).toBeCloseTo(0.6)
    expect(conditionOf(KIND_TANK, 125, [0, 0, 0, 0, 0, 0, 0, 0]).power).toBe(1)
  })
})

describe('helicopter prediction', () => {
  it('WASM flies where the native server does', async () => {
    await RAPIER.init()
    const world = new RAPIER.World({ x: 0, y: -CAR_GRAVITY, z: 0 })
    world.timestep = TICK_DT
    world.createCollider(RAPIER.ColliderDesc.cuboid(200, 0.5, 200).setTranslation(0, -0.5, 0))
    const heli = new CarSim(world, KIND_HELI, [0, 3, 0], 0)
    for (let tick = 0; tick < 480; tick++) {
      heli.drive(flight(tick), TICK_DT)
      heli.step(TICK_DT)
      const expected = flightCheckpoints.find(([t]) => t === tick + 1)
      if (!expected) continue
      heli.state().position.forEach((value, axis) => {
        expect(Math.abs(value - expected[1][axis]!), `tick ${tick + 1} axis ${axis}`).toBeLessThan(0.01)
      })
    }
    world.free()
  })

  it('jump climbs and run sinks, as on the server', () => {
    const frame = (buttons: number): InputFrame => ({ seq: 1, strafe: 0, forward: 0, yaw: 0, pitch: 0, buttons, view: 0, slot: 0 })
    expect(controlsOf(frame(1))).toMatchObject({ lift: 1, powered: true })
    expect(controlsOf(frame(2))).toMatchObject({ lift: -1 })
    expect(controlsOf(frame(3))).toMatchObject({ lift: 0 })
  })
})

describe('vehicles on a slope', () => {
  /** `ramp` in vehicle.rs: 12°, rising towards -Z, 5 m cells over 200 m. */
  const SLOPE = f32(0.2125566)
  const side = 41
  const ramp = (): TileGround => {
    const heights = new Float32Array(side * side)
    for (let row = 0; row < side; row++) {
      const metres = Math.round(f32(f32(100 - (row * 5 - 100)) * SLOPE) * 100) / 100
      for (let column = 0; column < side; column++) heights[row * side + column] = metres
    }
    return { x0: -100, z0: -100, cell: 5, side, heights }
  }
  const tile: TileData = { buildings: [], roads: [], cover: [], trees: [], collision: { vertices: new Float32Array(), triangles: new Uint32Array() }, ground: new Float32Array(), water: new Float32Array(), streams: [], bridges: [] }

  /** `RAMP_CHECKPOINTS` in vehicle.rs. */
  const checkpoints: [number, [number, number, number]][] = [
    [60, [0, 13.8145, 39.1671]],
    [180, [0, 17.2683, 22.8746]],
    [360, [0, 29.7911, -36.0072]],
  ]

  it('WASM car climbs where the native server does', async () => {
    await RAPIER.init()
    const world = new RAPIER.World({ x: 0, y: -CAR_GRAVITY, z: 0 })
    world.timestep = TICK_DT
    world.createCollider(RAPIER.ColliderDesc.cuboid(100, 0.5, 100).setTranslation(0, -0.5, 0))
    addTileCollider(world, tile, ramp())
    world.step()
    const sim = new CarSim(world, KIND_JEEP, [0, heightIn(ramp(), 0, 40) + 1.5, 40], 0)
    for (let tick = 1; tick <= 360; tick++) {
      sim.drive(FORWARD, TICK_DT)
      sim.step(TICK_DT)
      const expected = checkpoints.find(([t]) => t === tick)?.[1]
      if (!expected) continue
      sim.state().position.forEach((value, axis) => expect(Math.abs(value - expected[axis]!), `tick ${tick} axis ${axis}`).toBeLessThan(0.02))
    }
    world.free()
  })
})

describe('driving a predicted vehicle', () => {
  const flat = (): WorldData => ({
    origin: [0, 0], halfExtent: 200, tileSize: 100, spawns: [], tiles: [], groundCell: 0,
    collision: { vertices: new Float32Array(), triangles: new Uint32Array() },
  })
  /** A driver's input that goes forward, swings left and right and brakes. */
  const input = (seq: number): InputFrame => ({
    seq, strafe: Math.round(Math.sin(seq / 25) * 100), forward: seq < 200 ? 127 : -127, yaw: 0, pitch: 0, view: 0, slot: 0, buttons: seq % 150 === 0 ? 1 : 0,
  })

  /** What the server would send after `ticks` of the inputs: quantized as on the wire. */
  async function serverState(kind: number, ticks: number): Promise<{ vehicle: VehicleState; drive: DriveState }> {
    await RAPIER.init()
    const world = new RAPIER.World({ x: 0, y: -CAR_GRAVITY, z: 0 })
    world.timestep = TICK_DT
    world.createCollider(RAPIER.ColliderDesc.cuboid(200, 0.5, 200).setTranslation(0, -0.5, 0))
    const sim = new CarSim(world, kind, [0, 1, 0], 0)
    // Settle first, the way a parked car has before anyone boards it.
    for (let tick = 0; tick < 120; tick++) {
      sim.drive({ ...COAST, handbrake: true }, TICK_DT)
      sim.step(TICK_DT)
    }
    const start = sim.state()
    for (let seq = 1; seq <= ticks; seq++) {
      sim.drive(controlsOf(input(seq)), TICK_DT)
      sim.step(TICK_DT)
    }
    const s = sim.state()
    const cm = (v: number) => Math.round(v * 100) / 100
    const unit = (v: number) => Math.round(v * 32767) / 32767
    const sign = s.rotation[3] < 0 ? -1 : 1
    const [qx, qy, qz] = [unit(s.rotation[0] * sign), unit(s.rotation[1] * sign), unit(s.rotation[2] * sign)]
    const milli = (v: number) => Math.round(v * 1000) / 1000
    void start
    world.free()
    return {
      vehicle: {
        id: 3, kind, x: cm(s.position[0]), y: cm(s.position[1]), z: cm(s.position[2]),
        qx, qy, qz, qw: Math.sqrt(1 - qx * qx - qy * qy - qz * qz), steer: 0, seats: [1, 0, 0, 0], turret: null, hp: vehicleMaxHp(kind), damage: [0, 0, 0, 0, 0, 0, 0, 0],
      },
      drive: {
        vx: cm(s.linvel[0]), vy: cm(s.linvel[1]), vz: cm(s.linvel[2]),
        wx: milli(s.angvel[0]), wy: milli(s.angvel[1]), wz: milli(s.angvel[2]),
        steer: milli(sim.steer), spin: 0,
      },
    }
  }

  it('replaying over the server state lands where an uninterrupted run does', async () => {
    for (const kind of [KIND_JEEP, KIND_TANK, KIND_HELI]) {
      // The client is 20 ticks ahead of the last state the server has told it about.
      const driver = await VehicleDriver.create(flat())
      const early = await serverState(kind, 0)
      driver.reconcile(0, early.vehicle, early.drive)
      for (let seq = 1; seq <= 300; seq++) driver.apply(input(seq))
      const predicted = driver.pose(1, 0)!
      const late = await serverState(kind, 280)
      driver.reconcile(280, late.vehicle, late.drive)
      const corrected = driver.pose(1, 0)!
      // Wire quantization and a replayed 20 ticks of hard driving: about 5 cm in practice.
      const distance = Math.hypot(...predicted.position.map((v, i) => v - corrected.position[i]!))
      expect(distance, `kind ${kind}`).toBeLessThan(0.1)
      expect(driver.lastCorrection).toBeLessThan(0.1)
      driver.dispose()
    }
  })

  it('stops predicting when the player leaves the seat', async () => {
    const driver = await VehicleDriver.create(flat())
    const state = await serverState(KIND_JEEP, 0)
    driver.reconcile(0, state.vehicle, state.drive)
    expect(driver.driving).toBe(3)
    driver.leave()
    expect(driver.driving).toBeNull()
    expect(driver.pose(0, 0)).toBeNull()
    driver.dispose()
  })
})
