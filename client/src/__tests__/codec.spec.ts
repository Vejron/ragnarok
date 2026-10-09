import { describe, expect, it } from 'vitest'
import { decodeServer, encodeInput, encodePing, toMrad } from '@/net/codec'
import { BUTTON_JUMP, BUTTON_RUN } from '@/sim/consts'

// Byte-for-byte copies of the fixtures in `crates/shared/src/protocol.rs`. If these break, the
// TypeScript and Rust encoders have drifted apart.
const INPUT_FIXTURE = [147, 0, 1, 146, 149, 41, 127, 0, 205, 6, 35, 0, 149, 42, 208, 129, 127, 209, 248, 98, 3]
const SNAPSHOT_FIXTURE = [
  148, 1, 205, 4, 210, 42, 146, 152, 1, 0, 205, 4, 210, 210, 255, 254, 29, 192, 101, 205, 6, 35, 209, 254, 12, 0,
  152, 7, 0, 0, 85, 206, 0, 1, 134, 160, 0, 0, 1,
]

describe('codec', () => {
  it('encodes inputs exactly like the server expects', () => {
    const bytes = encodeInput([
      { seq: 41, strafe: 127, forward: 0, yaw: 1571, buttons: 0 },
      { seq: 42, strafe: -127, forward: 127, yaw: -1950, buttons: BUTTON_JUMP | BUTTON_RUN },
    ])
    expect([...bytes]).toEqual(INPUT_FIXTURE)
  })

  it('decodes the server snapshot fixture', () => {
    const message = decodeServer(new Uint8Array(SNAPSHOT_FIXTURE))
    expect(message).toEqual({
      type: 'snapshot',
      tick: 1234,
      ack: 42,
      entities: [
        { id: 1, kind: 0, x: 12.34, y: -1234.56, z: 1.01, yaw: 1.571, vy: -5, grounded: false },
        { id: 7, kind: 0, x: 0, y: 0.85, z: 1000, yaw: 0, vy: 0, grounded: true },
      ],
    })
  })

  it('rejects malformed messages', () => {
    expect(decodeServer(new Uint8Array([0xc1]))).toBeNull()
    expect(decodeServer(new Uint8Array([0x92, 0x09, 0x01]))).toBeNull()
    expect(decodeServer(new Uint8Array([0x93, 0x01, 0x01, 0x01]))).toBeNull()
  })

  it('round-trips pings and wraps angles', () => {
    expect([...encodePing(5)]).toEqual([0x92, 1, 5])
    expect(toMrad(Math.PI * 2 + 1)).toBe(1000)
    expect(Math.abs(toMrad(1e6))).toBeLessThanOrEqual(3142)
  })
})
