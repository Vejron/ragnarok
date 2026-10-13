import { describe, expect, it } from 'vitest'
import { choosePeers, CONNECT_RADIUS, DROP_RADIUS, MAX_PEERS } from '@/voice'

describe('choosePeers', () => {
  it('connects to those within the connect radius and ignores the rest', () => {
    const result = choosePeers([{ id: 1, distance: 5 }, { id: 2, distance: CONNECT_RADIUS + 1 }], new Set())
    expect(result).toEqual({ connect: [1], drop: [] })
  })

  it('keeps a connection out to the drop radius, then drops it', () => {
    const between = CONNECT_RADIUS + 5
    expect(choosePeers([{ id: 1, distance: between }], new Set([1]))).toEqual({ connect: [], drop: [] })
    expect(choosePeers([{ id: 1, distance: DROP_RADIUS + 1 }], new Set([1]))).toEqual({ connect: [], drop: [1] })
  })

  it('drops a player who is no longer in view', () => {
    expect(choosePeers([], new Set([7]))).toEqual({ connect: [], drop: [7] })
  })

  it('holds at most MAX_PEERS, the nearest', () => {
    const candidates = Array.from({ length: MAX_PEERS + 3 }, (_, i) => ({ id: i + 1, distance: i + 1 }))
    const { connect } = choosePeers(candidates, new Set())
    expect(connect).toHaveLength(MAX_PEERS)
    expect(connect).toEqual(candidates.slice(0, MAX_PEERS).map((c) => c.id))
  })

  it('drops a far peer to make room for a nearer newcomer when full', () => {
    const current = new Set(Array.from({ length: MAX_PEERS }, (_, i) => i + 1))
    const candidates = [...current].map((id) => ({ id, distance: 10 + id }))
    candidates.push({ id: 99, distance: 1 })
    const result = choosePeers(candidates, current)
    expect(result.connect).toEqual([99])
    expect(result.drop).toEqual([MAX_PEERS])
  })
})
