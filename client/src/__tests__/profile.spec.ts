import { afterEach, describe, expect, it } from 'vitest'
import {
  colorFor, emptyProfile, forgetAllPlayers, forgetPlayer, loadProfile, nameFor, parseProfile, PLAYER_COLORS, profileQuery,
  sanitizeName, saveProfile, setPlayer, worldKey,
} from '@/profile'

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
      version: 1, name: 'Åsa', color: null, spawns: { a: [[1, 2], [3, 4], [5, 6], [7, 8], [9, 10]] },
    })
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
})
