import { describe, expect, it } from 'vitest'
import { decodeServer as decode } from '../net/codec'
import { ARC_MS, HitFeedback, HIT_DEALT, HIT_KILL, HIT_TAKEN, KILL_MARKER_MS, MARKER_MS, VIGNETTE_MS } from '../feedback'
import { encode } from '@msgpack/msgpack'

const dealt = (amount: number, kind = 0) => ({ role: HIT_DEALT, kind, amount, weapon: 0, bearing: null })
const taken = (amount: number, bearing: number | null) => ({ role: HIT_TAKEN, kind: 0, amount, weapon: 0, bearing })

describe('hit feedback', () => {
  it('shows a marker for a landed hit and lets it end', () => {
    const feedback = new HitFeedback()
    expect(feedback.view(0, 0).marker).toBeNull()
    feedback.receive(dealt(25), 1000)
    expect(feedback.view(1000 + MARKER_MS / 2, 0).marker).toMatchObject({ kill: false, amount: 25 })
    expect(feedback.view(1000 + MARKER_MS + 1, 0).marker).toBeNull()
  })

  it('adds up hits in quick succession and keeps a kill marker longer', () => {
    const feedback = new HitFeedback()
    feedback.receive(dealt(10), 1000)
    feedback.receive(dealt(20, HIT_KILL), 1100)
    expect(feedback.view(1100 + MARKER_MS + 10, 0).marker).toMatchObject({ kill: true, amount: 30 })
    expect(feedback.view(1100 + KILL_MARKER_MS + 1, 0).marker).toBeNull()
  })

  it('flashes the screen harder for bigger hits and fades', () => {
    const small = new HitFeedback()
    small.receive(taken(5, null), 0)
    const big = new HitFeedback()
    big.receive(taken(60, null), 0)
    expect(big.view(0, 0).vignette).toBeGreaterThan(small.view(0, 0).vignette)
    expect(big.view(VIGNETTE_MS + 1, 0).vignette).toBe(0)
  })

  it('points an arc at where harm came from, relative to the way you look', () => {
    const feedback = new HitFeedback()
    // Attacker straight ahead of a camera looking along bearing 0: arc at the top.
    feedback.receive(taken(10, 0), 0)
    expect(feedback.view(0, 0).arcs[0]!.angle).toBeCloseTo(0)
    // Turning left (+yaw) by 90 degrees puts that attacker on the right of the screen (clockwise, +).
    expect(feedback.view(0, Math.PI / 2).arcs[0]!.angle).toBeCloseTo(Math.PI / 2)
    // Behind: the arc is at the bottom, either side of ±π.
    expect(Math.abs(feedback.view(0, Math.PI).arcs[0]!.angle)).toBeCloseTo(Math.PI)
    expect(feedback.view(ARC_MS + 1, 0).arcs).toHaveLength(0)
  })

  it('draws no arc for harm with no direction', () => {
    const feedback = new HitFeedback()
    feedback.receive(taken(10, null), 0)
    expect(feedback.view(0, 0).arcs).toHaveLength(0)
  })
})

describe('hit message', () => {
  it('decodes a taken hit with its bearing in radians', () => {
    const bytes = encode([8, 1, 0, 25, 0, -1571, 120, 90, -300, 7])
    expect(decode(bytes)).toEqual({ type: 'hit', role: 1, kind: 0, amount: 25, weapon: 0, bearing: -1.571, x: 1.2, y: 0.9, z: -3, target: 7 })
  })

  it('decodes the no-direction marker as null and rejects a short message', () => {
    expect(decode(encode([8, 0, 2, 100, 7, -32768, 0, 0, 0, 0]))).toMatchObject({ type: 'hit', bearing: null })
    expect(decode(encode([8, 0, 2]))).toBeNull()
  })
})
