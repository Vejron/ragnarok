import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Group, Mesh, SkinnedMesh, type Action } from '@ragnarok/engine'
import { describe, expect, it } from 'vitest'
import { Animals } from '@/entities/presenters/animals'
import type { Entity } from '@/entities/units'
import { DEATH_BLAST, DEATH_BODY, FLAG_DEAD, KIND_DEER, KIND_STAG } from '@/net/codec'
import type { Gore } from '@/render/gore'
import { AnimalModel, clipFor, parseAnimalModel } from '@/render/animal'

function load(kind: number, name: string) {
  const bytes = readFileSync(resolve(process.cwd(), `public/models/${name}.glb`))
  // A copy made here, so it is an ArrayBuffer of the realm the loader checks against.
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  parseAnimalModel(kind, copy.buffer)
}

describe('animal models', () => {
  it.each([
    ['deer', KIND_DEER],
    ['stag', KIND_STAG],
  ])('the %s has the idle, graze, walk, gallop and death clips the game plays', (name, kind) => {
    load(kind, name)
    const animal = new AnimalModel(kind)
    let skinned = 0
    let rigid = 0
    animal.root.traverse((node) => {
      if (node instanceof SkinnedMesh) skinned++
      else if (node instanceof Mesh) rigid++
    })
    expect(skinned).toBeGreaterThan(0)
    // The stag's antlers ride on its head bone, unskinned.
    expect(rigid > 0).toBe(name === 'stag')
    const animator = (animal as unknown as { animator: { actions: Map<string, Action> } }).animator
    expect([...animator.actions.keys()].sort()).toEqual(['death', 'gallop', 'graze', 'idle', 'walk'])
    animal.dispose()
  })

  it('grazes standing, walks slowly and gallops fast', () => {
    expect(clipFor(0, true)).toBe('graze')
    expect(clipFor(0, false)).toBe('idle')
    expect(clipFor(1.3, true)).toBe('walk')
    expect(clipFor(9, false)).toBe('gallop')
    expect(clipFor(9, false, true)).toBe('death')
  })

  it('stands where it is put, facing its yaw, and blends into a gallop as it runs', () => {
    load(KIND_DEER, 'deer')
    const deer = new AnimalModel(KIND_DEER)
    deer.update([3, 1, -4], 0.5, true, 1 / 60)
    expect(deer.root.position.toArray()).toEqual([3, 1, -4])
    expect(deer.root.rotation.y).toBeCloseTo(0.5)
    for (let i = 0; i < 60; i++) deer.update([3, 1, -4], 0.5, true, 1 / 60)
    expect(deer.playing).toBe('graze')
    for (let i = 1; i <= 60; i++) deer.update([3, 1, -4 - (9 * i) / 60], 0, false, 1 / 60)
    expect(deer.playing).toBe('gallop')
    deer.dispose()
  })
})

describe('animal blood', () => {
  const frame = (now: number) => ({ tick: 0, dt: 1 / 60, now, localId: null })
  const deer = (more: Partial<Entity> = {}): { entity: Entity; velocity: null } => ({
    entity: { kind: { kind: KIND_DEER }, place: { x: 1, y: 2, z: 3 }, heading: { yaw: 0 }, flags: { flags: 1, stance: 0 }, ...more } as Entity,
    velocity: null,
  })
  const struck = (count: number, amount: number) => ({ count, part: 0, amount, yaw: 0.5, x: 0, y: 0, z: 0 })
  function setup() {
    load(KIND_DEER, 'deer')
    const calls = { wound: 0, fell: [] as [number, number][], drip: 0 }
    const gore = {
      wound: () => calls.wound++,
      fell: (_: unknown, style: number, _from: number, pieces: unknown[]) => calls.fell.push([style, pieces.length]),
      dripAt: () => calls.drip++,
    } as unknown as Gore
    const animals = new Animals({ root: new Group(), blobFoot: () => {}, gore })
    return { animals, calls }
  }

  it('bleeds for each new hit, not for one it took before it was first drawn, and drips while wounded', () => {
    const { animals, calls } = setup()
    animals.update(7, deer({ struck: struck(2, 10) }), frame(0))
    expect(calls.wound).toBe(0)
    animals.update(7, deer({ struck: struck(3, 30) }), frame(16))
    animals.update(7, deer({ struck: struck(3, 30) }), frame(32))
    expect(calls.wound).toBe(1)
    for (let t = 48; t < 3000; t += 16) animals.update(7, deer({ struck: struck(3, 30) }), frame(t))
    expect(calls.drip).toBeGreaterThan(4)
  })

  it('falls and bleeds out when shot dead, and is torn into pieces by a blast', () => {
    const { animals, calls } = setup()
    animals.update(1, deer(), frame(0))
    animals.update(1, deer({ flags: { flags: 1 | FLAG_DEAD, stance: 0 }, fell: { style: DEATH_BODY }, struck: struck(1, 60) }), frame(16))
    animals.update(2, deer(), frame(0))
    animals.update(2, deer({ flags: { flags: 1 | FLAG_DEAD, stance: 0 }, fell: { style: DEATH_BLAST } }), frame(16))
    expect(calls.fell).toEqual([[DEATH_BODY, 0], [DEATH_BLAST, 10]])
    // One already lying dead when first drawn is just lying there.
    animals.update(3, deer({ flags: { flags: 1 | FLAG_DEAD, stance: 0 }, fell: { style: DEATH_BODY } }), frame(0))
    expect(calls.fell.length).toBe(2)
  })
})
