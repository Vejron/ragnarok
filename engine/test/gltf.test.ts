import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { Animator } from '../src/animation/animator'
import { parseGlb } from '../src/assets/gltf'
import { Mesh } from '../src/scene/mesh'
import { SkinnedMesh } from '../src/scene/skin'

// The game's own models: the loader only has to read what they use.
const MODELS = join(__dirname, '../../client/public/models')
const load = (name: string) => {
  const file = readFileSync(join(MODELS, name))
  return parseGlb(file.buffer.slice(file.byteOffset, file.byteOffset + file.byteLength))
}

describe('parseGlb', () => {
  for (const name of readdirSync(MODELS).filter((f) => f.endsWith('.glb'))) {
    it(`reads ${name}`, () => {
      const { scene } = load(name)
      let meshes = 0
      scene.traverse((node) => {
        if (!(node instanceof Mesh)) return
        meshes++
        const g = node.geometry
        expect(g.positions.length % 3).toBe(0)
        expect(g.normals?.length).toBe(g.positions.length)
        const max = g.indices ? Math.max(...g.indices) : 0
        expect(max).toBeLessThan(g.vertexCount)
      })
      expect(meshes).toBeGreaterThan(0)
    })
  }

  it('binds the player skeleton and clones it onto new bones', () => {
    const { scene, clips } = load('player.glb')
    let skinned: SkinnedMesh | undefined
    scene.traverse((node) => { if (node instanceof SkinnedMesh) skinned ??= node })
    expect(skinned).toBeDefined()
    expect(skinned!.skeleton.bones.length).toBeGreaterThan(5)
    expect(skinned!.skeleton.inverseBind.length).toBe(skinned!.skeleton.bones.length * 16)
    expect(clips.length).toBe(64)

    const copy = scene.clone()
    let copied: SkinnedMesh | undefined
    copy.traverse((node) => { if (node instanceof SkinnedMesh) copied ??= node })
    const bone = copied!.skeleton.bones[0]!
    expect(skinned!.skeleton.bones).not.toContain(bone)
    let inCopy = false
    copy.traverse((node) => { if (node === bone) inCopy = true })
    expect(inCopy).toBe(true)
  })

  it('poses bones from a clip and returns to rest at zero weight', () => {
    const { scene, clips } = load('player.glb')
    const animator = new Animator(scene, clips)
    const walk = [...animator.actions.values()].find((a) => a.clip.duration > 0.3)!
    let bone = scene
    scene.traverse((node) => { if (walk.clip.tracks.some((t) => t.node === node.name && t.path === 'rotation')) bone = node })
    const rest = [bone.quaternion.x, bone.quaternion.y, bone.quaternion.z, bone.quaternion.w]
    walk.weight = 1
    walk.time = walk.clip.duration * 0.37
    animator.apply()
    const posed = [bone.quaternion.x, bone.quaternion.y, bone.quaternion.z, bone.quaternion.w]
    expect(Math.hypot(...posed)).toBeCloseTo(1, 5)
    walk.weight = 0
    animator.apply()
    expect([bone.quaternion.x, bone.quaternion.y, bone.quaternion.z, bone.quaternion.w].map((v) => +v.toFixed(5)))
      .toEqual(rest.map((v) => +v.toFixed(5)))
  })
})
