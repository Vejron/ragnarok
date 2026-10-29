import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { Avatars } from '@/entities/avatars'
import { Crowd, parseCharacter } from '@/render/character'

async function crowd() {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/player.glb'))
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  const c = new Crowd()
  c.asset = await parseCharacter(copy.buffer)
  return c
}

describe('Avatars', () => {
  it('keeps a player’s soldier for whoever draws it, and lets go of those nobody drew', async () => {
    const avatars = new Avatars(await crowd())
    avatars.localId = 9
    avatars.begin()
    const local = avatars.local()
    const walking = avatars.draw(4)
    avatars.draw(5)
    avatars.sweep(() => {})
    // Next frame 4 sits in a car, drawn by the vehicle instead; 5 went out of range.
    avatars.begin()
    expect(avatars.draw(4)).toBe(walking)
    const gone: number[] = []
    avatars.sweep((key) => gone.push(key))
    expect(gone).toEqual([5])
    expect(avatars.of(5)).toBeUndefined()
    // The local player's is kept whether drawn or not, and found by their id.
    expect(avatars.of(9)).toBe(local)
    expect([...avatars.drawn]).toEqual([0, 4])
  })
})
