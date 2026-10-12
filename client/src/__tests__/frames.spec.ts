import { describe, expect, it } from 'vitest'
import { FrameError, FrameReader, MAX_FRAME } from '../net/frames'

const frame = (...bytes: number[]) => Uint8Array.from([0, 0, 0, bytes.length, ...bytes])

describe('FrameReader', () => {
  it('returns each message whole, in order', () => {
    const reader = new FrameReader()
    const both = Uint8Array.from([...frame(1, 2, 3), ...frame(9)])
    expect(reader.push(both).map((m) => [...m])).toEqual([[1, 2, 3], [9]])
  })

  it('waits for the rest of a message split anywhere, even inside the length', () => {
    const reader = new FrameReader()
    const all = Uint8Array.from([...frame(1, 2, 3, 4, 5), ...frame(6, 7)])
    const out: number[][] = []
    for (const byte of all) for (const message of reader.push(Uint8Array.of(byte))) out.push([...message])
    expect(out).toEqual([[1, 2, 3, 4, 5], [6, 7]])
  })

  it('delivers an empty message', () => {
    expect(new FrameReader().push(Uint8Array.of(0, 0, 0, 0)).map((m) => m.length)).toEqual([0])
  })

  it('refuses a length no honest server sends', () => {
    const huge = new DataView(new ArrayBuffer(4))
    huge.setUint32(0, MAX_FRAME + 1)
    expect(() => new FrameReader().push(new Uint8Array(huge.buffer))).toThrow(FrameError)
  })
})
