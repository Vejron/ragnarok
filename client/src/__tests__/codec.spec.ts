import { describe, expect, it } from 'vitest'
import { encode } from '@msgpack/msgpack'
import { ARMS_CUT, decodeServer, NOTICE_OBJECTIVE, KIND_ROADSTER, THRUST_BOOST, TURRET_STOWED, encodeInput, encodePing, encodeRtc, RTC_ICE, entityState, FLAG_CROWING, FLAG_DEAD, FLAG_GROUNDED, KIND_PLANE, KIND_ROOSTER, sceneStates, toMrad, type WireScene } from '@/net/codec'
import { Replica } from '@/net/replica'
import type { Update } from '@/net/replicated'
import { BUTTON_FIRE, BUTTON_JUMP, BUTTON_RUN } from '@/sim/consts'

// Byte-for-byte copies of the fixtures in `crates/shared/src/protocol.rs`. If these break, the
// TypeScript and Rust encoders have drifted apart.
const INPUT_FIXTURE = [148, 0, 39, 205, 4, 206, 146, 152, 41, 127, 0, 205, 6, 35, 0, 205, 1, 44, 205, 238, 72, 0, 152, 42, 208, 129, 127, 209, 248, 98, 11, 209, 255, 106, 7, 3]
const SNAPSHOT_FIXTURE = [155, 1, 205, 4, 210, 205, 4, 206, 42, 9, 148, 153, 1, 7, 25, 0, 0, 208, 136, 205, 2, 138, 0, 0, 156, 7, 205, 64, 15, 0, 85, 206, 0, 1, 134, 160, 0, 0, 0, 0, 3, 0, 0, 220, 0, 17, 10, 205, 76, 113, 205, 1, 44, 90, 209, 248, 48, 0, 205, 2, 188, 0, 32, 209, 253, 68, 204, 250, 3, 205, 4, 210, 1, 146, 1, 7, 204, 250, 2, 147, 12, 206, 128, 0, 0, 0, 205, 16, 0, 146, 3, 11, 152, 120, 251, 209, 246, 60, 30, 209, 254, 112, 15, 204, 250, 209, 253, 168, 147, 73, 204, 128, 147, 147, 2, 1, 17, 147, 0, 0, 0, 147, 3, 2, 0, 146, 149, 7, 0, 205, 5, 220, 204, 160, 209, 239, 152, 149, 1, 0, 236, 90, 205, 121, 24, 145, 154, 77, 7, 2, 120, 204, 165, 209, 244, 72, 205, 5, 220, 236, 209, 150, 136, 205, 4, 209]

describe('codec', () => {
  it('encodes inputs exactly like the server expects', () => {
    const bytes = encodeInput(
      [
        { seq: 41, strafe: 127, forward: 0, yaw: 1571, buttons: 0, pitch: 300, view: 61000, slot: 0 },
        { seq: 42, strafe: -127, forward: 127, yaw: -1950, buttons: BUTTON_JUMP | BUTTON_RUN | BUTTON_FIRE, pitch: -150, view: 7, slot: 3 },
      ],
      1230,
    )
    expect([...bytes]).toEqual(INPUT_FIXTURE)
  })

  it('matches the voice signaling fixture and rejects a malformed one', () => {
    // `Rtc(S_RTC, 5, RTC_ICE, "x")` in `crates/shared/src/protocol.rs`.
    const fixture = [148, 9, 5, 2, 161, 120]
    expect(decodeServer(new Uint8Array(fixture))).toEqual({ type: 'rtc', from: 5, kind: RTC_ICE, json: 'x' })
    expect(decodeServer(new Uint8Array([148, 9, 5, 2, 1]))).toBeNull()
    expect([...encodeRtc(5, RTC_ICE, 'x')]).toEqual([148, 2, 5, 2, 161, 120])
  })

  it('matches the profiles fixture and rejects a malformed one', () => {
    // `PROFILES_FIXTURE` in crates/shared/src/protocol.rs.
    const fixture = [146, 11, 145, 149, 7, 164, 195, 133, 115, 97, 3, 2, 1]
    expect(decodeServer(new Uint8Array(fixture))).toEqual({ type: 'profiles', players: [{ id: 7, name: 'Åsa', color: 3, team: 2, bot: true }] })
    expect(decodeServer(encode([11, [[7, 3, 3, 0, 0]]]))).toBeNull()
    expect(decodeServer(encode([11, [[7, 'Åsa', 3]]]))).toBeNull()
  })

  it('matches the notice fixture and rejects a malformed one', () => {
    // `NOTICE_FIXTURE` in crates/shared/src/protocol.rs.
    expect(decodeServer(new Uint8Array([147, 12, 1, 162, 71, 111]))).toEqual({ type: 'notice', kind: NOTICE_OBJECTIVE, text: 'Go' })
    expect(decodeServer(encode([12, 1]))).toBeNull()
    expect(decodeServer(encode([12, 1, 5]))).toBeNull()
  })

  it('decodes the server snapshot fixture', () => {
    const message = decodeServer(new Uint8Array(SNAPSHOT_FIXTURE))
    expect(message).toEqual({
      type: 'snapshot',
      tick: 1234,
      baseline: 1230,
      ack: 42,
      ride: 9,
      updates: [
        // A stickman moving, as differences; a dead one new to the receiver, whole; a tank, whole; a repair.
        { id: 1, cleared: 0, set: { place: { x: 25, y: 0, z: 0 }, heading: { yaw: -120 }, motion: { vy: 650, knock_x: 0, knock_z: 0 } } },
        { id: 7, cleared: 0, set: { place: { x: 0, y: 85, z: 100000 }, heading: { yaw: 0 }, motion: { vy: 0, knock_x: 0, knock_z: 0 }, flags: { flags: FLAG_GROUNDED | FLAG_DEAD, stance: 0 }, kind: { kind: 0 } } },
        {
          id: 10, cleared: 0,
          set: { place: { x: 300, y: 90, z: -2000 }, orientation: { qx: 0, qy: 700, qz: 0 }, steer: { steer: 32 }, turret: { yaw: -700, pitch: 250, shots: 3, range: 1234, flags: TURRET_STOWED }, seats: { riders: [1, 7] }, health: { hp: 250 }, kind: { kind: 2 } },
        },
        { id: 12, cleared: 1 << 12, set: {} },
      ],
      removed: [3, 11],
      drive: { vx: 1.2, vy: -0.05, vz: -25, wx: 0.03, wy: -0.4, wz: 0.015, steer: 0.25, spin: -0.6 },
      hp: 73,
      progress: 128,
      inventory: [
        { item: 2, count: 1, ammo: 17 },
        { item: 0, count: 0, ammo: 0 },
        { item: 3, count: 2, ammo: 0 },
      ],
      shots: [
        { shooter: 7, weapon: 0, x: 15, y: 1.6, z: -42 },
        { shooter: 1, weapon: 0, x: -0.2, y: 0.9, z: 310 },
      ],
      bullets: [{ id: 77, shooter: 7, item: 2, x: 1.2, y: 1.65, z: -30, vx: 15, vy: -0.2, vz: -270, tick: 1233 }],
    })
  })

  it('reads a crowing rooster from its kind and flags', () => {
    const raw = [12, KIND_ROOSTER, 100, 50, -200, 1571, 0, FLAG_GROUNDED | FLAG_CROWING, 0, 0]
    expect(entityState(raw)).toMatchObject({ id: 12, kind: 5, grounded: true, crowing: true, dead: false, held: 0 })
    expect(entityState([12, KIND_ROOSTER, 0, 0, 0, 0, 0, FLAG_GROUNDED, 0, 0]).crowing).toBe(false)
  })

  it('converts a rebuilt scene to game units', () => {
    const scene: WireScene = {
      entities: [[1, 0, 1234, -123456, 101, 1571, -500, 0, 0, 650, -300], [7, 0, 0, 85, 100000, 0, 0, 3, 0, 0, 0]],
      vehicles: [
        [9, KIND_ROADSTER, -250, 80, 4100, 1200, -3000, 15, -64, [1, 0, 7, 0], null, 100, [0, 0, 40, 0, 0, 0, 255, 3], null, [51, THRUST_BOOST], [[0, 0, 0], [1571, -100, 2]]],
        [10, 2, 300, 90, -2000, 0, 700, 0, 32, [1, 7], [-700, 250, 3, 1234, TURRET_STOWED], 0, null, null, null, []],
        [12, KIND_PLANE, 0, 140, 0, 0, 0, 0, 0, [4, 0], [3142, 500, 9, 0, 0], 70, null, [17, 255, 51, 3, ARMS_CUT], null, []],
      ],
    }
    expect(sceneStates(scene)).toEqual({
      entities: [
        { id: 1, kind: 0, x: 12.34, y: -1234.56, z: 1.01, yaw: 1.571, vy: -5, grounded: false, dead: false, crouching: false, prone: false, chute: false, knockX: 6.5, knockZ: -3, held: 0, crowing: false },
        { id: 7, kind: 0, x: 0, y: 0.85, z: 1000, yaw: 0, vy: 0, grounded: true, dead: true, crouching: false, prone: false, chute: false, knockX: 0, knockZ: 0, held: 0, crowing: false },
      ],
      vehicles: [
        {
          id: 9, kind: KIND_ROADSTER, x: -2.5, y: 0.8, z: 41,
          qx: 1200 / 32767, qy: -3000 / 32767, qz: 15 / 32767, qw: expect.closeTo(Math.sqrt(1 - (1200 ** 2 + 3000 ** 2 + 15 ** 2) / 32767 ** 2), 6),
          steer: -64 / 127, seats: [1, 0, 7, 0], turret: null, hp: 100, damage: [0, 0, 40, 0, 0, 0, 255, 3], arms: null, thrust: { air: 0.2, flags: THRUST_BOOST },
          crew: [{ yaw: 0, pitch: 0, held: 0 }, { yaw: 1.571, pitch: -0.1, held: 2 }],
        },
        {
          id: 10, kind: 2, x: 3, y: 0.9, z: -20,
          qx: 0, qy: 700 / 32767, qz: 0, qw: expect.closeTo(Math.sqrt(1 - (700 / 32767) ** 2), 6),
          steer: 32 / 127, seats: [1, 7], turret: { yaw: -0.7, pitch: 0.25, shots: 3, range: 123.4, stowed: true }, hp: 0, damage: [0, 0, 0, 0, 0, 0, 0, 0], arms: null, thrust: null, crew: [],
        },
        {
          id: 12, kind: KIND_PLANE, x: 0, y: 1.4, z: 0, qx: 0, qy: 0, qz: 0, qw: 1, steer: 0, seats: [4, 0],
          turret: { yaw: 3.142, pitch: 0.5, shots: 9, range: 0, stowed: false }, hp: 70, damage: [0, 0, 0, 0, 0, 0, 0, 0],
          arms: { shots: 17, heat: 1, rearHeat: 0.2, bombs: 3, flags: ARMS_CUT }, thrust: null, crew: [],
        },
      ],
    })
  })

  it('rejects malformed messages', () => {
    expect(decodeServer(new Uint8Array([0xc1]))).toBeNull()
    expect(decodeServer(new Uint8Array([0x92, 0x09, 0x01]))).toBeNull()
    expect(decodeServer(new Uint8Array([0x93, 0x01, 0x01, 0x01]))).toBeNull()
    const snapshot = (updates: unknown, removed: unknown = []) => encode([1, 5, 0, 0, 0, updates, removed, null, [100, 0, []], [], []])
    expect(decodeServer(snapshot([]))).not.toBeNull()
    // A v1 snapshot, with its sixteen fields.
    expect(decodeServer(encode([1, 5, 0, 0, 0, [], [], [], [], [], null, [100, 0, []], [], [], [], []]))).toBeNull()
    // An update cut short, a field that is not a number, a removed id that is not one.
    expect(decodeServer(snapshot([[9, 1, 10, 20]]))).toBeNull()
    expect(decodeServer(snapshot([[9, 2, 'x']]))).toBeNull()
    expect(decodeServer(snapshot([], ['x']))).toBeNull()
    expect(decodeServer(snapshot('x'))).toBeNull()
  })

  it('decodes an empty snapshot', () => {
    const message = decodeServer(encode([1, 5, 3, 2, 0, [], [], null, [100, 0, []], [], []]))
    expect(message).toMatchObject({ type: 'snapshot', tick: 5, baseline: 3, ack: 2, hp: 100, updates: [], removed: [] })
  })

  it('decodes kills and the score table', () => {
    expect(decodeServer(encode([4, 7, 3, 12, 5, 2, 3, -1571, 100]))).toEqual({
      type: 'kill', killer: 7, victim: 3, weapon: 12, killerKills: 5, victimDeaths: 2, style: 3, yaw: -1.571, power: 20,
    })
    expect(decodeServer(encode([5, [[1, 2, 3], [4, 0, 9]]]))).toEqual({
      type: 'scores',
      scores: [{ id: 1, kills: 2, deaths: 3 }, { id: 4, kills: 0, deaths: 9 }],
    })
    expect(decodeServer(encode([4, 7, 3]))).toBeNull()
    expect(decodeServer(encode([5, [[1, 2]]]))).toBeNull()
  })

  it('round-trips pings and wraps angles', () => {
    expect([...encodePing(5)]).toEqual([0x92, 1, 5])
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
