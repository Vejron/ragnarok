import { decode } from '@msgpack/msgpack'
import { describe, expect, it } from 'vitest'
import type { Components } from '../net/components.generated'
import { applyUpdates, decodeUpdate, decodeUpdates } from '../net/replicated'
import fixture from './replicated.fixture.json'

// `replicated.fixture.json` is written by the Rust tests (`UPDATE_PARITY=1 cargo test -p shared`) from
// the server's own encoder: each case is a baseline, the bytes of the updates against it, the ids
// removed, and the scene they must make.
type Listed = [number, Components][]
const scene = (listed: unknown) => new Map(listed as Listed)

describe('replicated components', () => {
  for (const { name, base, bytes, removed, expected } of fixture.cases) {
    it(`rebuilds what the server sent: ${name}`, () => {
      const updates = decodeUpdates(decode(new Uint8Array(bytes)))
      expect(updates).not.toBeNull()
      expect(applyUpdates(scene(base), updates!, removed)).toEqual(scene(expected))
    })
  }

  it('leaves the baseline as it was, for the next snapshot built on it', () => {
    const { base, bytes, removed } = fixture.cases[1]!
    const held = scene(base)
    const before = structuredClone([...held])
    applyUpdates(held, decodeUpdates(decode(new Uint8Array(bytes)))!, removed)
    expect([...held]).toEqual(before)
  })

  it('refuses a malformed update rather than guess', () => {
    expect(decodeUpdate([5])).toBeNull()
    expect(decodeUpdate(['5', 0])).toBeNull()
    // Place is set but only two of its three fields came.
    expect(decodeUpdate([5, 1, 10, 20])).toBeNull()
    // One field too many.
    expect(decodeUpdate([5, 2, 30, 99])).toBeNull()
    // A crew rider that is not three numbers.
    expect(decodeUpdate([5, 1 << 7, [[1, 2]]])).toBeNull()
    // A field that is not an integer, or a list of them.
    expect(decodeUpdate([5, 2, 'north'])).toBeNull()
    expect(decodeUpdate([5, 1 << 10, [1, 'x']])).toBeNull()
    expect(decodeUpdate([5, 2, 30])).toEqual({ id: 5, cleared: 0, set: { heading: { yaw: 30 } } })
    expect(decodeUpdates('nope')).toBeNull()
  })
})
