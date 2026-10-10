import { describe, expect, it } from 'vitest'
import { Predictor } from '@/sim/predict'
import { RapierMotor, stepCharacter, type CharacterMotor, type CharacterState, type Vec3 } from '@/sim/physics'
import { heightIn, type TileGround } from '@/sim/terrain'
import { f32 } from '@/sim/movement'
import { BUTTON_JUMP, BUTTON_RUN, STANDING_Y, TANK_HALF_EXTENTS, TICK_DT, TICK_RATE, WALK_SPEED } from '@/sim/consts'
import { KIND_TANK, type InputFrame } from '@/net/codec'
import type { TileData, WorldData } from '@/world'

/** A floor at y = 0 and nothing else, in place of Rapier. */
const flat: CharacterMotor = {
  move(position: Vec3, desired: Vec3) {
    const floor = STANDING_Y
    const y = Math.max(floor, position[1] + desired[1])
    return { translation: [desired[0], y - position[1], desired[2]], grounded: y <= floor + 1e-6 }
  },
}

const standing = (): CharacterState => ({ position: [0, STANDING_Y, 0], yaw: 0, vy: 0, grounded: true, knock: [0, 0], ride: 0 })
const forward = (seq: number, buttons = 0): InputFrame => ({ seq, strafe: 0, forward: 127, yaw: 0, buttons, pitch: 0, view: 0 })

/** What the server would compute: the same inputs from the same start. */
function serverRun(inputs: InputFrame[]): CharacterState {
  return inputs.reduce((state, input) => stepCharacter(flat, state, input, TICK_DT), standing())
}

describe('Predictor', () => {
  it('a correct prediction produces no correction', () => {
    const predictor = new Predictor(flat, standing())
    const inputs = Array.from({ length: 10 }, (_, i) => forward(i + 1, i === 2 ? BUTTON_JUMP : 0))
    inputs.forEach((input) => predictor.apply(input))
    const before = [...predictor.state.position]
    // Server has processed the first 6 inputs.
    predictor.reconcile(6, serverRun(inputs.slice(0, 6)))
    expect(predictor.pendingCount).toBe(4)
    predictor.state.position.forEach((value, i) => expect(value).toBeCloseTo(before[i]!, 6))
    expect(predictor.lastCorrection).toBeLessThan(1e-6)
  })

  it('inputs recorded in a car are replayed over the state after stepping out', () => {
    const predictor = new Predictor(flat, standing())
    predictor.apply(forward(1))
    const parked = [...predictor.state.position]
    // Seq 2..5 are sent while the player sits in the car; the server has applied only 2 and 3 to the car.
    for (let seq = 2; seq <= 5; seq++) predictor.record(forward(seq))
    expect(predictor.state.position).toEqual(parked)
    predictor.acknowledge(3)
    expect(predictor.pendingCount).toBe(2)
    // The server stepped the player out at seq 3 and reports where; 4 and 5 walk on from there.
    const outside: CharacterState = { position: [10, STANDING_Y, 10], yaw: 0, vy: 0, grounded: true, knock: [0, 0], ride: 0 }
    predictor.reconcile(3, outside)
    const expected = [forward(4), forward(5)].reduce((state, input) => stepCharacter(flat, state, input, TICK_DT), outside)
    expect(predictor.state.position).toEqual(expected.position)
    // Far from where it was parked: a snap, not a slide.
    expect(predictor.renderPosition(1, 0)[0]).toBeCloseTo(expected.position[0], 5)
  })

  it('a misprediction is corrected and smoothed, not snapped', () => {
    const predictor = new Predictor(flat, standing())
    const inputs = Array.from({ length: 10 }, (_, i) => forward(i + 1))
    inputs.forEach((input) => predictor.apply(input))
    // The server says we were pushed 0.5 m sideways by the time it applied input 5.
    const server = serverRun(inputs.slice(0, 5))
    server.position[0] += 0.5
    predictor.reconcile(5, server)
    expect(predictor.state.position[0]).toBeCloseTo(0.5, 6)
    expect(predictor.lastCorrection).toBeCloseTo(0.5, 6)
    // Visually we have not moved yet; the offset then decays away.
    expect(predictor.renderPosition(1, 0)[0]).toBeCloseTo(0, 6)
    expect(Math.abs(predictor.renderPosition(1, 1)[0] - 0.5)).toBeLessThan(0.01)
  })

  it('a teleport snaps', () => {
    const predictor = new Predictor(flat, standing())
    predictor.apply(forward(1))
    predictor.reconcile(1, { ...standing(), position: [100, STANDING_Y, 100] })
    expect(predictor.renderPosition(1, 0)[0]).toBeCloseTo(100)
  })
})

describe('RapierMotor cars', () => {
  const empty: WorldData = {
    origin: [0, 0], halfExtent: 100, tileSize: 100, spawns: [], tiles: [], groundCell: 0,
    collision: { vertices: new Float32Array(), triangles: new Uint32Array() },
  }
  const car = (x: number) => ({ id: 1, x, y: 0.66, z: 0, qx: 0, qy: 0, qz: 0, qw: 1 })

  // Mirrors `cars_are_solid_to_characters` in vehicle.rs: a 1.8 m wide car at the origin.
  it('blocks the stickman like the server does, and lets go when the car leaves', async () => {
    const motor = await RapierMotor.create(empty)
    motor.setVehicles([car(0)])
    let state: CharacterState = { ...standing(), position: [-4, STANDING_Y, 0] }
    // Yaw -π/2 faces +X, into the car's flank.
    const towardCar = (seq: number): InputFrame => ({ seq, strafe: 0, forward: 127, yaw: -1571, buttons: 0, pitch: 0, view: 0 })
    for (let i = 0; i < 180; i++) state = stepCharacter(motor, state, towardCar(i), TICK_DT)
    expect(state.position[0]).toBeLessThan(-0.9 - 0.35 + 0.1)
    expect(state.position[0]).toBeGreaterThan(-3)
    motor.setVehicles([])
    for (let i = 0; i < 120; i++) state = stepCharacter(motor, state, towardCar(i), TICK_DT)
    expect(state.position[0]).toBeGreaterThan(1)
  })

  it('makes a tank wider than a car', async () => {
    const motor = await RapierMotor.create(empty)
    motor.setVehicles([{ ...car(0), kind: KIND_TANK }])
    let state: CharacterState = { ...standing(), position: [-4, STANDING_Y, 0] }
    const towardTank = (seq: number): InputFrame => ({ seq, strafe: 0, forward: 127, yaw: -1571, buttons: 0, pitch: 0, view: 0 })
    for (let i = 0; i < 180; i++) state = stepCharacter(motor, state, towardTank(i), TICK_DT)
    // The tank's half width is 1.3 m: the stickman stops there, not at the car's 0.9 m.
    expect(state.position[0]).toBeLessThan(-TANK_HALF_EXTENTS[0] - 0.35 + 0.1)
    expect(state.position[0]).toBeGreaterThan(-3)
  })
})

describe('RapierMotor', () => {
  const world: WorldData = {
    origin: [0, 0],
    halfExtent: 100,
    tileSize: 100, spawns: [], tiles: [], groundCell: 0,
    collision: { vertices: new Float32Array(), triangles: new Uint32Array() },
  }

  // Mirrors `walks_forward_at_walk_speed` and `falls_and_lands_on_the_ground` in physics.rs.
  it('walks and lands like the server', async () => {
    const motor = await RapierMotor.create(world)
    let state = standing()
    for (let i = 0; i < TICK_RATE; i++) state = stepCharacter(motor, state, forward(i + 1), TICK_DT)
    expect(state.position[2]).toBeCloseTo(-WALK_SPEED, 1)
    expect(state.grounded).toBe(true)

    state = { ...standing(), position: [0, 3, 0], grounded: false }
    for (let i = 0; i < 120; i++) state = stepCharacter(motor, state, { seq: i, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: 0, view: 0 }, TICK_DT)
    expect(state.grounded).toBe(true)
    expect(state.position[1]).toBeCloseTo(STANDING_Y, 1)
  })
})

describe('knockback and cars', () => {
  it('a hit that only the server knew about is picked up from the snapshot and keeps sliding', () => {
    const predictor = new Predictor(flat, standing())
    const idle = (seq: number): InputFrame => ({ seq, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: 0, view: 0 })
    for (let seq = 1; seq <= 6; seq++) predictor.apply(idle(seq))
    // By input 3 a car had thrown the player 4 m/s towards +X and up.
    const hit: CharacterState = { position: [0.2, STANDING_Y + 0.3, 0], yaw: 0, vy: 3, grounded: false, knock: [4, 0], ride: 0 }
    predictor.reconcile(3, hit)
    const expected = [4, 5, 6].reduce((state, seq) => stepCharacter(flat, state, idle(seq), TICK_DT), hit)
    expect(predictor.state.position).toEqual(expected.position)
    expect(predictor.state.position[0]).toBeGreaterThan(0.2 + 3 * 4 * TICK_DT * 0.9)
    expect(predictor.state.knock[0]).toBeGreaterThan(3.5)
  })

  it('knockback fades out on the ground', () => {
    let state: CharacterState = { ...standing(), knock: [6, 0] }
    for (let seq = 1; seq <= 240; seq++) state = stepCharacter(flat, state, { seq, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: 0, view: 0 }, TICK_DT)
    expect(state.knock).toEqual([0, 0])
    expect(state.position[0]).toBeGreaterThan(0.5)
  })
})

describe('riding a car', () => {
  /** A flat floor, plus a car going 10 m/s along -Z that the stickman may stand on. */
  const onCar: CharacterMotor = { ...flat, vehicleVelocity: () => [0, -10] }
  const riding = (): CharacterState => ({ ...standing(), ride: 7 })
  const frame = (buttons: number, seq = 1): InputFrame => ({ seq, strafe: 0, forward: 0, yaw: 0, buttons, pitch: 0, view: 0 })

  it('keeps the car\'s speed when jumping off, like the server', () => {
    const state = stepCharacter(onCar, riding(), frame(BUTTON_JUMP), TICK_DT)
    expect(state.grounded).toBe(false)
    expect(state.ride).toBe(0)
    // 10 m/s minus one tick of air decay.
    expect(state.knock[1]).toBeCloseTo(-10 * 0.99, 4)
    expect(state.position[2]).toBeLessThan(-0.15)
  })

  it('keeps the car\'s speed when walking off its edge into the air', () => {
    const edge: CharacterMotor = { ...onCar, move: (_position, desired) => ({ translation: desired, grounded: false }) }
    const state = stepCharacter(edge, riding(), frame(0), TICK_DT)
    expect(state.ride).toBe(0)
    expect(state.knock[1]).toBeCloseTo(-10 * 0.99, 4)
  })

  it('stays on the car while walking about on it, and gains nothing', () => {
    const state = stepCharacter(onCar, riding(), frame(0), TICK_DT)
    expect(state.ride).toBe(7)
    expect(state.knock).toEqual([0, 0])
  })

  it('a stickman on foot ignores cars when jumping', () => {
    const state = stepCharacter(onCar, standing(), frame(BUTTON_JUMP), TICK_DT)
    expect(state.knock).toEqual([0, 0])
  })
})

describe('cross-language physics', () => {
  /** The 1 × 2 × 2 m box from `world_with_wall` in `crates/server/src/physics.rs`. */
  const wallWorld = (): WorldData => {
    const v = [[5, 0, -1], [6, 0, -1], [6, 0, 1], [5, 0, 1], [5, 2, -1], [6, 2, -1], [6, 2, 1], [5, 2, 1]]
    const t = [[0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2]]
    return {
      origin: [0, 0], halfExtent: 100, tileSize: 100, spawns: [], tiles: [], groundCell: 0,
      collision: { vertices: Float32Array.from(v.flat()), triangles: Uint32Array.from(t.flat()) },
    }
  }

  /** Same as `scripted_input` in physics.rs. */
  const scripted = (seq: number): InputFrame => {
    const yaw = -1571 + (((seq * 13) % 1200) - 600)
    const buttons = seq % 90 === 30 ? BUTTON_JUMP : seq > 150 ? BUTTON_RUN : 0
    return { seq, strafe: (seq % 7) * 10 - 30, forward: 127, yaw, buttons, pitch: 0, view: 0 }
  }

  it('WASM prediction lands where the native server does', async () => {
    const motor = await RapierMotor.create(wallWorld())
    let state = standing()
    // `SCRIPTED_CHECKPOINTS` in physics.rs. The engines agree to well under a millimetre per
    // tick, but tiny differences accumulate. Reconciliation only ever replays about an RTT of
    // ticks on top of fresh server state, so the short-horizon checkpoints are what matter.
    const checkpoints = new Map<number, { position: Vec3; tolerance: number }>([
      [30, { position: [1.8138, 0.9722, 0.748], tolerance: 0.01 }],
      [60, { position: [3.7758, 1.4722, 0.7701], tolerance: 0.01 }],
      [90, { position: [4.6299, 0.8649, 0.0827], tolerance: 0.03 }],
      [300, { position: [4.6295, 0.9728, 0.4872], tolerance: 0.15 }],
    ])
    for (let seq = 1; seq <= 300; seq++) {
      state = stepCharacter(motor, state, scripted(seq), TICK_DT)
      const checkpoint = checkpoints.get(seq)
      if (!checkpoint) continue
      state.position.forEach((value, axis) => expect(Math.abs(value - checkpoint.position[axis]!), `tick ${seq} axis ${axis}`).toBeLessThan(checkpoint.tolerance))
    }
    expect(state.grounded).toBe(false)
  })
})

describe('cross-language terrain', () => {
  /** The 20° ramp of `ramp_world` in physics.rs: 5 m cells over 200 m, rising towards +Z. */
  const side = 41
  const ramp = (): TileGround => {
    const slope = Math.tan((20 * Math.PI) / 180)
    const heights = new Float32Array(side * side)
    for (let row = 0; row < side; row++) for (let column = 0; column < side; column++) heights[row * side + column] = Math.round(f32(row * 5 * slope) * 100) / 100
    return { x0: -100, z0: -100, cell: 5, side, heights }
  }
  const emptyTile: TileData = { buildings: [], roads: [], cover: [], collision: { vertices: new Float32Array(), triangles: new Uint32Array() }, ground: new Float32Array() }
  const world = (): WorldData => ({
    origin: [0, 0], halfExtent: 100, tileSize: 200, spawns: [], tiles: [], groundCell: 5,
    collision: { vertices: new Float32Array(), triangles: new Uint32Array() },
  })

  it('samples a plane exactly and clamps outside the grid', () => {
    const ground = ramp()
    const slope = Math.tan((20 * Math.PI) / 180)
    expect(heightIn(ground, 0, 0)).toBeCloseTo(100 * slope, 1)
    expect(heightIn(ground, 3.3, -61.7)).toBeCloseTo(38.3 * slope, 1)
    expect(heightIn(ground, 0, -500)).toBe(0)
  })

  it('WASM prediction on a slope lands where the native server does', async () => {
    const motor = await RapierMotor.create(world())
    motor.addTile(emptyTile, ramp())
    const ground = heightIn(ramp(), 0, 40)
    let state: CharacterState = { ...standing(), position: [0, ground + STANDING_Y, 40] }
    // `RAMP_CHECKPOINTS` in physics.rs.
    const checkpoints = new Map<number, Vec3>([
      [30, [-0.018, 50.4969, 36.2721]],
      [60, [-0.0733, 50.4621, 32.5448]],
      [120, [-0.2956, 46.4279, 25.0934]],
      [240, [-0.4475, 45.1151, 21.4869]],
    ])
    for (let seq = 1; seq <= 240; seq++) {
      const buttons = seq === 50 ? BUTTON_RUN | BUTTON_JUMP : seq < 150 ? BUTTON_RUN : 0
      const input = { seq, strafe: (seq % 5) * 10 - 20, forward: seq < 150 ? 127 : 0, yaw: Math.trunc(seq / 3) % 200, buttons, pitch: 0, view: 0 }
      state = stepCharacter(motor, state, input, TICK_DT)
      const expected = checkpoints.get(seq)
      if (!expected) continue
      state.position.forEach((value, axis) => expect(Math.abs(value - expected[axis]!), `tick ${seq} axis ${axis}`).toBeLessThan(0.01))
    }
  })
})
