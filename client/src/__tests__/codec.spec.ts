import { describe, expect, it } from 'vitest'
import { encode } from '@msgpack/msgpack'
import { decodeServer, NOTICE_OBJECTIVE, encodeInput, encodePing, encodeRtc, RTC_ICE, FLAG_DEAD, FLAG_GROUNDED, toMrad, TURRET_STOWED, DEATH_ROADKILL, WEAPON_ROADKILL, WEAPON_SNIPER } from '@/net/codec'
import { Replica } from '@/net/replica'
import type { Update } from '@/net/replicated'
import { BUTTON_FIRE, BUTTON_JUMP, BUTTON_RUN, PROTOCOL_VERSION } from '@/sim/consts'
import fixture from './messages.fixture.json'

// `messages.fixture.json` holds every message as the Rust tests encode it (`crates/shared/src/protocol/tests.rs`).
// If these break, the generated codec and the Rust have drifted apart.
const bytes = (message: keyof typeof fixture) => new Uint8Array(fixture[message])

describe('codec', () => {
  it('encodes inputs exactly like the server expects', () => {
    const encoded = encodeInput(
      [
        { seq: 41, strafe: 127, forward: 0, yaw: 1571, buttons: 0, pitch: 300, view: 61000, slot: 0 },
        { seq: 42, strafe: -127, forward: 127, yaw: -1950, buttons: BUTTON_JUMP | BUTTON_RUN | BUTTON_FIRE, pitch: -150, view: 7, slot: 3 },
      ],
      1230,
    )
    expect([...encoded]).toEqual(fixture.input)
  })

  it('matches the voice signaling fixture and rejects a malformed one', () => {
    expect(decodeServer(bytes('rtc'))).toEqual({ type: 'rtc', peer: 5, kind: RTC_ICE, json: 'x' })
    expect(decodeServer(new Uint8Array([148, 9, 5, 2, 1]))).toBeNull()
    expect([...encodeRtc(5, RTC_ICE, 'x')]).toEqual(fixture.rtcOut)
  })

  it('matches the profiles fixture and rejects a malformed one', () => {
    expect(decodeServer(bytes('profiles'))).toEqual({
      type: 'profiles', players: [{ id: 7, name: 'Åsa', color: 3, team: 2, bot: true }, { id: 8, name: '', color: 0, team: 0, bot: false }],
    })
    expect(decodeServer(encode([11, [[7, 3, 3, 0, false]]]))).toBeNull()
    expect(decodeServer(encode([11, [[7, 'Åsa', 3, 0, 1]]])), 'a bot is true or false').toBeNull()
    expect(decodeServer(encode([11, [[7, 'Åsa', 3]]]))).toBeNull()
  })

  it('matches the notice fixture and rejects a malformed one', () => {
    expect(decodeServer(bytes('notice'))).toEqual({ type: 'notice', kind: NOTICE_OBJECTIVE, text: 'Go' })
    expect(decodeServer(bytes('backpack'))).toEqual({ type: 'backpack', x: -123.45, z: 678.9, radius: 75, carrier: 9, held: 301 })
    expect(decodeServer(encode([12, 1]))).toBeNull()
    expect(decodeServer(encode([12, 1, 5]))).toBeNull()
  })

  it('decodes the server snapshot fixture', () => {
    const message = decodeServer(bytes('snapshot'))
    expect(message).toEqual({
      type: 'snapshot',
      tick: 1234,
      baseline: 1230,
      ack: 42,
      ride: 9,
      updates: [
        // The receiver moving, as differences, with what only they are told; a dead soldier new to the
        // receiver, whole; the tank the receiver drives, whole, with its motion; a repair; a rider.
        {
          id: 1, cleared: 0,
          set: {
            place: { x: 25, y: 0, z: 0 }, heading: { yaw: -120 }, motion: { vy: 650, knock_x: 0, knock_z: 0 }, vitals: { hp: 73, progress: 128, spent: 0, winded: 0, parts: [50, 40, 40, 0, 40] },
            hotbar: { slots: [{ item: 2, count: 1, ammo: 17 }, { item: 0, count: 0, ammo: 0 }, { item: 3, count: 2, ammo: 0 }, { item: 0, count: 0, ammo: 0 }, { item: 0, count: 0, ammo: 0 }] },
          },
        },
        { id: 7, cleared: 0, set: { place: { x: 0, y: 85, z: 100000 }, heading: { yaw: 0 }, motion: { vy: 0, knock_x: 0, knock_z: 0 }, flags: { flags: FLAG_GROUNDED | FLAG_DEAD, stance: 0 }, kind: { kind: 0 } } },
        {
          id: 10, cleared: 0,
          set: { place: { x: 300, y: 90, z: -2000 }, orientation: { qx: 0, qy: 700, qz: 0 }, steer: { steer: 32 }, turret: { yaw: -700, pitch: 250, shots: 3, range: 1234, flags: TURRET_STOWED }, health: { hp: 250 }, kind: { kind: 2 },
            drive: { vx: 120, vy: -5, vz: -2500, wx: 30, wy: -400, wz: 15, steer: 250, spin: -600 },
          },
        },
        { id: 12, cleared: 1 << 12, set: {} },
        // Someone sitting in the tank: no longer anywhere, but in their seat, looking about.
        { id: 13, cleared: 1 << 0 | 1 << 2, set: { heading: { yaw: 900 }, flags: { flags: 4, stance: 0 }, seated: { vehicle: 12, seat: 1 }, look: { pitch: -60 } } },
      ],
      removed: [3, 11],
      shots: [
        { shooter: 7, weapon: 0, x: 15, y: 1.6, z: -42 },
        { shooter: 1, weapon: 0, x: -0.2, y: 0.9, z: 310 },
      ],
      bullets: [{ id: 77, shooter: 7, item: 2, x: 1.2, y: 1.65, z: -30, vx: 15, vy: -0.2, vz: -270, tick: 1233 }],
    })
  })

  it('rejects malformed messages', () => {
    expect(decodeServer(new Uint8Array([0xc1]))).toBeNull()
    expect(decodeServer(new Uint8Array([0x92, 0x09, 0x01]))).toBeNull()
    expect(decodeServer(new Uint8Array([0x93, 0x01, 0x01, 0x01]))).toBeNull()
    const snapshot = (updates: unknown, removed: unknown = []) => encode([1, 5, 0, 0, 0, updates, removed, [], []])
    expect(decodeServer(snapshot([]))).not.toBeNull()
    // A v1 snapshot, with its sixteen fields, and one from before the owner's components (protocol 39).
    expect(decodeServer(encode([1, 5, 0, 0, 0, [], [], [], [], [], null, [100, 0, []], [], [], [], []]))).toBeNull()
    expect(decodeServer(encode([1, 5, 0, 0, 0, [], [], null, [100, 0, []], [], []]))).toBeNull()
    // An update cut short, a field that is not a number, a removed id that is not one.
    expect(decodeServer(snapshot([[9, 1, 10, 20]]))).toBeNull()
    expect(decodeServer(snapshot([[9, 2, 'x']]))).toBeNull()
    expect(decodeServer(snapshot([], ['x']))).toBeNull()
    expect(decodeServer(snapshot('x'))).toBeNull()
  })

  it('decodes an empty snapshot', () => {
    const message = decodeServer(encode([1, 5, 3, 2, 0, [], [], [], []]))
    expect(message).toMatchObject({ type: 'snapshot', tick: 5, baseline: 3, ack: 2, updates: [], removed: [] })
  })

  it('decodes the welcome, pongs and leaving', () => {
    expect(decodeServer(bytes('welcome'))).toEqual({ type: 'welcome', version: PROTOCOL_VERSION, id: 70000, tickRate: 60, snapshotEvery: 2, worldHash: 0xdeadbeef })
    expect(decodeServer(bytes('pong'))).toEqual({ type: 'pong', nonce: 5 })
    expect(decodeServer(bytes('leave'))).toEqual({ type: 'leave', id: 9 })
    expect(decodeServer(encode([0, 1, 2, 3, 4]))).toBeNull()
  })

  it('decodes hits, with a bearing and without', () => {
    expect(decodeServer(bytes('hit'))).toEqual({ type: 'hit', role: 1, kind: 0, amount: 25, weapon: 0, bearing: -1.571, x: 1, y: 0.9, z: -2.5, target: 7 })
    expect(decodeServer(bytes('hitNowhere'))).toEqual({ type: 'hit', role: 0, kind: 2, amount: 100, weapon: WEAPON_SNIPER, bearing: null, x: 0, y: 0, z: 0, target: 0 })
    expect(decodeServer(encode([8, 0, 0, 1, 0, 'x', 0, 0, 0, 0]))).toBeNull()
  })

  it('decodes kills and the score table', () => {
    expect(decodeServer(bytes('kill'))).toEqual({
      type: 'kill', killer: 7, victim: 3, weapon: WEAPON_ROADKILL, killerKills: 5, victimDeaths: 2, style: DEATH_ROADKILL, yaw: -1.571, power: 20,
    })
    expect(decodeServer(bytes('scores'))).toEqual({
      type: 'scores',
      scores: [{ id: 1, kills: 2, deaths: 3, held: 0 }, { id: 4, kills: 0, deaths: 900, held: 754 }],
    })
    expect(decodeServer(encode([4, 7, 3]))).toBeNull()
    expect(decodeServer(encode([5, [[1, 2]]]))).toBeNull()
  })

  it('round-trips pings and wraps angles', () => {
    expect([...encodePing(5)]).toEqual(fixture.ping)
    expect(toMrad(Math.PI * 2 + 1)).toBe(1000)
    expect(Math.abs(toMrad(1e6))).toBeLessThanOrEqual(3142)
  })
})

describe('Replica', () => {
  const person = (id: number, x: number): Update => ({ id, cleared: 0, set: { kind: { kind: 0 }, place: { x, y: 90, z: -x } } })
  const moved = (id: number, dx: number): Update => ({ id, cleared: 0, set: { place: { x: dx, y: 0, z: 0 } } })

  it('rebuilds from a whole snapshot, then from deltas against what it acknowledged', () => {
    const replica = new Replica()
    expect(replica.baseline).toBe(0)
    const first = replica.receive(2, 0, [person(1, 100), person(2, 200)], [])!
    expect(first.latest).toBe(true)
    expect(replica.baseline).toBe(2)
    const second = replica.receive(4, 2, [moved(1, 10)], [])!
    expect(second.scene.get(1)?.place).toEqual({ x: 110, y: 90, z: -100 })
    expect(second.scene.get(2)).toEqual(first.scene.get(2))
    expect(first.scene.get(1)?.place?.x, 'the baseline is kept as it was').toBe(100)
    expect(replica.baseline).toBe(4)
    const third = replica.receive(6, 4, [], [2])!
    expect([...third.scene.keys()]).toEqual([1])
  })

  it('refuses a delta against a baseline it does not hold', () => {
    const replica = new Replica()
    expect(replica.receive(10, 8, [], [])).toBeNull()
    expect(replica.baseline).toBe(0)
  })

  it('does not move the baseline back for an older datagram that arrives late', () => {
    const replica = new Replica()
    replica.receive(2, 0, [person(1, 1)], [])
    replica.receive(6, 2, [], [])
    const late = replica.receive(4, 2, [], [])!
    expect(late.latest).toBe(false)
    expect(replica.baseline).toBe(6)
  })

  it('forgets everything on reset', () => {
    const replica = new Replica()
    replica.receive(2, 0, [person(1, 1)], [])
    replica.reset()
    expect(replica.baseline).toBe(0)
    expect(replica.receive(4, 2, [], [])).toBeNull()
  })
})

describe('a reliable message bigger than a datagram', () => {
  it('still decodes: a joiner is told every cell of a collapsed tower', () => {
    // A big building's join catch-up: thousands of cells, well past the 1200-byte datagram size.
    const cells = Array.from({ length: 4000 }, (_, i) => 100_000 + i * 3)
    const decoded = decodeServer(encode([10, false, [[1, 8, 5, [0, 0, 0], cells, []]]]))
    expect(decoded).toEqual({ type: 'carved', live: false, buildings: [{ col: 1, row: 8, index: 5, point: [0, 0, 0], blasted: cells, fell: [] }] })
  })
})
