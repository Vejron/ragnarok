import { describe, expect, it } from 'vitest'
import { VehicleDriver } from '@/sim/driver'
import { controlsOf, SimMotor, type CarControls, type Neighbour, type VehiclePose } from '@/sim/motor'
import { heightIn, type TileGround } from '@/sim/terrain'
import { f32 } from '@/sim/round'
import { BUTTON_CROUCH, BUTTON_JUMP, BUTTON_RUN, LOITER_SPEED, TICK_DT, vehicleMaxHp } from '@/sim/consts'
import { type CarvedBuilding, KIND_AIR_AMBULANCE, KIND_DRONE, KIND_HELI, KIND_HEN, KIND_JEEP, KIND_LOITER, KIND_PLANE, KIND_ROADSTER, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_WALKER, type InputFrame } from '@/net/codec'
import type { DriveState } from '@/entities/units'
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
  // `[tick, position, tank, slack]`: a checkpoint with its own `slack` (metres) is held to that, not to `tolerance`.
  checkpoints: readonly (readonly [number, readonly number[], number?, number?])[],
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
      expect(Math.abs(value - expected[1][axis]!), `kind ${kind} tick ${tick + 1} axis ${axis}`).toBeLessThan(expected[3] ?? tolerance)
    })
    if (expected[2] !== undefined) expect(state.tank!).toBeCloseTo(expected[2], 3)
  }
  motor.dispose()
}

const frame = (buttons: number, rest: Partial<InputFrame> = {}): InputFrame => ({ seq: 1, strafe: 0, forward: 0, yaw: 0, buttons, pitch: 0, view: 0, slot: 0, collective: 0, ...rest })

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
  [KIND_JEEP, 300, [30.2642, 0.8562, -23.7819]],
  [KIND_JEEP, 480, [59.2548, 0.8568, -59.0352]],
  [KIND_SPORTS, 90, [-0.0004, 0.4175, -1.5324]],
  [KIND_SPORTS, 210, [4.4973, 0.4166, -34.9608]],
  [KIND_SPORTS, 300, [42.8601, 0.4161, -25.5642]],
  [KIND_SPORTS, 480, [93.0031, 0.4166, -71.6836]],
  [KIND_TANK, 90, [0, 1.13, -0.949]],
  [KIND_TANK, 210, [0.5394, 1.13, -17.9451]],
  [KIND_TANK, 300, [10.8795, 1.13, -28.9229]],
  [KIND_TANK, 480, [23.109, 1.13, -38.4465]],
  [KIND_STRV, 90, [0, 1.2, -1.139]],
  [KIND_STRV, 210, [0.8057, 1.2, -22.6697]],
  [KIND_STRV, 300, [16.8312, 1.2, -33.1654]],
  [KIND_STRV, 480, [38.4407, 1.2, -39.3431]],
]

/** Same as `scripted_flight` in vehicle.rs. */
const PILOTED: CarControls = { ...COAST, powered: true }
const flying = (lift: number, look: number): CarControls => ({ ...PILOTED, lift, aim: [look, 0] })
const flight = (tick: number): CarControls => {
  const step = Math.floor(tick / 30)
  if (step <= 3) return flying(0, 0)
  if (step <= 7) return flying(1, 0)
  if (step === 8) return { ...flying(0.7, 0), throttle: 0.6 }
  if (step <= 11) return flying(0.7, 0)
  if (step === 12) return { ...flying(0.6, -1), steer: -0.5 }
  if (step === 13) return flying(0.6, -1)
  if (step === 14) return { ...flying(0.4, -1), throttle: -0.6 }
  return flying(0.4, -1)
}

/** `FLIGHT_CHECKPOINTS` in vehicle.rs: a helicopter started at [0, 3, 0] in the wind, its rotor stopped. */
const flightCheckpoints: [number, [number, number, number]][] = [
  [90, [0.0717, 0.7999, 0.0029]],
  [210, [0.9357, 3.1621, 0.0317]],
  [330, [6.8222, 15.8349, -0.7983]],
  [480, [18.4246, 24.7133, -10.4861]],
]

/** `AMBULANCE_CHECKPOINTS` in vehicle.rs: an air ambulance flown the same, whose solids the gunship has none of. */
const ambulanceCheckpoints: [number, [number, number, number]][] = [
  [90, [0.0502, 1.0999, 0.0014]],
  [210, [0.8792, 2.8737, 0.0216]],
  [330, [6.4967, 12.076, -0.817]],
  [480, [17.4981, 15.9223, -10.5615]],
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
  // The corner with no wheel drags its hull's swept underside along the ground, and the two builds'
  // arithmetic differs in the last bit, which that sliding contact grows: a few centimetres after
  // five seconds, a couple of metres after eight (the server's corrections never let it get that far).
  const damagedCheckpoints: [number, [number, number, number], undefined, number][] = [
    [90, [0.1301, 0.764, -0.3042], undefined, 0.01],
    [300, [8.7144, 0.7641, -14.3911], undefined, 0.05],
    [480, [12.9401, 0.7384, -28.2222], undefined, 2.5],
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
const thrustCheckpoints: [number, [number, number, number], number, number?][] = [
  [120, [-0.0028, 0.3968, -8.9719], 1.0],
  [180, [-0.0028, 0.3992, -46.4499], 0.67],
  [240, [0.8566, 4.5892, -100.0662], 0.2742],
  // After the hop it lands on its belly and slides on the handbrake, where the two builds drift apart.
  [420, [20.8949, 0.3971, -188.5134], 0.6808, 1.0],
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

  it('WASM flies the air ambulance where the native server does', async () => {
    await run(KIND_AIR_AMBULANCE, [0, 3, 0], 480, flight, ambulanceCheckpoints)
  })

  it('reads the collective lever from the input, not jump or run, as on the server', () => {
    expect(controlsOf(frame(0, { collective: 255 }), KIND_HELI)).toMatchObject({ lift: 1, powered: true, handbrake: false })
    expect(controlsOf(frame(BUTTON_JUMP), KIND_HELI)).toMatchObject({ lift: 0 })
    expect(controlsOf(frame(0, { collective: 255 }), KIND_AIR_AMBULANCE)).toMatchObject({ lift: 1, powered: true, handbrake: false })
    expect(controlsOf(frame(BUTTON_RUN, { collective: 51 }), KIND_HELI).lift).toBeCloseTo(0.2, 6)
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
    [360, [0, 29.4666, -34.4876]],
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
    seq, strafe: Math.round(Math.sin(seq / 25) * 100), forward: seq < 200 ? 127 : -127, yaw: 0, pitch: 0, view: 0, slot: 0, collective: 0, buttons: seq % 150 === 0 ? 1 : 0,
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
        qx, qy, qz, qw: Math.sqrt(1 - qx * qx - qy * qy - qz * qz), hp: vehicleMaxHp(kind), damage: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], air: null,
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
      expect(driver.lastCorrection, `kind ${kind}`).toBeLessThan(0.1)
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

  /** A vehicle of `kind` standing empty `ahead` metres in front of the one `serverState` gives, as a snapshot shows it. */
  const parked = (from: VehiclePose, id: number, kind: number, ahead: number): Neighbour => ({ ...from, id, kind, z: from.z - ahead, hp: vehicleMaxHp(kind), vx: 0, vz: 0, steer: 0, throttle: 0, driven: false })
  /** Full throttle, straight ahead. */
  const ahead = (seq: number): InputFrame => ({ seq, strafe: 0, forward: 127, yaw: 0, pitch: 0, view: 0, slot: 0, collective: 0, buttons: 0 })

  it('shoves a parked car simulated beside it, where a stand-in stops it dead', async () => {
    const state = await serverState(KIND_JEEP, 0)
    const other = parked(state.vehicle, 4, KIND_JEEP, 15)
    /** Where the driven jeep and the parked one are along z after four seconds of full throttle. */
    const ram = async (beside: boolean) => {
      const motor = await field(200)
      const driver = new VehicleDriver(motor)
      motor.setVehicles([state.vehicle, other])
      driver.reconcile(0, state.vehicle, state.drive, undefined, null, beside ? driver.near(state.vehicle, [other]) : [])
      for (let seq = 1; seq <= 240; seq++) driver.apply(ahead(seq))
      const ends = [motor.carState(3)!.position[2], motor.carState(4)?.position[2] ?? other.z]
      driver.dispose()
      motor.dispose()
      return ends
    }
    const [stopped, stood] = await ram(false)
    const [pushing, pushed] = await ram(true)
    expect(stood).toBe(other.z)
    expect(pushed).toBeLessThan(other.z - 0.5)
    expect(pushing).toBeLessThan(stopped! - 0.5)
  })

  it('simulates only the vehicles near it that stay on their wheels, and lets them go when the player leaves', async () => {
    const motor = await field(200)
    const driver = new VehicleDriver(motor)
    const state = await serverState(KIND_JEEP, 0)
    const [close, heli, edge, far] = [parked(state.vehicle, 4, KIND_JEEP, 10), parked(state.vehicle, 5, KIND_HELI, 20), parked(state.vehicle, 6, KIND_JEEP, 35), parked(state.vehicle, 7, KIND_JEEP, 60)]
    const others = [far, edge, heli, close]
    // Nearest first, and not the driven one itself; a helicopter is offered but stays a stand-in.
    expect(driver.near(state.vehicle, [...others, { ...close, ...state.vehicle }]).map((car) => car.id)).toEqual([4, 5])
    driver.reconcile(0, state.vehicle, state.drive, undefined, null, driver.near(state.vehicle, others))
    expect(motor.carState(4)).not.toBeNull()
    expect(motor.carState(5)).toBeNull()
    // One already simulated is kept a little further out than a new one is taken.
    driver.reconcile(0, state.vehicle, state.drive, undefined, null, [close, edge])
    expect(driver.near(state.vehicle, others).map((car) => car.id)).toEqual([4, 5, 6])
    driver.reconcile(0, state.vehicle, state.drive, undefined, null, [close])
    expect(motor.carState(6)).toBeNull()
    driver.leave()
    expect(motor.carState(4)).toBeNull()
    motor.dispose()
  })
})

describe('a predicted ram through a wall', () => {
  /** A motor that only counts: its vehicle gains 1 m/s along x a tick, and breaks through a wall on the ticks in `breaks`, which halves its speed. */
  function fake(breaks: Set<number>) {
    const state = { tick: 0, vx: 0, forgotten: 0 }
    const hole: CarvedBuilding = { col: 1, row: 2, index: 3, point: [0, 1, 0], blasted: [7], fell: [8] }
    const motor = {
      startCar() {},
      stopCar() {},
      setNeighbours: () => new Set<number>(),
      setCarState() {
        state.vx = 0
      },
      driveCar() {
        state.tick += 1
        state.vx += 1
        if (!breaks.has(state.tick)) return false
        state.vx /= 2
        return true
      },
      setCarVelocity(_id: number, velocity: readonly number[]) {
        state.vx = velocity[0]!
      },
      carState: () => ({ position: [0, 0, 0], rotation: [0, 0, 0, 1], linvel: [state.vx, 0, 0], angvel: [0, 0, 0], steer: 0, spin: 0, tank: null, firing: 0, steerFraction: 0 }),
      takeRams: () => [hole],
      forgetRams() {
        state.forgotten += 1
      },
    }
    return { state, motor: motor as unknown as SimMotor }
  }
  const vehicle: VehiclePose = { id: 3, kind: KIND_JEEP, x: 0, y: 0, z: 0, qx: 0, qy: 0, qz: 0, qw: 1, hp: 255, damage: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], air: null }
  const drive: DriveState = { vx: 0, vy: 0, vz: 0, wx: 0, wy: 0, wz: 0, steer: 0, spin: 0 }
  const input = (seq: number): InputFrame => ({ seq, strafe: 0, forward: 127, yaw: 0, pitch: 0, view: 0, slot: 0, collective: 0, buttons: 0 })

  it('is slowed again at the same input by a replay that finds the hole already made', () => {
    // The wall breaks on the fourth tick only: the replay's fourth input drives through the hole.
    const { state, motor } = fake(new Set([5]))
    const driver = new VehicleDriver(motor)
    driver.reconcile(0, vehicle, drive)
    state.tick = 1
    for (let seq = 1; seq <= 6; seq++) driver.apply(input(seq))
    // 1, 2, 3, then (3 + 1) / 2 = 2, then 3, 4.
    expect(state.vx).toBe(4)
    driver.reconcile(0, vehicle, drive)
    expect(state.vx).toBe(4)
    // Once the server has stepped the ram, its own state carries the slowing.
    driver.reconcile(4, vehicle, drive)
    expect(state.vx).toBe(2)
  })

  it('has its hole closed when the server steps well past it without a word, or the player leaves', () => {
    const { state, motor } = fake(new Set([2]))
    const driver = new VehicleDriver(motor)
    // Whoever draws the world is handed the hole as it is broken, and told when it is given up.
    const seen: string[] = []
    driver.onBreach = (holes) => seen.push(`hole in ${holes[0]!.index}`)
    driver.onForget = () => seen.push('forgotten')
    driver.reconcile(0, vehicle, drive)
    state.tick = 1
    driver.apply(input(1))
    expect(seen).toEqual(['hole in 3'])
    driver.acknowledge(40)
    expect(state.forgotten).toBe(0)
    driver.acknowledge(61)
    expect(state.forgotten).toBe(1)
    expect(seen).toEqual(['hole in 3', 'forgotten'])
    driver.acknowledge(200)
    expect(state.forgotten).toBe(1)
    state.tick = 1
    driver.apply(input(201))
    driver.leave()
    expect(state.forgotten).toBe(2)
  })
})

describe('a tank’s turret', () => {
  it('is dropped as a solid for a predicted tank and put back', async () => {
    const motor = await field(200)
    motor.startCar(CAR, KIND_TANK, [0, 1.13, 0])
    motor.driveCar(CAR, COAST, TICK_DT)
    motor.placeCar(CAR, { position: [0, 1.13, 0] })
    // The turret ring is at y 1.125 above the centre; the ray at 0.5 above the hull's top meets only it.
    const ray = () => motor.castRay([0, 2.26, 8], [0, 0, -1], 20)
    motor.driveCar(CAR, { ...COAST, handbrake: true }, TICK_DT)
    expect(ray()).not.toBeNull()
    motor.placeCar(CAR, { position: [0, 1.13, 0], turret: false })
    motor.driveCar(CAR, { ...COAST, handbrake: true }, TICK_DT)
    expect(ray()).toBeNull()
    motor.placeCar(CAR, { position: [0, 1.13, 0], turret: true })
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
