import { describe, expect, it } from 'vitest'
import init, { ClientSim } from '@/sim/wasm/sim'
import { LiftPath, STRETCH_TOP, STRETCH_UP } from '@/sim/lifts'
import { LIFT_SPEED, TICK_RATE } from '@/sim/consts'
import type { Lift } from '@/world'

// The client draws the chairs from its own copy of `crates/shared/src/lifts.rs`; the server carries
// riders on the Rust one. They must be the same chairs, or a rider sits beside the chair drawn.
describe('lifts', () => {
  // A bent lift over two pylons, in centimetres as the manifest has it.
  const points = [0, 0, 1_500, -9_000, 2_000, -21_000, 2_100, -30_000]
  const cable = [310, 1_400, 2_900, 3_510]
  const lift: Lift = {
    kind: 0,
    seats: 4,
    points: [0, 1, 2, 3].map((i) => [points[i * 2]! / 100, points[i * 2 + 1]! / 100]),
    base: [0, 0, 0, 0],
    cable: cable.map((h) => h / 100),
  }

  it('put each chair where the sim does', async () => {
    await init()
    const path = new LiftPath(lift)
    for (const [index, tick] of [[0, 0], [3, 1234], [11, 98_765], [7, 4_000_000_000]] as const) {
      const [x, y, z, yaw, along, stretch, px, py, pz, chairs] = ClientSim.liftChair(new Int32Array(points), new Int32Array(cable), 4, index, tick, 3)
      const chair = path.chair(index, tick)
      expect(path.chairs).toBe(chairs)
      expect(chair.along).toBeCloseTo(along!, 1)
      expect(chair.x).toBeCloseTo(x!, 2)
      expect(chair.y).toBeCloseTo(y!, 2)
      expect(chair.z).toBeCloseTo(z!, 2)
      expect(chair.yaw).toBeCloseTo(yaw!, 3)
      expect(chair.stretch).toBe(stretch)
      const place = path.place(chair, 3)
      expect(place[0]).toBeCloseTo(px!, 2)
      expect(place[1]).toBeCloseTo(py!, 2)
      expect(place[2]).toBeCloseTo(pz!, 2)
    }
  })

  it('run the chairs up at the lift’s speed and round the top', () => {
    const path = new LiftPath(lift)
    const a = path.chair(0, 0)
    expect(a.stretch).toBe(STRETCH_UP)
    expect(path.chair(0, TICK_RATE).along - a.along).toBeCloseTo(LIFT_SPEED, 4)
    expect(path.at(path.up + 0.5).stretch).toBe(STRETCH_TOP)
  })
})
