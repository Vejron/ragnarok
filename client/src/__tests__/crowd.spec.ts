import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import { Crowd, Stickman } from '@/render/stickman'
import { MAX_PLAYERS } from '@/sim/consts'

/** Where instance `index` of the first mesh of the crowd (the heads) is, in world space. */
function headPosition(crowd: Crowd, index: number) {
  const heads = crowd.root.children[0] as THREE.InstancedMesh
  const matrix = new THREE.Matrix4()
  heads.getMatrixAt(index, matrix)
  return new THREE.Vector3().setFromMatrixPosition(matrix)
}

describe('Crowd', () => {
  it('draws every stickman in five instanced meshes', () => {
    const crowd = new Crowd()
    expect(crowd.root.children).toHaveLength(5)
    const people = Array.from({ length: 40 }, (_, i) => new Stickman(crowd, i + 1))
    people.forEach((person, i) => person.update([i, 0.87, 0], 0, 0, true, 0.016))
    expect(crowd.size).toBe(40)
    expect(crowd.root.children).toHaveLength(5)
    crowd.dispose()
  })

  it('puts the head above the capsule centre at the stickman’s place', () => {
    const crowd = new Crowd()
    const person = new Stickman(crowd, 1)
    person.update([10, 0.87, -4], 0, 0, true, 0.016)
    const head = headPosition(crowd, 0)
    expect(head.x).toBeCloseTo(10)
    expect(head.z).toBeCloseTo(-4)
    // Feet at ground level, head at 1.5 m on the body.
    expect(head.y).toBeCloseTo(0.87 - 0.85 + 1.5)
    crowd.dispose()
  })

  it('lays a dead stickman down, and hides one in a car', () => {
    const crowd = new Crowd()
    const person = new Stickman(crowd, 1)
    person.dead = true
    for (let i = 0; i < 20; i++) person.update([0, 0.87, 0], 0, 0, true, 0.1)
    expect(headPosition(crowd, 0).y).toBeLessThan(0.5)
    person.hidden = true
    expect(person.visible).toBe(false)
    expect(headPosition(crowd, 0).length()).toBe(0)
    person.hidden = false
    person.update([0, 0.87, 0], 0, 0, true, 0.1)
    expect(headPosition(crowd, 0).length()).toBeGreaterThan(0)
    crowd.dispose()
  })

  it('reuses the slots of stickmen that left, and has room for a full room', () => {
    const crowd = new Crowd()
    const first = Array.from({ length: MAX_PLAYERS }, (_, i) => new Stickman(crowd, i + 1))
    expect(crowd.size).toBe(MAX_PLAYERS)
    first.forEach((person) => person.dispose())
    expect(crowd.size).toBe(0)
    const again = Array.from({ length: MAX_PLAYERS }, (_, i) => new Stickman(crowd, i + 1))
    again.forEach((person, i) => person.update([i, 0.87, 0], 0, 0, true, 0.016))
    expect(crowd.size).toBe(MAX_PLAYERS)
    crowd.dispose()
  })
})
