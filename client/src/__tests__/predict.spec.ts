import { describe, expect, it } from 'vitest'
import { Predictor } from '@/sim/predict'
import { RapierMotor, stepCharacter, type CharacterMotor, type CharacterState, type Vec3 } from '@/sim/physics'
import { BUTTON_JUMP, BUTTON_RUN, STANDING_Y, TICK_DT, TICK_RATE, WALK_SPEED } from '@/sim/consts'
import type { InputFrame } from '@/net/codec'
import type { WorldData } from '@/world'

/** A floor at y = 0 and nothing else, in place of Rapier. */
const flat: CharacterMotor = {
  move(position: Vec3, desired: Vec3) {
    const floor = STANDING_Y
    const y = Math.max(floor, position[1] + desired[1])
    return { translation: [desired[0], y - position[1], desired[2]], grounded: y <= floor + 1e-6 }
  },
}

const standing = (): CharacterState => ({ position: [0, STANDING_Y, 0], yaw: 0, vy: 0, grounded: true })
const forward = (seq: number, buttons = 0): InputFrame => ({ seq, strafe: 0, forward: 127, yaw: 0, buttons })

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

describe('RapierMotor', () => {
  const world: WorldData = {
    origin: [0, 0],
    halfExtent: 100,
    buildings: [],
    roads: [],
    spawns: [],
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
    for (let i = 0; i < 120; i++) state = stepCharacter(motor, state, { seq: i, strafe: 0, forward: 0, yaw: 0, buttons: 0 }, TICK_DT)
    expect(state.grounded).toBe(true)
    expect(state.position[1]).toBeCloseTo(STANDING_Y, 1)
  })
})

describe('cross-language physics', () => {
  /** The 1 × 2 × 2 m box from `world_with_wall` in `crates/server/src/physics.rs`. */
  const wallWorld = (): WorldData => {
    const v = [[5, 0, -1], [6, 0, -1], [6, 0, 1], [5, 0, 1], [5, 2, -1], [6, 2, -1], [6, 2, 1], [5, 2, 1]]
    const t = [[0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2]]
    return {
      origin: [0, 0], halfExtent: 100, buildings: [], roads: [], spawns: [],
      collision: { vertices: Float32Array.from(v.flat()), triangles: Uint32Array.from(t.flat()) },
    }
  }

  /** Same as `scripted_input` in physics.rs. */
  const scripted = (seq: number): InputFrame => {
    const yaw = -1571 + (((seq * 13) % 1200) - 600)
    const buttons = seq % 90 === 30 ? BUTTON_JUMP : seq > 150 ? BUTTON_RUN : 0
    return { seq, strafe: (seq % 7) * 10 - 30, forward: 127, yaw, buttons }
  }

  it('WASM prediction lands where the native server does', async () => {
    const motor = await RapierMotor.create(wallWorld())
    let state = standing()
    // `SCRIPTED_CHECKPOINTS` in physics.rs. The engines agree to well under a millimetre per
    // tick, but tiny differences accumulate. Reconciliation only ever replays about an RTT of
    // ticks on top of fresh server state, so the short-horizon checkpoints are what matter.
    const checkpoints = new Map<number, { position: Vec3; tolerance: number }>([
      [30, { position: [1.8128, 0.972, 0.7471], tolerance: 0.01 }],
      [60, { position: [3.7747, 1.472, 0.7692], tolerance: 0.01 }],
      [90, { position: [4.6299, 0.8701, 0.0616], tolerance: 0.01 }],
      [300, { position: [4.6299, 0.9729, 0.5537], tolerance: 0.05 }],
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
