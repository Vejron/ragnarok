import { describe, expect, it } from 'vitest'
import { encode } from '@msgpack/msgpack'
import { seatView } from '@/seats'
import { busStopSigns } from '@/render/busstops'
import { BUS_SEATS, KIND_BUS } from '@/sim/consts'
import { WORLD_VERSION, busLineAt, decodeManifest, type BusLine } from '@/world'

/** East for 300 m, then south for 400 m, with a stop at each end and at the corner. */
const LINE: BusLine = { points: [[0, 0], [300, 0], [300, 400]], stops: [0, 300, 700], signs: [[0, 5.6], [300, 5.6], [294.4, 400]] }

describe('bus lines', () => {
  it('come with the manifest, in metres', () => {
    const cm = (line: BusLine) => [line.points.map(([x, z]) => [x * 100, z * 100]), line.stops.map((stop) => stop * 100), line.signs.map(([x, z]) => [Math.round(x * 100), Math.round(z * 100)])]
    const world = decodeManifest(encode([WORLD_VERSION, [0, 0, 0], 50_000, 25_600, [], [], 0, 0, [], false, [cm(LINE)], [87, 82, 42], true]))
    expect(world.busLines).toEqual([LINE])
    expect(world.riverTint).toBe(0x57522a)
    expect(world.tintAll).toBe(true)
  })

  it('are followed by distance along them, as the server follows them', () => {
    expect(busLineAt(LINE, 100)).toEqual({ at: [100, 0], way: [1, 0] })
    expect(busLineAt(LINE, 500)).toEqual({ at: [300, 200], way: [0, 1] })
    // Past either end is the end.
    expect(busLineAt(LINE, 900)).toEqual({ at: [300, 400], way: [0, 1] })
    expect(busLineAt(LINE, -5).at).toEqual([0, 0])
  })

  it('have one sign at every stop, where the world put it, facing along the road', () => {
    const signs = busStopSigns([LINE])
    expect(signs.map(({ x, z }) => [x, z])).toEqual(LINE.signs)
    // The first stop is on a road running east, the last on the stretch running south.
    expect(signs[0]!.yaw).toBeCloseTo(Math.PI / 2)
    expect(signs[2]!.yaw).toBeCloseTo(0)
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
