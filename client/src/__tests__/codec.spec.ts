import { describe, expect, it } from 'vitest'
import { decodeServer, encodeInput, encodePing, toMrad } from '@/net/codec'
import { BUTTON_FIRE, BUTTON_JUMP, BUTTON_RUN } from '@/sim/consts'

// Byte-for-byte copies of the fixtures in `crates/shared/src/protocol.rs`. If these break, the
// TypeScript and Rust encoders have drifted apart.
const INPUT_FIXTURE = [147, 0, 5, 146, 150, 41, 127, 0, 205, 6, 35, 0, 205, 1, 44, 150, 42, 208, 129, 127, 209, 248, 98, 11, 209, 255, 106]
const SNAPSHOT_FIXTURE = [
  150, 1, 205, 4, 210, 42, 9, 146, 154, 1, 0, 205, 4, 210, 210, 255, 254, 29, 192, 101, 205, 6, 35, 209, 254, 12, 0, 205, 2, 138, 209, 254, 212, 154, 7, 0, 0, 85, 206, 0, 1, 134, 160, 0, 0, 1, 0, 0, 146, 155, 9, 1, 209, 255, 6, 80, 205, 16, 4, 205, 4, 176, 209, 244, 72, 15, 208, 192, 148, 1, 0, 7, 0, 192, 155, 10, 2, 205, 1, 44, 90, 209, 248, 48, 0, 205, 2, 188, 0, 32, 146, 1, 7, 148, 209, 253, 68, 204, 250, 3, 205, 4, 210,
]

describe('codec', () => {
  it('encodes inputs exactly like the server expects', () => {
    const bytes = encodeInput([
      { seq: 41, strafe: 127, forward: 0, yaw: 1571, buttons: 0, pitch: 300 },
      { seq: 42, strafe: -127, forward: 127, yaw: -1950, buttons: BUTTON_JUMP | BUTTON_RUN | BUTTON_FIRE, pitch: -150 },
    ])
    expect([...bytes]).toEqual(INPUT_FIXTURE)
  })

  it('decodes the server snapshot fixture', () => {
    const message = decodeServer(new Uint8Array(SNAPSHOT_FIXTURE))
    expect(message).toEqual({
      type: 'snapshot',
      tick: 1234,
      ack: 42,
      ride: 9,
      entities: [
        { id: 1, kind: 0, x: 12.34, y: -1234.56, z: 1.01, yaw: 1.571, vy: -5, grounded: false, knockX: 6.5, knockZ: -3 },
        { id: 7, kind: 0, x: 0, y: 0.85, z: 1000, yaw: 0, vy: 0, grounded: true, knockX: 0, knockZ: 0 },
      ],
      vehicles: [
        {
          id: 9, kind: 1, x: -2.5, y: 0.8, z: 41,
          qx: 1200 / 32767, qy: -3000 / 32767, qz: 15 / 32767, qw: expect.closeTo(Math.sqrt(1 - (1200 ** 2 + 3000 ** 2 + 15 ** 2) / 32767 ** 2), 6),
          steer: -64 / 127, seats: [1, 0, 7, 0], turret: null,
        },
        {
          id: 10, kind: 2, x: 3, y: 0.9, z: -20,
          qx: 0, qy: 700 / 32767, qz: 0, qw: expect.closeTo(Math.sqrt(1 - (700 / 32767) ** 2), 6),
          steer: 32 / 127, seats: [1, 7], turret: { yaw: -0.7, pitch: 0.25, shots: 3, range: 123.4 },
        },
      ],
    })
  })

  it('rejects malformed messages', () => {
    expect(decodeServer(new Uint8Array([0xc1]))).toBeNull()
    expect(decodeServer(new Uint8Array([0x92, 0x09, 0x01]))).toBeNull()
    expect(decodeServer(new Uint8Array([0x93, 0x01, 0x01, 0x01]))).toBeNull()
    // A snapshot without the ride field or vehicle list is from an older protocol.
    expect(decodeServer(new Uint8Array([0x94, 1, 0, 0, 0x90]))).toBeNull()
    expect(decodeServer(new Uint8Array([0x95, 1, 0, 0, 0x90, 0x90]))).toBeNull()
  })

  it('round-trips pings and wraps angles', () => {
    expect([...encodePing(5)]).toEqual([0x92, 1, 5])
    expect(toMrad(Math.PI * 2 + 1)).toBe(1000)
    expect(Math.abs(toMrad(1e6))).toBeLessThanOrEqual(3142)
  })
})
