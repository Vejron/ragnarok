import { describe, expect, it } from 'vitest'
import { ALLY_COLOR, BOT_COLOR, Blips, MAX_SHOTS, OWN_SHOT_RADIUS, SHOT_MS, UNKNOWN_COLOR } from '../blips'

const sides = {
  colorOf: (id: number) => `#${id}`,
  isBot: (id: number) => id >= 10,
  // 10 and 11 are on one team; the local player is 1, and 12 is on theirs.
  allies: (a: number, b: number) => a !== b && ((a >= 10 && a <= 11 && b >= 10 && b <= 11) || [a, b].sort().join() === '1,12'),
}

describe('Blips', () => {
  it('marks gunfire by who fired, fading, and forgets it', () => {
    const blips = new Blips()
    blips.fired(10, 50, 0, 0)
    blips.fired(3, 0, 50, 0)
    const now = blips.view(SHOT_MS / 2, 1, 0, 0, [], sides)
    expect(now.map((b) => [b.color, b.kind, b.strength])).toEqual([[BOT_COLOR, 'shot', 0.5], ['#3', 'shot', 0.5]])
    expect(blips.view(SHOT_MS, 1, 0, 0, [], sides)).toEqual([])
  })

  it('leaves out the player\'s own shots and their side\'s, and shells fired from next to them', () => {
    const blips = new Blips()
    blips.fired(1, 30, 0, 0)
    blips.fired(12, 30, 0, 0)
    blips.fired(0, OWN_SHOT_RADIUS - 1, 0, 0)
    blips.fired(0, 40, 0, 0)
    expect(blips.view(0, 1, 0, 0, [], sides).map((b) => [b.x, b.color])).toEqual([[40, UNKNOWN_COLOR]])
  })

  it('shows allies where they are, and a burst from one place as one mark', () => {
    const blips = new Blips()
    for (let i = 0; i < 10; i++) blips.fired(10, 100 + i * 0.1, 0, i)
    const view = blips.view(10, 11, 0, 0, [{ id: 12, x: 5, z: 5 }], sides)
    expect(view.map((b) => [b.kind, b.color])).toEqual([['ally', ALLY_COLOR]])
    expect(blips.view(10, 1, 0, 0, [], sides)).toHaveLength(1)
  })

  it('keeps only the latest shots', () => {
    const blips = new Blips()
    for (let i = 0; i < MAX_SHOTS + 5; i++) blips.fired(20, i * 10, 0, 0)
    expect(blips.view(0, 1, 0, 0, [], sides)[0]!.x).toBe(50)
  })
})
