import { afterEach, describe, expect, it } from 'vitest'
import {
  allies, colorFor, emptyProfile, forgetAllPlayers, forgetPlayer, isBot, loadProfile, nameFor, parseProfile, PLAYER_COLORS, profileQuery,
  sanitizeName, saveProfile, setPlayer, teamFor, worldKey,
} from '@/profile'
import { ITEM_GOLDEN_EGG } from '@/sim/items'

describe('profile', () => {
  afterEach(() => {
    forgetAllPlayers()
    localStorage.clear()
  })

  it('cleans names the way the server does', () => {
    expect(sanitizeName('  Bo\u0000b \n\t  Ek ')).toBe('Bob Ek')
    expect(sanitizeName('‮evil​﻿')).toBe('evil')
    expect(sanitizeName('abcdefghijklmnopqrstuvwxyz')).toBe('abcdefghijklmnop')
    expect(sanitizeName('abcdefghijklmno pq')).toBe('abcdefghijklmno')
    expect(sanitizeName('😀'.repeat(20))).toBe('😀'.repeat(16))
  })

  it('keeps only what is well formed from storage', () => {
    expect(parseProfile(null)).toEqual(emptyProfile())
    expect(parseProfile({ name: ' Åsa ', color: 99, spawns: { a: [[1, 2], [1.5, 2], 'x', [3, 4], [5, 6], [7, 8], [9, 10], [11, 12]], b: 'no' } })).toEqual({
      version: 1, name: 'Åsa', color: null, spawns: { a: [[1, 2], [3, 4], [5, 6], [7, 8], [9, 10]] }, loadout: null,
    })
  })

  it('keeps a loadout only if the server would take it', () => {
    expect(parseProfile({ loadout: [2, 0, 13, 0, 11] }).loadout).toEqual([2, 0, 13, 0, 11])
    for (const loadout of [[2, 5], [2, 2, 10, 8, 11], [2, ITEM_GOLDEN_EGG, 10, 8, 11], [2, 99, 10, 8, 11], [2, 'x', 10, 8, 11], 'no']) {
      expect(parseProfile({ loadout }).loadout).toBeNull()
    }
  })

  it('sends a chosen loadout, and nothing for the standard one', () => {
    expect(profileQuery({ ...emptyProfile(), loadout: [2, 0, 13, 0, 11] }, 7)).toBe('loadout=2%2C0%2C13%2C0%2C11')
    expect(profileQuery(emptyProfile(), 7)).toBe('')
  })

  it('saves and loads, and survives junk in storage', () => {
    const profile = { ...emptyProfile(), name: 'Åsa', color: 4, spawns: { [worldKey(7)]: [[100, -200]] as [number, number][] } }
    saveProfile(profile)
    expect(loadProfile()).toEqual(profile)
    localStorage.setItem('ragnarok.profile', '{not json')
    expect(loadProfile()).toEqual(emptyProfile())
  })

  it('sends the spawns of this world only', () => {
    const profile = { ...emptyProfile(), name: 'Bo Ek', color: 2, spawns: { [worldKey(7)]: [[100, -200], [0, 5]] as [number, number][], [worldKey(8)]: [[9, 9]] as [number, number][] } }
    expect(profileQuery(profile, 7)).toBe('name=Bo+Ek&color=2&spawn=100%2C-200&spawn=0%2C5')
    expect(profileQuery(profile, 8)).toBe('name=Bo+Ek&color=2&spawn=9%2C9')
    expect(profileQuery(emptyProfile(), 7)).toBe('')
  })

  it('names and colours players as the server says, by id until then', () => {
    expect(nameFor(3)).toBe('Player 3')
    expect(colorFor(3)).toBe(PLAYER_COLORS[3]!.hex)
    setPlayer(3, 'Åsa', 9)
    expect(nameFor(3)).toBe('Åsa')
    expect(colorFor(3)).toBe(PLAYER_COLORS[9]!.hex)
    forgetPlayer(3)
    expect(nameFor(3)).toBe('Player 3')
  })

  it('knows bots and sides, and that people are on nobody\'s', () => {
    setPlayer(4, 'Bot 1', 0, 2, true)
    setPlayer(5, 'Bot 2', 0, 2, true)
    setPlayer(6, 'Åsa', 0)
    setPlayer(7, 'Bo', 0)
    expect([isBot(4), isBot(6), teamFor(4), teamFor(6), teamFor(99)]).toEqual([true, false, 2, 0, 0])
    expect([allies(4, 5), allies(4, 4), allies(6, 7), allies(4, 6)]).toEqual([true, false, false, false])
    forgetAllPlayers()
  })
})
