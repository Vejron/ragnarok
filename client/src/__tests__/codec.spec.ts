import { describe, expect, it } from 'vitest'
import { encode } from '@msgpack/msgpack'
import { applyDelta, ARMS_CUT, ARMS_JAMMED, decodeServer, NOTICE_OBJECTIVE, KIND_ROADSTER, THRUST_BOOST, THRUST_LIFT, TURRET_STOWED, encodeInput, encodePing, encodeRtc, RTC_ICE, entityState, FLAG_CROWING, FLAG_GROUNDED, KIND_PLANE, KIND_ROOSTER, sceneStates, toMrad, type RawEntity, type RawVehicle, type RawVehicleDelta, type WireScene } from '@/net/codec'
import { Replica } from '@/net/replica'
import { BUTTON_FIRE, BUTTON_JUMP, BUTTON_RUN } from '@/sim/consts'

// Byte-for-byte copies of the fixtures in `crates/shared/src/protocol.rs`. If these break, the
// TypeScript and Rust encoders have drifted apart.
const INPUT_FIXTURE = [148, 0, 35, 205, 4, 206, 146, 152, 41, 127, 0, 205, 6, 35, 0, 205, 1, 44, 205, 238, 72, 0, 152, 42, 208, 129, 127, 209, 248, 98, 11, 209, 255, 106, 7, 3]
const SNAPSHOT_FIXTURE = [220, 0, 16, 1, 205, 4, 210, 205, 4, 206, 42, 9, 145, 155, 7, 0, 0, 85, 206, 0, 1, 134, 160, 0, 0, 3, 0, 0, 0, 145, 154, 1, 25, 253, 40, 208, 136, 205, 2, 138, 1, 0, 209, 254, 212, 12, 146, 159, 10, 2, 205, 1, 44, 90, 209, 248, 48, 0, 205, 2, 188, 0, 32, 146, 1, 7, 149, 209, 253, 68, 204, 250, 3, 205, 4, 210, 1, 0, 192, 192, 192, 159, 12, 6, 209, 254, 12, 205, 1, 44, 205, 3, 32, 10, 236, 30, 0, 146, 4, 0, 149, 205, 11, 184, 204, 200, 9, 0, 0, 70, 192, 149, 17, 204, 200, 0, 3, 3, 192, 145, 158, 9, 100, 0, 208, 211, 3, 254, 0, 4, 148, 1, 0, 7, 0, 192, 100, 152, 0, 0, 40, 0, 0, 0, 204, 255, 3, 192, 146, 204, 200, 3, 146, 3, 11, 152, 120, 251, 209, 246, 60, 30, 209, 254, 112, 15, 204, 250, 209, 253, 168, 147, 73, 204, 128, 147, 147, 2, 1, 17, 147, 0, 0, 0, 147, 3, 2, 0, 146, 149, 7, 0, 205, 5, 220, 204, 160, 209, 239, 152, 149, 1, 0, 236, 90, 205, 121, 24, 145, 152, 40, 1, 205, 4, 176, 204, 150, 209, 252, 224, 205, 11, 184, 0, 209, 246, 60, 145, 149, 60, 4, 209, 254, 212, 5, 205, 4, 226, 145, 154, 77, 7, 2, 120, 204, 165, 209, 244, 72, 205, 5, 220, 236, 209, 150, 136, 205, 4, 209]

const person = (id: number, x: number): RawEntity => [id, 0, x, 90, -x, 100, 0, 1, 0, 0, 0]
const car = (id: number, x: number, seats: number[]): RawVehicle => [id, 1, x, 40, 7, 10, -20, 30, 5, seats, null, 100, null, null, null]
const tank = (id: number, yaw: number): RawVehicle => [id, 2, 0, 40, 0, 0, 0, 0, 0, [0, 0], [yaw, 100, 1, 500, 0], 250, null, null, null]
const plane = (id: number, arms: number[]): RawVehicle => [id, KIND_PLANE, 0, 140, 0, 0, 0, 0, 0, [0, 0], [0, 0, 0, 0, 0], 70, null, arms, null]

describe('codec', () => {
  const empty = { newEntities: [], entities: [], newVehicles: [], vehicles: [], removed: [] }
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
      delta: {
        newEntities: [[7, 0, 0, 85, 100000, 0, 0, 3, 0, 0, 0]],
        entities: [[1, 25, -3, 40, -120, 650, 1, 0, -300, 12]],
        newVehicles: [
          [10, 2, 300, 90, -2000, 0, 700, 0, 32, [1, 7], [-700, 250, 3, 1234, TURRET_STOWED], 0, null, null, null],
          [12, KIND_PLANE, -500, 300, 800, 10, -20, 30, 0, [4, 0], [3000, 200, 9, 0, 0], 70, null, [17, 200, 0, 3, ARMS_CUT | ARMS_JAMMED], null],
        ],
        vehicles: [[9, 100, 0, -45, 3, -2, 0, 4, [1, 0, 7, 0], null, 100, [0, 0, 40, 0, 0, 0, 255, 3], null, [200, THRUST_BOOST | THRUST_LIFT]]],
        removed: [3, 11],
      },
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
      rockets: [{ id: 40, kind: 1, x: 12, y: 1.5, z: -8, vx: 30, vy: 0, vz: -25 }],
      pickups: [{ id: 60, item: 4, x: -3, y: 0.05, z: 12.5 }],
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
        [9, KIND_ROADSTER, -250, 80, 4100, 1200, -3000, 15, -64, [1, 0, 7, 0], null, 100, [0, 0, 40, 0, 0, 0, 255, 3], null, [51, THRUST_BOOST]],
        [10, 2, 300, 90, -2000, 0, 700, 0, 32, [1, 7], [-700, 250, 3, 1234, TURRET_STOWED], 0, null, null, null],
        [12, KIND_PLANE, 0, 140, 0, 0, 0, 0, 0, [4, 0], [3142, 500, 9, 0, 0], 70, null, [17, 255, 51, 3, ARMS_CUT], null],
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
        },
        {
          id: 10, kind: 2, x: 3, y: 0.9, z: -20,
          qx: 0, qy: 700 / 32767, qz: 0, qw: expect.closeTo(Math.sqrt(1 - (700 / 32767) ** 2), 6),
          steer: 32 / 127, seats: [1, 7], turret: { yaw: -0.7, pitch: 0.25, shots: 3, range: 123.4, stowed: true }, hp: 0, damage: [0, 0, 0, 0, 0, 0, 0, 0], arms: null, thrust: null,
        },
        {
          id: 12, kind: KIND_PLANE, x: 0, y: 1.4, z: 0, qx: 0, qy: 0, qz: 0, qw: 1, steer: 0, seats: [4, 0],
          turret: { yaw: 3.142, pitch: 0.5, shots: 9, range: 0, stowed: false }, hp: 70, damage: [0, 0, 0, 0, 0, 0, 0, 0],
          arms: { shots: 17, heat: 1, rearHeat: 0.2, bombs: 3, flags: ARMS_CUT }, thrust: null,
        },
      ],
    })
  })

  it('rejects malformed messages', () => {
    expect(decodeServer(new Uint8Array([0xc1]))).toBeNull()
    expect(decodeServer(new Uint8Array([0x92, 0x09, 0x01]))).toBeNull()
    expect(decodeServer(new Uint8Array([0x93, 0x01, 0x01, 0x01]))).toBeNull()
    // A snapshot of an older protocol has fewer fields.
    expect(decodeServer(new Uint8Array([0x94, 1, 0, 0, 0x90]))).toBeNull()
    expect(decodeServer(encode([1, 5, 0, 0, 0, [], [], [], [], [], null, [100, 0, []], [], []]))).toBeNull()
    // Wrong shapes inside the lists.
    expect(decodeServer(encode([1, 5, 0, 0, 0, [[1, 2, 3]], [], [], [], [], null, [100, 0, []], [], [], [], []]))).toBeNull()
    expect(decodeServer(encode([1, 5, 0, 0, 0, [], [[1, 2, 3]], [], [], [], null, [100, 0, []], [], [], [], []]))).toBeNull()
    expect(decodeServer(encode([1, 5, 0, 0, 0, [], [], [], [], ['x'], null, [100, 0, []], [], [], [], []]))).toBeNull()
    expect(decodeServer(encode([1, 5, 0, 0, 0, [], [], [], [[1, 0, 0, 0, 0, 0, 0, 0, [1, 2, 3, 4, 5], null, 100, null, null]], [], null, [100, 0, []], [], [], [], []]))).toBeNull()
    // A vehicle of an old protocol, without arms or thrust.
    expect(decodeServer(encode([1, 5, 0, 0, 0, [], [], [[9, 1, 0, 0, 0, 0, 0, 0, 0, [0, 0, 0, 0], null, 100, null]], [], [], null, [100, 0, []], [], [], [], []]))).toBeNull()
  })

  it('rejects a vehicle with damage of the wrong shape', () => {
    const vehicle = (damage: unknown) => [1, 5, 0, 0, 0, [], [], [[9, 1, 0, 0, 0, 0, 0, 0, 0, [0, 0, 0, 0], null, 100, damage, null, null]], [], [], null, [100, 0, []], [], [], [], []]
    expect(decodeServer(encode(vehicle(null)))).not.toBeNull()
    expect(decodeServer(encode(vehicle([1, 2, 3, 4, 5, 6, 7, 8])))).not.toBeNull()
    expect(decodeServer(encode(vehicle([1, 2, 3])))).toBeNull()
    expect(decodeServer(encode(vehicle('x')))).toBeNull()
  })

  it('rejects arms of the wrong shape', () => {
    const vehicle = (arms: unknown) => [1, 5, 0, 0, 0, [], [], [[12, KIND_PLANE, 0, 0, 0, 0, 0, 0, 0, [0, 0], [0, 0, 0, 0, 0], 70, null, arms, null]], [], [], null, [100, 0, []], [], [], [], []]
    expect(decodeServer(encode(vehicle([1, 2, 3, 4, 5])))).not.toBeNull()
    expect(decodeServer(encode(vehicle([1, 2, 3])))).toBeNull()
    expect(decodeServer(encode(vehicle(7)))).toBeNull()
  })

  it('rejects thrust of the wrong shape and keeps it until a delta changes it', () => {
    const vehicle = (thrust: unknown) => [1, 5, 0, 0, 0, [], [], [[9, KIND_ROADSTER, 0, 0, 0, 0, 0, 0, 0, [0, 0], null, 100, null, null, thrust]], [], [], null, [100, 0, []], [], [], [], []]
    expect(decodeServer(encode(vehicle([255, 0])))).not.toBeNull()
    expect(decodeServer(encode(vehicle([255])))).toBeNull()
    expect(decodeServer(encode(vehicle('x')))).toBeNull()
    const base: WireScene = { entities: [], vehicles: [[9, KIND_ROADSTER, 0, 0, 0, 0, 0, 0, 0, [0, 0], null, 100, null, null, [255, 0]]] }
    const delta = (thrust: number[] | null) => ({ ...empty, vehicles: [[9, 0, 0, 0, 0, 0, 0, 0, null, null, 100, null, null, thrust] as RawVehicleDelta] })
    const boosting = applyDelta(base, delta([200, THRUST_BOOST]))
    expect(boosting.vehicles[0]![14]).toEqual([200, THRUST_BOOST])
    expect(applyDelta(boosting, delta(null)).vehicles[0]![14]).toEqual([200, THRUST_BOOST])
  })

  it('keeps a plane’s arms until a delta changes them', () => {
    const base: WireScene = { entities: [], vehicles: [plane(12, [0, 0, 0, 4, 0])] }
    const delta = (arms: number[] | null) => ({ ...empty, vehicles: [[12, 0, 0, 0, 0, 0, 0, 0, null, null, 70, null, arms, null] as RawVehicleDelta] })
    const fired = applyDelta(base, delta([3, 31, 0, 4, ARMS_CUT]))
    expect(fired.vehicles[0]![13]).toEqual([3, 31, 0, 4, ARMS_CUT])
    expect(applyDelta(fired, delta(null)).vehicles[0]![13]).toEqual([3, 31, 0, 4, ARMS_CUT])
  })

  it('applies damage from a delta and clears it on a repair', () => {
    const base: WireScene = { entities: [], vehicles: [car(9, 0, [0, 0, 0, 0])] }
    const delta = (damage: number[] | null) => ({ ...empty, vehicles: [[9, 0, 0, 0, 0, 0, 0, 0, null, null, 100, damage, null, null] as RawVehicleDelta] })
    const hurt = applyDelta(base, delta([90, 0, 0, 20, 0, 0, 0, 255]))
    expect(hurt.vehicles[0]![12]).toEqual([90, 0, 0, 20, 0, 0, 0, 255])
    expect(applyDelta(hurt, delta(null)).vehicles[0]![12]).toEqual([90, 0, 0, 20, 0, 0, 0, 255])
    expect(applyDelta(hurt, delta([0, 0, 0, 0, 0, 0, 0, 0])).vehicles[0]![12]).toBeNull()
  })

  it('decodes an empty snapshot', () => {
    const message = decodeServer(encode([1, 5, 3, 2, 0, [], [], [], [], [], null, [100, 0, []], [], [], [], []]))
    expect(message).toMatchObject({ type: 'snapshot', tick: 5, baseline: 3, ack: 2, hp: 100 })
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

describe('delta snapshots', () => {
  const empty = { newEntities: [], entities: [], newVehicles: [], vehicles: [], removed: [] }

  // The same scene and changes as `apply_reproduces_the_new_scene` in `crates/shared/src/delta.rs`.
  it('applies a delta like the server does', () => {
    const base: WireScene = { entities: [person(1, 100), person(2, 200), person(5, 300)], vehicles: [car(9, 0, [0, 0, 0, 0]), tank(10, 0)] }
    const moved: RawEntity = [2, 0, 230, 90, -230, -3000 + 100, 650, 1, 0, -400, 0]
    const scene = applyDelta(base, {
      newEntities: [person(7, 50)],
      entities: [[2, 30, 0, -30, -3000, 650, 1, 0, -400, 0]],
      newVehicles: [],
      vehicles: [[9, 120, 0, 0, 0, 0, 0, 0, [1, 0, 0, 0], null, 100, null, null, null], [10, 0, 0, 0, 0, 0, 0, 0, null, [-700, 100, 1, 500, 0], 250, null, null, null]],
      removed: [5],
    })
    expect(scene.entities).toEqual([person(1, 100), moved, person(7, 50)])
    expect(scene.vehicles).toEqual([car(9, 120, [1, 0, 0, 0]), tank(10, -700)])
  })

  it('keeps what was not mentioned, sorted by id', () => {
    const base: WireScene = { entities: [person(1, 100), person(9, 5)], vehicles: [] }
    const scene = applyDelta(base, { ...empty, newEntities: [person(4, 1)] })
    expect(scene.entities.map((e) => e[0])).toEqual([1, 4, 9])
  })
})

describe('Replica', () => {
  const empty = { newEntities: [], entities: [], newVehicles: [], vehicles: [], removed: [] }

  it('rebuilds from a whole snapshot, then from deltas against what it acknowledged', () => {
    const replica = new Replica()
    expect(replica.baseline).toBe(0)
    const first = replica.receive(2, 0, { ...empty, newEntities: [person(1, 100), person(2, 200)] })!
    expect(first.latest).toBe(true)
    expect(replica.baseline).toBe(2)
    const second = replica.receive(4, 2, { ...empty, entities: [[1, 10, 0, -10, 0, 0, 1, 0, 0, 0]] })!
    expect(second.scene.entities[0]![2]).toBe(110)
    expect(second.scene.entities[1]).toEqual(person(2, 200))
    expect(replica.baseline).toBe(4)
  })

  it('refuses a delta against a baseline it does not hold', () => {
    const replica = new Replica()
    expect(replica.receive(10, 8, empty)).toBeNull()
    expect(replica.baseline).toBe(0)
  })

  it('does not move the baseline back for an older datagram that arrives late', () => {
    const replica = new Replica()
    replica.receive(2, 0, { ...empty, newEntities: [person(1, 1)] })
    replica.receive(6, 2, empty)
    const late = replica.receive(4, 2, empty)!
    expect(late.latest).toBe(false)
    expect(replica.baseline).toBe(6)
  })

  it('forgets everything on reset', () => {
    const replica = new Replica()
    replica.receive(2, 0, { ...empty, newEntities: [person(1, 1)] })
    replica.reset()
    expect(replica.baseline).toBe(0)
    expect(replica.receive(4, 2, empty)).toBeNull()
  })
})
