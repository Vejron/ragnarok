import { describe, expect, it } from 'vitest'
import { ConnectionHealth } from '@/net/health'

/** A session joined at 0 ms, with snapshots and inputs flowing normally up to `until`; returns the health and the next input's seq. */
function playing(until: number, correction = 0.02) {
  const health = new ConnectionHealth()
  health.connected(0)
  health.welcomed(100)
  let seq = 0
  for (let t = 100; t <= until; t += 1000 / 60) {
    health.sent(++seq, t)
    health.heard(t)
    // The server acknowledges what it has had for about 100 ms.
    if (Math.round(t / (1000 / 30)) % 2 === 0) {
      health.snapshot(t, true, Math.max(0, seq - 6))
      health.corrected(correction, t)
    }
  }
  return { health, seq }
}

/** Checks once a second from `from` to `to`, ms, and returns the first trouble found. */
function checks(health: ConnectionHealth, from: number, to: number) {
  for (let t = from; t <= to; t += 1000) {
    const trouble = health.check(t)
    if (trouble) return { at: t, ...trouble }
  }
  return null
}

describe('connection health', () => {
  it('a session that plays normally is left alone', () => {
    const { health } = playing(20_000)
    expect(checks(health, 1000, 20_000)).toBeNull()
    expect(health.state).toBe('ok')
  })

  it('a server that falls silent is reconnected to', () => {
    const { health } = playing(10_000)
    const trouble = checks(health, 11_000, 20_000)
    expect(trouble?.kind).toBe('silent')
    expect(trouble!.at).toBeLessThanOrEqual(16_000)
  })

  it('a session never welcomed is given up on', () => {
    const health = new ConnectionHealth()
    health.connected(0)
    const trouble = checks(health, 1000, 15_000)
    expect(trouble?.kind).toBe('silent')
    expect(trouble!.at).toBeGreaterThan(10_000)
  })

  it('a page that was not running is not judged on its first check back', () => {
    const { health } = playing(10_000)
    health.check(10_000)
    // The main thread was blocked for 8 s: the check comes late, and what was queued is read after it.
    expect(health.check(18_000)).toBeNull()
    health.heard(18_100)
    expect(health.check(19_000)).toBeNull()
  })

  it('a soldier corrected all the time is resynced', () => {
    const { health } = playing(10_000, 1.2)
    expect(checks(health, 1000, 10_000)?.kind).toBe('resync')
  })

  it('a single knock or a respawn is not', () => {
    const { health } = playing(10_000)
    health.corrected(5, 9000)
    health.corrected(2, 9100)
    expect(checks(health, 1000, 10_000)).toBeNull()
  })

  it('inputs the server does not acknowledge are resynced', () => {
    const health = new ConnectionHealth()
    health.connected(0)
    health.welcomed(100)
    let seq = 0
    for (let t = 100; t <= 10_000; t += 1000 / 60) {
      health.sent(++seq, t)
      health.heard(t)
      health.snapshot(t, true, 0)
    }
    expect(checks(health, 1000, 10_000)?.reason).toBe('inputs not acknowledged')
  })

  it('a tab put away, sending nothing, is not taken for unacknowledged inputs', () => {
    const health = new ConnectionHealth()
    health.connected(0)
    health.welcomed(100)
    for (let t = 100; t <= 10_000; t += 1000 / 30) {
      health.heard(t)
      health.snapshot(t, true, 0)
    }
    expect(checks(health, 1000, 10_000)).toBeNull()
  })

  it('snapshots that cannot be rebuilt are resynced', () => {
    const health = new ConnectionHealth()
    health.connected(0)
    health.welcomed(100)
    for (let t = 100; t <= 10_000; t += 1000 / 30) {
      health.heard(t)
      health.snapshot(t, false, 0)
    }
    expect(checks(health, 1000, 10_000)?.reason).toBe('snapshots not rebuilt')
  })

  it('resyncs are spaced out, and after three it gives up', () => {
    const health = new ConnectionHealth()
    const kinds: string[] = []
    for (let start = 0; start < 200_000; start += 10_000) {
      // Every session is out of step from the start.
      health.connected(start)
      health.welcomed(start)
      for (let t = start; t < start + 10_000; t += 1000 / 30) {
        health.heard(t)
        health.snapshot(t, true, 0)
        health.corrected(1.5, t)
      }
      const trouble = checks(health, start + 1000, start + 9000)
      if (trouble) kinds.push(`${trouble.kind}@${start / 1000}`)
      health.disconnected()
    }
    expect(kinds).toEqual(['resync@0', 'resync@30', 'resync@60', 'give-up@90'])
  })
})
