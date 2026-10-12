import { describe, expect, it } from 'vitest'
import { FEED_LENGTH, FEED_MS, Scoreboard } from '../scoreboard'

const kill = (killer: number, victim: number, killerKills = 1, victimDeaths = 1) => ({ killer, victim, weapon: 0, killerKills, victimDeaths })

describe('Scoreboard', () => {
  it('takes the totals the server sends rather than counting', () => {
    const board = new Scoreboard()
    board.kill(kill(7, 9, 4, 2), 0)
    expect([board.kills(7), board.deaths(9), board.kills(9)]).toEqual([4, 2, 0])
  })

  it('credits nobody when the world did it', () => {
    const board = new Scoreboard()
    board.kill(kill(0, 9, 0, 3), 0)
    expect(board.deaths(9)).toBe(3)
    expect(board.rows(null, []).map((row) => row.id)).toEqual([9])
  })

  it('marks the local player in the feed and expires old kills', () => {
    const board = new Scoreboard()
    board.kill(kill(7, 9), 1000)
    board.kill(kill(3, 7), 2000)
    expect(board.recent(2500, 7).map((e) => e.you)).toEqual(['killer', 'victim'])
    expect(board.recent(1000 + FEED_MS, 7)).toHaveLength(1)
  })

  it('keeps only the newest few kills', () => {
    const board = new Scoreboard()
    for (let i = 1; i <= FEED_LENGTH + 3; i++) board.kill(kill(i, 100), 0)
    expect(board.recent(0, null).map((e) => e.killer)).toEqual([4, 5, 6, 7, 8])
  })

  it('ranks by kills, then fewer deaths, then id, and includes you and those in view', () => {
    const board = new Scoreboard()
    board.replace([{ id: 1, kills: 2, deaths: 5 }, { id: 2, kills: 2, deaths: 1 }, { id: 3, kills: 9, deaths: 9 }])
    expect(board.rows(8, [4]).map((r) => r.id)).toEqual([3, 2, 1, 4, 8])
    expect(board.rows(8, []).find((r) => r.id === 8)?.you).toBe(true)
  })

  it('forgets a player who left and clears on disconnect', () => {
    const board = new Scoreboard()
    board.replace([{ id: 1, kills: 2, deaths: 0 }])
    board.forget(1)
    expect(board.kills(1)).toBe(0)
    board.kill(kill(2, 3), 0)
    board.clear()
    expect(board.recent(0, null)).toEqual([])
    expect(board.rows(null, [])).toEqual([])
  })
})
