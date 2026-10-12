import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import { limbOf, poseLimb, splitLimbs } from '@/render/gibs'

/** Two triangles bound to different bones: one to the head, one to a thigh. */
function skinned() {
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.Float32BufferAttribute([0, 1, 0, 1, 1, 0, 0, 2, 0, 0, 0, 0, 1, 0, 0, 0, -1, 0], 3))
  geometry.setAttribute('normal', new THREE.Float32BufferAttribute(new Array(18).fill(0), 3))
  const joints = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]
  geometry.setAttribute('skinIndex', new THREE.Uint16BufferAttribute(joints.map((j, i) => (i < 12 ? 0 : j)), 4))
  geometry.setAttribute('skinWeight', new THREE.Float32BufferAttribute(new Array(6).fill([1, 0, 0, 0]).flat(), 4))
  const head = new THREE.Bone()
  head.name = 'head'
  const thigh = new THREE.Bone()
  thigh.name = 'thigh_L'
  const mesh = new THREE.SkinnedMesh(geometry, new THREE.MeshBasicMaterial())
  mesh.add(head, thigh)
  mesh.bind(new THREE.Skeleton([head, thigh]))
  return mesh
}

describe('gibs', () => {
  it('groups bones into limbs that break at the elbow and the knee', () => {
    expect(limbOf('neck').key).toBe('head')
    expect(limbOf('hand_R')).toEqual({ key: 'forearm_R', bone: 'forearm_R' })
    expect(limbOf('foot_L').key).toBe('shin_L')
    expect(limbOf('hips').key).toBe('torso')
  })

  it('cuts every triangle into exactly one limb, centred on itself', () => {
    const mesh = skinned()
    const limbs = splitLimbs(mesh)
    expect(limbs.map((limb) => limb.key).sort()).toEqual(['head', 'thigh_L'])
    expect(limbs.reduce((sum, limb) => sum + limb.geometry.getAttribute('position').count, 0)).toBe(6)
    for (const limb of limbs) {
      limb.geometry.computeBoundingBox()
      expect(limb.geometry.boundingBox!.getCenter(new THREE.Vector3()).length()).toBeLessThan(1e-6)
    }
  })

  it('poses a limb where its bone has it', () => {
    const mesh = skinned()
    mesh.skeleton.bones[0]!.position.set(0, 0, 5)
    mesh.updateWorldMatrix(true, true)
    const head = splitLimbs(mesh).find((limb) => limb.key === 'head')!
    const group = poseLimb(mesh, head)!
    // The bone moved 5 m along z from its bind pose, so the limb is 5 m off its bind-pose centre.
    expect(group.position.z).toBeCloseTo(head.centre.z + 5)
  })
})
