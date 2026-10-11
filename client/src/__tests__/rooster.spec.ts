import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import { parseRoosterModel, RoosterModel } from '@/render/rooster'

const file = resolve(process.cwd(), 'public/models/rooster.glb')

async function load() {
  const bytes = readFileSync(file)
  // A copy made here, so it is an ArrayBuffer of the realm the loader checks against.
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseRoosterModel(copy.buffer)
}

describe('rooster model', () => {
  it('has the idle, walk and crow clips the game plays', async () => {
    await load()
    const rooster = new RoosterModel()
    let skinned = 0
    rooster.root.traverse((node) => {
      if ((node as THREE.SkinnedMesh).isSkinnedMesh) skinned++
    })
    expect(skinned).toBeGreaterThan(0)
    // The clip lengths are what the server's timings assume: a 50-frame crow at 30 fps is 100 ticks at 60 Hz.
    const clips = new Map<string, number>()
    const mixer = (rooster as unknown as { actions: Map<string, THREE.AnimationAction> }).actions
    for (const [name, action] of mixer) clips.set(name, action.getClip().duration)
    expect([...clips.keys()].sort()).toEqual(['crow', 'idle', 'walk'])
    expect(clips.get('crow')).toBeCloseTo(100 / 60, 1)
  })

  it('stands where it is put, facing its yaw, and reports the start of a crow once', async () => {
    await load()
    const rooster = new RoosterModel()
    expect(rooster.update([3, 1, -4], 0.5, false, 1 / 60)).toBe(false)
    expect(rooster.root.position.toArray()).toEqual([3, 1, -4])
    expect(rooster.root.rotation.y).toBeCloseTo(0.5)
    expect(rooster.update([3, 1, -4], 0.5, true, 1 / 60)).toBe(true)
    expect(rooster.update([3, 1, -4], 0.5, true, 1 / 60)).toBe(false)
    rooster.dispose()
  })
})
