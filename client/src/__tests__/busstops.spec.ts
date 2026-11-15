import { describe, expect, it } from 'vitest'
import { encode } from '@msgpack/msgpack'
import { seatView } from '@/seats'
import { busStopSigns } from '@/render/busstops'
import { BUS_SEATS, BUS_STOP_SETBACK, KIND_BUS } from '@/sim/consts'
import { WORLD_VERSION, busLineAt, decodeManifest, type BusLine } from '@/world'

/** East for 300 m, then south for 400 m, with a stop at each end and at the corner. */
const LINE: BusLine = { points: [[0, 0], [300, 0], [300, 400]], stops: [0, 300, 700] }

describe('bus lines', () => {
  it('come with the manifest, in metres', () => {
    const cm = (line: BusLine) => [line.points.map(([x, z]) => [x * 100, z * 100]), line.stops.map((stop) => stop * 100)]
    const world = decodeManifest(encode([WORLD_VERSION, [0, 0, 0], 50_000, 25_600, [], [], 0, 0, [], false, [cm(LINE)]]))
    expect(world.busLines).toEqual([LINE])
  })

  it('are followed by distance along them, as the server follows them', () => {
    expect(busLineAt(LINE, 100)).toEqual({ at: [100, 0], way: [1, 0] })
    expect(busLineAt(LINE, 500)).toEqual({ at: [300, 200], way: [0, 1] })
    // Past either end is the end.
    expect(busLineAt(LINE, 900)).toEqual({ at: [300, 400], way: [0, 1] })
    expect(busLineAt(LINE, -5).at).toEqual([0, 0])
  })

  it('have a sign on each side of the road at every stop', () => {
    const signs = busStopSigns([LINE])
    expect(signs).toHaveLength(6)
    // The first stop, on a road running east: one sign south of the road and one north of it.
    expect(signs[0]!.x).toBeCloseTo(0)
    expect(signs[0]!.z).toBeCloseTo(BUS_STOP_SETBACK)
    expect(signs[1]!.z).toBeCloseTo(-BUS_STOP_SETBACK)
    // The last, on the stretch running south: east and west of it.
    expect(Math.abs(signs[4]!.x - 300)).toBeCloseTo(BUS_STOP_SETBACK)
    expect(signs[4]!.z).toBeCloseTo(400)
    expect(busStopSigns([])).toEqual([])
  })
})

describe('the bus', () => {
  it('seats a driver and passengers in every other seat', () => {
    expect(seatView(KIND_BUS, 0).role).toBe('driver')
    for (let seat = 1; seat < BUS_SEATS; seat++) {
      expect(seatView(KIND_BUS, seat).role).toBe('passenger')
      expect(seatView(KIND_BUS, seat).board).toBe('E: board the bus')
    }
  })
})
