import { describe, expect, it } from 'vitest'
import { Group, Node, Quat, Vec3 } from '../src'

describe('rotation', () => {
  it('mirrors the quaternion in both directions', () => {
    const node = new Node()
    node.rotation.set(0.3, -1.1, 0.5, 'YXZ')
    const q = new Quat().setFromEuler(0.3, -1.1, 0.5, 'YXZ')
    expect(node.quaternion.x).toBeCloseTo(q.x)
    expect(node.quaternion.w).toBeCloseTo(q.w)
    node.quaternion.setFromAxisAngle(0, 1, 0, 0.7)
    expect(node.rotation.y).toBeCloseTo(0.7)
    expect(node.rotation.x).toBeCloseTo(0)
    node.rotation.x += 0.25
    expect(node.rotation.x).toBeCloseTo(0.25)
    expect(node.rotation.y).toBeCloseTo(0.7)
  })

  it('round-trips every order', () => {
    for (const order of ['XYZ', 'YXZ', 'ZXY', 'ZYX', 'YZX', 'XZY'] as const) {
      const node = new Node()
      node.rotation.set(0.4, 0.9, -0.6, order)
      const q = node.quaternion.clone()
      node.quaternion.set(0, 0, 0, 1)
      node.quaternion.copy(q)
      expect(node.rotation.x).toBeCloseTo(0.4)
      expect(node.rotation.y).toBeCloseTo(0.9)
      expect(node.rotation.z).toBeCloseTo(-0.6)
    }
  })
})

describe('hierarchy', () => {
  it('attach keeps the world transform', () => {
    const a = new Group()
    a.position.set(5, 0, 0)
    a.quaternion.setFromAxisAngle(0, 1, 0, Math.PI / 2)
    const b = new Group()
    b.position.set(0, 2, 3)
    const root = new Group()
    root.add(a, b)
    root.updateWorld()
    const before = b.getWorldPosition(new Vec3())
    a.attach(b)
    root.updateWorld()
    const after = b.getWorldPosition(new Vec3())
    expect(after.x).toBeCloseTo(before.x)
    expect(after.y).toBeCloseTo(before.y)
    expect(after.z).toBeCloseTo(before.z)
  })

  it('maps points to and from local space', () => {
    const node = new Group()
    node.position.set(1, 2, 3)
    node.scale.set(2, 2, 2)
    node.updateWorld()
    const world = node.localToWorld(new Vec3(1, 0, 0))
    expect(world.x).toBeCloseTo(3)
    const local = node.worldToLocal(world.clone())
    expect(local.x).toBeCloseTo(1)
  })

  it('slerp and setFromUnitVectors', () => {
    const q = new Quat().setFromUnitVectors({ x: 0, y: 1, z: 0 }, { x: 1, y: 0, z: 0 })
    const v = new Vec3(0, 1, 0).applyQuaternion(q)
    expect(v.x).toBeCloseTo(1)
    expect(v.y).toBeCloseTo(0)
    const half = new Quat().slerp(q, 0.5)
    expect(half.w).toBeCloseTo(Math.cos(Math.PI / 8))
  })
})
