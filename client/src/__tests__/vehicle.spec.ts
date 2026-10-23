import { describe, expect, it } from 'vitest'
import { VehicleDriver } from '@/sim/driver'
import { controlsOf, SimMotor, type CarControls, type VehiclePose } from '@/sim/motor'
import { heightIn, type TileGround } from '@/sim/terrain'
import { f32 } from '@/sim/round'
import { BUTTON_CROUCH, BUTTON_JUMP, BUTTON_RUN, LOITER_SPEED, TICK_DT, vehicleMaxHp } from '@/sim/consts'
import { KIND_DRONE, KIND_HELI, KIND_HEN, KIND_JEEP, KIND_LOITER, KIND_PLANE, KIND_ROADSTER, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_WALKER, type DriveState, type InputFrame } from '@/net/codec'
import { manifestBytes, tileBytes } from './worlds'

// Every vehicle here is driven by the server's own physics (`crates/sim`, built to WASM), and the
// checkpoints are where the native build put it (`crates/sim/src/vehicle/tests.rs`): what is checked
// is that the WASM build of the same code agrees.

/** A flat field `half` metres from the middle to each side, as the native tests' worlds are. */
const field = (half = 400) => SimMotor.create(manifestBytes(half, 2 * half))

/** The vehicle being run. */
const CAR = 1

/** Runs a fresh vehicle of `kind` from `start` under `controls`, checking it against `checkpoints` (`[tick, position]`) within `tolerance`. */
async function run(
  kind: number,
  start: [number, number, number],
  ticks: number,
  controls: (tick: number) => CarControls,
  checkpoints: readonly (readonly [number, readonly number[], number?])[],
  tolerance = 0.01,
  setUp: (motor: SimMotor) => void | Promise<void> = () => {},
  half = 400,
) {
  const motor = await field(half)
  motor.startCar(CAR, kind, start)
  await setUp(motor)
  for (let tick = 0; tick < ticks; tick++) {
    motor.driveCar(CAR, controls(tick), TICK_DT)
    const expected = checkpoints.find(([t]) => t === tick + 1)
    if (!expected) continue
    const state = motor.carState(CAR)!
    state.position.forEach((value, axis) => {
      expect(Math.abs(value - expected[1][axis]!), `kind ${kind} tick ${tick + 1} axis ${axis}`).toBeLessThan(tolerance)
    })
    if (expected[2] !== undefined) expect(state.tank!).toBeCloseTo(expected[2], 3)
  }
  motor.dispose()
}

const frame = (buttons: number, rest: Partial<InputFrame> = {}): InputFrame => ({ seq: 1, strafe: 0, forward: 0, yaw: 0, buttons, pitch: 0, view: 0, slot: 0, ...rest })

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
  [KIND_STRV, 90, [0, 1.0798, -1.1357]],
  [KIND_STRV, 210, [0.8855, 1.08, -23.6921]],
  [KIND_STRV, 300, [18.7068, 1.08, -35.1709]],
  [KIND_STRV, 480, [47.1117, 1.0799, -41.0568]],
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

/** Same as `scripted_drone` in vehicle.rs. */
const looking = (yaw: number, pitch: number): CarControls => ({ ...PILOTED, aim: [yaw, pitch] })
const droneFlight = (tick: number): CarControls => {
  switch (Math.floor(tick / 60)) {
    case 0: return looking(0, 0)
    case 1: case 2: return { ...looking(0, 0), lift: 1 }
    case 3: case 4: return { ...looking(0, -0.2), throttle: 1 }
    case 5: case 6: return { ...looking(1, 0), throttle: 1, steer: 0.5 }
    default: return { ...looking(1, 0), throttle: -0.5, lift: -1 }
  }
}

/** `DRONE_CHECKPOINTS` in vehicle.rs: a drone started at [0, 3, 0]. */
const droneCheckpoints: [number, [number, number, number]][] = [
  [90, [0, 4.8267, 0]],
  [210, [0, 21.206, -1.9828]],
  [330, [-0.9514, 18.6062, -42.9446]],
  [480, [-38.1789, 15.5587, -89.1635]],
]

/** Same as `scripted_loiter` in vehicle.rs. */
const loiterFlight = (tick: number): CarControls => {
  switch (Math.floor(tick / 60)) {
    case 0: return looking(0, 0)
    case 1: case 2: return looking(1, 0)
    case 3: case 4: return looking(1, -0.4)
    case 5: return { ...looking(1, 0), steer: 1 }
    default: return { ...looking(1, 0), throttle: -1 }
  }
}

/** `LOITER_CHECKPOINTS` in vehicle.rs: a loitering drone started at [0, 60, 100], flying towards -Z. */
const loiterCheckpoints: [number, [number, number, number]][] = [
  [90, [-1.9952, 60.7377, 48.0302]],
  [210, [-55.8591, 63.7674, 5.9786]],
  [330, [-109.7632, 38.6259, -29.9292]],
  [480, [-157.6974, 55.486, -57.6976]],
]

/** Same as `scripted_walk` in vehicle.rs. */
const WALKING: CarControls = { ...COAST, powered: true }
const walked = (tick: number): CarControls => {
  switch (Math.floor(tick / 60)) {
    case 0: return WALKING
    case 1: case 2: case 3: return { ...WALKING, throttle: 1 }
    case 4: case 5: return { ...WALKING, throttle: 1, steer: 0.7 }
    default: return { ...WALKING, throttle: -1 }
  }
}

/** `WALK_CHECKPOINTS` in vehicle.rs: a walker started at [0, 4.9, 0]. */
const walkCheckpoints: [number, [number, number, number]][] = [
  [90, [0, 5.2644, -0.5608]],
  [210, [0, 5.2194, -14.715]],
  [330, [3.3595, 5.2467, -32.3642]],
  [480, [9.8452, 5.1868, -36.6408]],
]

describe('vehicle prediction', () => {
  it('a WASM walker walks where the native server does', async () => {
    await run(KIND_WALKER, [0, 4.9, 0], 480, walked, walkCheckpoints, 0.05)
  })

  it('WASM vehicles drive where the native server does', async () => {
    for (const kind of [KIND_JEEP, KIND_SPORTS, KIND_TANK, KIND_STRV]) {
      await run(kind, [0, 1.5, 0], 480, scripted, checkpoints.filter(([k]) => k === kind).map(([, tick, position]) => [tick, position] as const))
    }
  })

  /** `DAMAGED_CHECKPOINTS` in vehicle.rs: a jeep with a bent front left wheel and no rear right one. */
  const damagedCheckpoints: [number, [number, number, number]][] = [
    [90, [0.1147, 0.7626, -0.3659]],
    [300, [9.175, 0.7638, -15.2648]],
    [480, [13.0374, 0.7641, -30.9333]],
  ]

  it('WASM drives a damaged jeep where the native server does', async () => {
    // 30 of 100 health is 0.8 of the engine's power; a bent front left wheel and no rear right one.
    await run(KIND_JEEP, [0, 1.5, 0], 480, scripted, damagedCheckpoints, 0.01, (motor) => motor.placeCar(CAR, { position: [0, 1.5, 0], hp: 30, damage: [0, 0, 0, 0, 180, 0, 0, 255] }))
  })
})

/** Same as `scripted_thrust` in vehicle.rs. */
const thrusted = (tick: number): CarControls => {
  switch (Math.floor(tick / 60)) {
    case 0: return COAST
    case 1: return FORWARD
    case 2: return { ...FORWARD, boost: true }
    case 3: return { ...FORWARD, hop: true, steer: 0.5 }
    default: return { ...COAST, handbrake: true }
  }
}

/** `THRUST_CHECKPOINTS` in vehicle.rs: `[tick, position, tank]`. */
const thrustCheckpoints: [number, [number, number, number], number][] = [
  [120, [-0.0019, 0.3968, -8.9728], 1.0],
  [180, [-0.0019, 0.3992, -46.4509], 0.67],
  [240, [0.8576, 4.5892, -100.0672], 0.2742],
  [420, [13.2106, 0.3972, -179.1133], 0.6742],
]

describe('roadster prediction', () => {
  it('WASM boosts, hops and hovers where the native server does', async () => {
    await run(KIND_ROADSTER, [0, 1.5, 0], 420, thrusted, thrustCheckpoints)
  })

  it('maps run, jump and crouch to boost, hop and handbrake for a Roadster only', () => {
    expect(controlsOf(frame(BUTTON_RUN), KIND_ROADSTER)).toMatchObject({ boost: true, hop: false, handbrake: false, lift: 0 })
    expect(controlsOf(frame(BUTTON_JUMP), KIND_ROADSTER)).toMatchObject({ boost: false, hop: true, handbrake: false, lift: 0 })
    expect(controlsOf(frame(BUTTON_CROUCH), KIND_ROADSTER)).toMatchObject({ handbrake: true })
    expect(controlsOf(frame(BUTTON_JUMP), KIND_SPORTS)).toMatchObject({ boost: false, hop: false, handbrake: true })
  })
})

describe('helicopter prediction', () => {
  it('WASM flies where the native server does', async () => {
    await run(KIND_HELI, [0, 3, 0], 480, flight, flightCheckpoints)
  })

  it('jump climbs and run sinks, as on the server', () => {
    expect(controlsOf(frame(BUTTON_JUMP), KIND_HELI)).toMatchObject({ lift: 1, powered: true })
    expect(controlsOf(frame(BUTTON_RUN), KIND_HELI)).toMatchObject({ lift: -1 })
    expect(controlsOf(frame(BUTTON_JUMP | BUTTON_RUN), KIND_HELI)).toMatchObject({ lift: 0 })
  })
})

describe('drone prediction', () => {
  it('WASM flies a drone where the native server does', async () => {
    await run(KIND_DRONE, [0, 3, 0], 480, droneFlight, droneCheckpoints)
  })

  it('flies towards where its pilot looks', () => {
    const looks = frame(0, { forward: 127, yaw: 500, pitch: -300 })
    // The aim comes back from the server's code as f32.
    const aimed = (aim: [number, number] | null | undefined) => aim!.forEach((value, i) => expect(value).toBeCloseTo([0.5, -0.3][i]!, 6))
    for (const kind of [KIND_DRONE, KIND_LOITER]) {
      const controls = controlsOf(looks, kind)
      expect(controls).toMatchObject({ throttle: 1, powered: true })
      aimed(controls.aim)
    }
    // Nothing that drives on the ground reads the look.
    expect(controlsOf(looks, KIND_JEEP).aim).toBeNull()
  })

  it('WASM flies a loitering drone where the native server does', async () => {
    await run(KIND_LOITER, [0, 60, 100], 480, loiterFlight, loiterCheckpoints, 0.01, (motor) => motor.placeCar(CAR, { position: [0, 60, 100], linvel: [0, 0, -LOITER_SPEED] }))
  })
})

/** `pilot` in vehicle.rs: the throttle at `lift` (Space +1, Shift -1), the stick pulled `pull` (S +1) and rolled `roll` (D +1). */
const pilot = (lift: number, pull: number, roll: number): CarControls => ({ ...COAST, lift, throttle: -pull, steer: roll, powered: true })

/** `scripted_plane` in vehicle.rs. */
const planeFlight = (tick: number): CarControls => {
  const second = Math.floor(tick / 60)
  if (second === 0) return pilot(0, 0, 0)
  if (second < 7) return pilot(1, 0.3, 0)
  if (second < 9) return pilot(0, 0, 0.8)
  if (second < 11) return pilot(-1, 0.5, 0.8)
  return pilot(1, 0.1, -0.5)
}

/** `PLANE_CHECKPOINTS` in vehicle.rs: a plane started at [0, 1.5, 600] on a field 3 km from the middle to each edge. */
const planeCheckpoints: [number, [number, number, number]][] = [
  [120, [0.0000, 1.0504, 595.1553]],
  [420, [0.0000, 12.9997, 500.8397]],
  [660, [17.7795, 11.9774, 408.6677]],
  [900, [78.6062, 2.9332, 305.7454]],
]

describe('plane prediction', () => {
  it('WASM flies a plane where the native server does', async () => {
    // A field 3 km from the middle to each edge, as the native test's.
    await run(KIND_PLANE, [0, 1.5, 600], 900, planeFlight, planeCheckpoints, 0.01, () => {}, 3000)
  })

  it('reads the stick and throttle off the keys and the aim off where the mouse looks', () => {
    // W is forward on the stick (nose down), Space the throttle up, Shift the throttle down.
    const keys = (forward: number, strafe: number, buttons: number) => frame(buttons, { forward, strafe, yaw: 1200, pitch: -300 })
    expect(controlsOf(keys(127, 0, BUTTON_JUMP), KIND_PLANE)).toMatchObject({ throttle: 1, lift: 1, powered: true })
    expect(controlsOf(keys(-127, 127, BUTTON_RUN), KIND_PLANE)).toMatchObject({ throttle: -1, steer: 1, lift: -1 })
    controlsOf(keys(0, 0, 0), KIND_PLANE).aim!.forEach((value, i) => expect(value).toBeCloseTo([1.2, -0.3][i]!, 6))
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
  /** `RAMP_CHECKPOINTS` in vehicle.rs. */
  const checkpoints: [number, [number, number, number]][] = [
    [60, [0, 13.8145, 39.1671]],
    [180, [0, 17.2683, 22.8746]],
    [360, [0, 29.7911, -36.0072]],
  ]

  it('WASM car climbs where the native server does', async () => {
    // One 200 m tile with 5 m ground cells: the ramp is its ground.
    const motor = await SimMotor.create(manifestBytes(100, 200, 5))
    motor.addTile({ col: 0, row: 0 }, tileBytes({ ground: ramp().heights }))
    motor.startCar(CAR, KIND_JEEP, [0, heightIn(ramp(), 0, 40) + 1.5, 40])
    for (let tick = 1; tick <= 360; tick++) {
      motor.driveCar(CAR, FORWARD, TICK_DT)
      const expected = checkpoints.find(([t]) => t === tick)?.[1]
      if (!expected) continue
      motor.carState(CAR)!.position.forEach((value, axis) => expect(Math.abs(value - expected[axis]!), `tick ${tick} axis ${axis}`).toBeLessThan(0.02))
    }
    motor.dispose()
  })
})

describe('driving a predicted vehicle', () => {
  /** A driver's input that goes forward, swings left and right and brakes. */
  const input = (seq: number): InputFrame => ({
    seq, strafe: Math.round(Math.sin(seq / 25) * 100), forward: seq < 200 ? 127 : -127, yaw: 0, pitch: 0, view: 0, slot: 0, buttons: seq % 150 === 0 ? 1 : 0,
  })

  /** What the server would send after `ticks` of the inputs: quantized as on the wire. */
  async function serverState(kind: number, ticks: number): Promise<{ vehicle: VehiclePose; drive: DriveState }> {
    const server = await field(200)
    server.startCar(3, kind, [0, 1, 0])
    // Settle first, the way a parked car has before anyone boards it.
    for (let tick = 0; tick < 120; tick++) server.driveCar(3, { ...COAST, handbrake: true }, TICK_DT)
    for (let seq = 1; seq <= ticks; seq++) server.driveCar(3, controlsOf(input(seq), kind), TICK_DT)
    const s = server.carState(3)!
    server.dispose()
    const cm = (v: number) => Math.round(v * 100) / 100
    const unit = (v: number) => Math.round(v * 32767) / 32767
    const sign = s.rotation[3] < 0 ? -1 : 1
    const [qx, qy, qz] = [unit(s.rotation[0] * sign), unit(s.rotation[1] * sign), unit(s.rotation[2] * sign)]
    const milli = (v: number) => Math.round(v * 1000) / 1000
    return {
      vehicle: {
        id: 3, kind, x: cm(s.position[0]), y: cm(s.position[1]), z: cm(s.position[2]),
        qx, qy, qz, qw: Math.sqrt(1 - qx * qx - qy * qy - qz * qz), hp: vehicleMaxHp(kind), damage: [0, 0, 0, 0, 0, 0, 0, 0], air: null,
      },
      drive: {
        vx: cm(s.linvel[0]), vy: cm(s.linvel[1]), vz: cm(s.linvel[2]),
        wx: milli(s.angvel[0]), wy: milli(s.angvel[1]), wz: milli(s.angvel[2]),
        steer: milli(s.steer), spin: milli(s.spin),
      },
    }
  }

  it('replaying over the server state lands where an uninterrupted run does', async () => {
    for (const kind of [KIND_JEEP, KIND_TANK, KIND_HELI, KIND_PLANE]) {
      // The client is 20 ticks ahead of the last state the server has told it about.
      const motor = await field(200)
      const driver = new VehicleDriver(motor)
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
      motor.dispose()
    }
  })

  it('stops predicting when the player leaves the seat, and the vehicle is a solid again', async () => {
    const motor = await field(200)
    const driver = new VehicleDriver(motor)
    const state = await serverState(KIND_JEEP, 0)
    motor.setVehicles([state.vehicle])
    driver.reconcile(0, state.vehicle, state.drive)
    expect(driver.driving).toBe(3)
    expect(motor.carState(3)).not.toBeNull()
    driver.leave()
    expect(driver.driving).toBeNull()
    expect(driver.pose(0, 0)).toBeNull()
    expect(motor.carState(3)).toBeNull()
    motor.setVehicles([state.vehicle])
    const roof = motor.castRay([state.vehicle.x, 10, state.vehicle.z], [0, -1, 0], 20)!
    expect(10 - roof.t).toBeGreaterThan(1)
    motor.dispose()
  })
})

describe('a tank’s turret', () => {
  it('is dropped as a solid for a predicted tank and put back', async () => {
    const motor = await field(200)
    motor.startCar(CAR, KIND_TANK, [0, 0.94, 0])
    motor.driveCar(CAR, COAST, TICK_DT)
    motor.placeCar(CAR, { position: [0, 0.94, 0] })
    // The turret ring is at y 0.85 above the centre; the ray at 0.76 above the hull's top meets only it.
    const ray = () => motor.castRay([0, 1.7, 5.3], [0, 0, -1], 20)
    motor.driveCar(CAR, { ...COAST, handbrake: true }, TICK_DT)
    expect(ray()).not.toBeNull()
    motor.placeCar(CAR, { position: [0, 0.94, 0], turret: false })
    motor.driveCar(CAR, { ...COAST, handbrake: true }, TICK_DT)
    expect(ray()).toBeNull()
    motor.placeCar(CAR, { position: [0, 0.94, 0], turret: true })
    motor.driveCar(CAR, { ...COAST, handbrake: true }, TICK_DT)
    expect(ray()).not.toBeNull()
    motor.dispose()
  })
})

/** Mirrors `scripted_hen` in vehicle.rs: walk, leap, flap and glide forward, land. */
function henned(tick: number): CarControls {
  const driven = { ...COAST, powered: true }
  const second = Math.floor(tick / 60)
  if (second <= 1) return driven
  if (second === 2) return { ...driven, throttle: 1 }
  if (second <= 7) return { ...driven, hop: true, throttle: 1, steer: 0.3 }
  return driven
}

/** `HEN_CHECKPOINTS` in vehicle.rs: `[tick, position, stamina]`. */
const henCheckpoints: [number, [number, number, number], number][] = [
  [180, [0.0, 5.2071, -2.2903], 1.0],
  [240, [0.0279, 11.1432, -13.4641], 0.7835],
  [420, [4.6358, 13.9028, -50.7188], 0.0],
  [600, [20.7118, 5.2639, -82.4246], 0.4608],
]

describe('hen prediction', () => {
  it('WASM leaps, flaps and glides where the native server does', async () => {
    await run(KIND_HEN, [0, 5.4, 0], 600, henned, henCheckpoints)
  })

  it('maps jump to a leap for a hen, with no handbrake or climb', () => {
    expect(controlsOf(frame(BUTTON_JUMP), KIND_HEN)).toMatchObject({ hop: true, handbrake: false, lift: 0, boost: false })
    expect(controlsOf(frame(BUTTON_RUN), KIND_HEN)).toMatchObject({ hop: false, boost: false, lift: 0 })
  })
})
