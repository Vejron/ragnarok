import { existsSync, statSync } from 'node:fs'
import { resolve } from 'node:path'
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

  it('has every variation it names in public/sfx', () => {
    for (const [name, count] of Object.entries(SAMPLES)) {
      for (let n = 1; n <= count; n++) {
        const file = resolve(process.cwd(), `public/sfx/${name}_${n}.m4a`)
        expect(existsSync(file), file).toBe(true)
        expect(statSync(file).size).toBeGreaterThan(1000)
      }
    }
  })
})
