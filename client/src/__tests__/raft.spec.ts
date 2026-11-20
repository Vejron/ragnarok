import { afterEach, describe, expect, it, vi } from 'vitest'
import { triangleCount } from './helpers'
import { kindView } from '@/kinds'
import { KIND_RAFT } from '@/net/codec'
import { RaftModel } from '@/render/raft'
import { RAFT_HALF_EXTENTS, RAFT_MOTOR_SWING, RAFT_SEATS, RAFT_TURN_RATE } from '@/sim/consts'
import { crewGun, seatHead, vehicleDef } from '@/sim/vehicles'

/** The raft's rotation turned `yaw` radians about +Y, as a quaternion. */
const turned = (yaw: number): [number, number, number, number] => [0, Math.sin(yaw / 2), 0, Math.cos(yaw / 2)]

/** Draws `raft` for `frames` frames 1/60 s apart, turning at `rate` rad/s and going `speed` m/s north. */
function sail(raft: RaftModel, frames: number, rate: number, speed: number, seats: readonly number[]) {
  let clock = 1000
  const now = vi.spyOn(performance, 'now').mockImplementation(() => clock)
  for (let k = 0; k <= frames; k++) {
    raft.update([0, 0, (-speed * k) / 60], turned((rate * k) / 60), 0, seats)
    clock += 1000 / 60
  }
  now.mockRestore()
}

const outboard = (raft: RaftModel) => (raft as unknown as { motor: { rotation: { y: number } } }).motor.rotation.y
const propeller = (raft: RaftModel) => (raft as unknown as { prop: { rotation: { z: number } } }).prop.rotation.z

afterEach(() => vi.restoreAllMocks())

describe('RaftModel', () => {
  it('stays low-poly', () => {
    const raft = new RaftModel(1)
    expect(triangleCount(raft.root)).toBeLessThan(2500)
    raft.dispose()
  })

  it('swings its outboard the way its driver turns it, and holds it straight let be', () => {
    const right = new RaftModel(1)
    // Turning right is the yaw falling: the propeller swings out to the right (+Y turn of the motor).
    sail(right, 60, -RAFT_TURN_RATE, 8, [5, 0])
    expect(outboard(right)).toBeGreaterThan(0.6 * RAFT_MOTOR_SWING)
    const left = new RaftModel(2)
    sail(left, 60, RAFT_TURN_RATE, 8, [5, 0])
    expect(outboard(left)).toBeLessThan(-0.6 * RAFT_MOTOR_SWING)
    const straight = new RaftModel(3)
    sail(straight, 60, 0, 8, [5, 0])
    expect(Math.abs(outboard(straight))).toBeLessThan(1e-6)
    for (const raft of [right, left, straight]) raft.dispose()
  })

  it('spins its propeller only while someone has the tiller', () => {
    const manned = new RaftModel(1)
    sail(manned, 7, 0, 5, [5, 0])
    expect(propeller(manned)).not.toBe(0)
    const empty = new RaftModel(1)
    sail(empty, 7, 0, 5, [0, 6])
    expect(propeller(empty)).toBe(0)
    for (const raft of [manned, empty]) raft.dispose()
  })
})

describe('the raft as a kind', () => {
  it('seats a driver at the tiller and a rider in the bow who fires their own gun, both in the open', () => {
    const { seats } = kindView(KIND_RAFT).ride
    expect(seats).toHaveLength(RAFT_SEATS)
    expect(seats.map((seat) => seat.role)).toEqual(['driver', 'passenger'])
    expect([0, 1].map((seat) => crewGun(KIND_RAFT, seat))).toEqual([false, true])
    expect(vehicleDef(KIND_RAFT).crew).toBe('open')
    for (let seat = 0; seat < RAFT_SEATS; seat++) {
      const head = seatHead(KIND_RAFT, seat)!
      expect(Math.abs(head[0])).toBeLessThan(RAFT_HALF_EXTENTS[0])
      expect(Math.abs(head[2])).toBeLessThan(RAFT_HALF_EXTENTS[2])
    }
    expect(kindView(KIND_RAFT).fx.floats).toBe(true)
  })
})
