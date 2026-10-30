import { Geometry, Group, Material, Node, Skeleton, SkinnedMesh, mat4, Vector3 } from '@ragnarok/engine'
import { Box } from './helpers'
import { describe, expect, it } from 'vitest'
import { limbOf, poseLimb, splitLimbs } from '@/render/gibs'

/** Two triangles bound to different bones: one to the head, one to a thigh. */
function skinned() {
  const joints = new Uint8Array(6 * 4)
  // The first triangle follows bone 0 (the head), the second bone 1 (the thigh).
  for (let v = 3; v < 6; v++) joints[v * 4] = 1
  const weights = new Float32Array(6 * 4)
  for (let v = 0; v < 6; v++) weights[v * 4] = 1
  const geometry = new Geometry({
    positions: new Float32Array([0, 1, 0, 1, 1, 0, 0, 2, 0, 0, 0, 0, 1, 0, 0, 0, -1, 0]),
    normals: new Float32Array(18),
    joints,
    weights,
  })
  const root = new Group()
  const head = new Node()
  head.name = 'head'
  const thigh = new Node()
  thigh.name = 'thigh_L'
  root.add(head, thigh)
  const inverse = new Float32Array(32)
  mat4.identity(inverse.subarray(0, 16) as never)
  mat4.identity(inverse.subarray(16, 32) as never)
  const mesh = new SkinnedMesh(geometry, new Material(), new Skeleton([head, thigh], inverse))
  root.add(mesh)
  return { root, mesh }
}

describe('gibs', () => {
  it('groups bones into limbs that break at the elbow and the knee', () => {
    expect(limbOf('neck').key).toBe('head')
    expect(limbOf('hand_R')).toEqual({ key: 'forearm_R', bone: 'forearm_R' })
    expect(limbOf('foot_L').key).toBe('shin_L')
    expect(limbOf('hips').key).toBe('torso')
  })

  it('cuts every triangle into exactly one limb, centred on itself', () => {
    const { mesh } = skinned()
    const limbs = splitLimbs(mesh)
    expect(limbs.map((limb) => limb.key).sort()).toEqual(['head', 'thigh_L'])
    expect(limbs.reduce((sum, limb) => sum + limb.geometry.vertexCount, 0)).toBe(6)
    for (const limb of limbs) {
      // Centred: the middle of its box is the origin.
      const p = limb.geometry.positions
      const box = new Box()
      for (let i = 0; i < p.length; i += 3) box.expandByPoint(new Vector3(p[i], p[i + 1], p[i + 2]))
      expect(box.getCenter(new Vector3()).length()).toBeLessThan(1e-6)
    }
  })

  it('poses a limb where its bone has it', () => {
    const { root, mesh } = skinned()
    mesh.skeleton.bones[0]!.position.set(0, 0, 5)
    root.updateMatrixWorld(true)
    const head = splitLimbs(mesh).find((limb) => limb.key === 'head')!
    const group = poseLimb(mesh, head)!
    // The bone moved 5 m along z from its bind pose, so the limb is 5 m off its bind-pose centre.
    expect(group.position.z).toBeCloseTo(head.centre.z + 5)
  })
})
