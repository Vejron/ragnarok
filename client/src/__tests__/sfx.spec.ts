import { afterEach, describe, expect, it, vi } from 'vitest'
import { SAMPLES, SampleBank } from '@/sfx'

afterEach(() => vi.unstubAllGlobals())

describe('SampleBank', () => {
  it('says nothing until a variation has decoded, then picks one', async () => {
    const decoded = { duration: 1 } as AudioBuffer
    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: true, arrayBuffer: async () => new ArrayBuffer(4) })))
    const ctx = { decodeAudioData: vi.fn(async () => decoded) } as unknown as BaseAudioContext
    const bank = new SampleBank(ctx, '/sfx/')
    expect(bank.pick('rifle')).toBeNull()
    bank.load()
    await vi.waitFor(() => expect(bank.pick('rifle')).toBe(decoded))
    expect(fetch).toHaveBeenCalledTimes(Object.values(SAMPLES).reduce((a, b) => a + b, 0))
    expect(bank.pick('nothing')).toBeNull()
  })

  it('survives files that are missing or will not decode', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false })))
    const bank = new SampleBank({ decodeAudioData: async () => ({}) } as unknown as BaseAudioContext, '/sfx/')
    bank.load()
    await new Promise((r) => setTimeout(r, 0))
    expect(bank.pick('boom')).toBeNull()
  })
})
