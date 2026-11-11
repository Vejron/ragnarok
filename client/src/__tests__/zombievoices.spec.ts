import { describe, expect, it } from 'vitest'
import { ZombieVoices } from '@/zombievoices'

describe('ZombieVoices', () => {
  function voices() {
    const heard = { groans: [] as number[], claws: 0 }
    const v = new ZombieVoices({ groan: (_at, pitch) => heard.groans.push(pitch), claw: () => heard.claws++ }, () => 0.5)
    return { v, heard }
  }

  it('groans every few seconds per zombie, each in a voice of its own', () => {
    const { v, heard } = voices()
    for (let t = 0; t < 30; t += 0.1) {
      v.hear(1, 0, 0, 0, false, 0.1)
      v.hear(2, 5, 0, 0, false, 0.1)
      v.sweep()
    }
    expect(heard.groans.length).toBeGreaterThanOrEqual(4)
    expect(heard.groans.length).toBeLessThanOrEqual(12)
    expect(new Set(heard.groans).size).toBe(2)
  })

  it('growls once each time it starts clawing, not every frame of it', () => {
    const { v, heard } = voices()
    for (const clawing of [false, true, true, true, false, true]) v.hear(1, 0, 0, 0, clawing, 1 / 60)
    expect(heard.claws).toBe(2)
  })
})
