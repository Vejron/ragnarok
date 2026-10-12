import { describe, expect, it } from 'vitest'
import { Backoff } from '../net/backoff'

describe('Backoff', () => {
  it('doubles up to the cap', () => {
    const backoff = new Backoff(1000, 8000, () => 0.5)
    expect([1, 2, 3, 4, 5, 6].map(() => backoff.next())).toEqual([1000, 2000, 4000, 8000, 8000, 8000])
  })

  it('starts over after a connection worked', () => {
    const backoff = new Backoff(1000, 8000, () => 0.5)
    backoff.next()
    backoff.next()
    backoff.reset()
    expect(backoff.next()).toBe(1000)
  })

  it('spreads each delay by a quarter either way', () => {
    expect(new Backoff(1000, 8000, () => 0).next()).toBe(750)
    expect(new Backoff(1000, 8000, () => 1).next()).toBe(1250)
  })

  it('never overflows however long the server is away', () => {
    const backoff = new Backoff(1000, 30_000, () => 0.5)
    for (let i = 0; i < 100; i++) backoff.next()
    expect(backoff.next()).toBe(30_000)
  })
})
