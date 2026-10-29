import { describe, expect, it } from 'vitest'
import * as THREE from 'three'
import { Bloodstains } from '@/render/bloodstains'
import { Debris } from '@/render/debris'
import { Effects } from '@/render/effects'
import { Gore } from '@/render/gore'
import { Particles } from '@/render/particles'
import type { Character } from '@/render/character'
import { DEATH_BODY, LIMB_LEG_L, PART_TORSO } from '@/net/codec'

describe('Bloodstains', () => {
  it('lays a blot at once and joins a smear into a strip the way it was dragged', () => {
    const stains = new Bloodstains(10)
    stains.blot(0, 0, 0, 0.2)
    expect(stains.laid).toBe(1)
    stains.smear('a', new THREE.Vector3(0, 0, 0), 0.4)
    stains.smear('a', new THREE.Vector3(0, 0, -0.1), 0.4)
    expect(stains.laid).toBe(1)
    stains.smear('a', new THREE.Vector3(0, 0, -1), 0.4)
    expect(stains.laid).toBe(2)
    // A jump starts over rather than smearing across the gap.
    stains.smear('a', new THREE.Vector3(0, 0, -20), 0.4)
    stains.smear('a', new THREE.Vector3(0, 0, -21), 0.4)
    expect(stains.laid).toBe(2)
    stains.lift('a')
    stains.smear('a', new THREE.Vector3(0, 0, -22), 0.4)
    expect(stains.laid).toBe(2)
  })

  it('never holds more than its capacity: the oldest are laid over', () => {
    const stains = new Bloodstains(3)
    for (let i = 0; i < 5; i++) stains.blot(i * 10, 0, 0, 0.2)
    expect(stains.laid).toBe(5)
    const xs = (stains as unknown as { positions: Float32Array }).positions
    expect(xs.length).toBe(3 * 4 * 3)
    // Quad 0 now holds the fourth blot, quad 1 the fifth, quad 2 still the third.
    expect([xs[0]! + xs[3]!, xs[12]! + xs[15]!, xs[24]! + xs[27]!].map((x) => Math.round(x / 2))).toEqual([30, 40, 20])
  })
})

describe('Gore on the ground', () => {
  const setup = () => {
    const stains = new Bloodstains(500)
    const gore = new Gore(new Debris(), new Particles(2000), new Effects(64), stains, () => 0)
    const position = new THREE.Vector3(0, 0.9, 0)
    const avatar = { position, stump: (_: number, out = new THREE.Vector3()) => out.copy(position), launch: () => {} } as unknown as Character
    return { stains, gore, avatar }
  }

  it('stains the ground where the blood from a hit comes down, not before', () => {
    const { stains, gore, avatar } = setup()
    for (let i = 0; i < 10; i++) gore.hit(avatar, PART_TORSO, 0, 40)
    expect(stains.laid).toBe(0)
    for (let t = 0; t < 2; t += 1 / 60) gore.update(1 / 60)
    expect(stains.laid).toBeGreaterThan(10)
  })

  it('leaves a pool under a body once it has come to rest', () => {
    const { stains, gore, avatar } = setup()
    gore.die(avatar, DEATH_BODY, 0, 5, () => 0)
    for (let t = 0; t < 3; t += 1 / 60) gore.update(1 / 60)
    expect(stains.laid).toBeGreaterThan(1)
  })

  it('smears the ground behind a crawler', () => {
    const { stains, gore, avatar } = setup()
    for (let z = 0; z > -3; z -= 0.5) {
      avatar.position.z = z
      gore.drag('crawl1', avatar)
    }
    expect(stains.laid).toBe(5)
  })
})
