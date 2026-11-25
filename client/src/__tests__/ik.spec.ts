import { Group, Matrix4, Quaternion, Vector3 } from '@ragnarok/engine'
import { describe, expect, it } from 'vitest'
import { Follow, Inertia, rotationOf, turnWorld, twoBone, worldRotation } from '@/render/ik'

/** Where a chain's middle and end go once `root` and `mid` (world turns) are applied to it. */
function solved(a: Vector3, b: Vector3, c: Vector3, root: Quaternion, mid: Quaternion) {
  const knee = b.clone().sub(a).applyQuaternion(root).add(a)
  const end = c.clone().sub(b).applyQuaternion(mid).applyQuaternion(root).add(knee)
  return { knee, end }
}

/** A leg of nodes: a hip turned off square, a knee 0.45 m below it and an ankle 0.43 m below that. */
function leg() {
  const model = new Group()
  model.rotation.set(0, 0.7, 0)
  const hip = new Group()
  hip.position.set(0.1, 0.9, 0)
  hip.quaternion.setFromAxisAngle(new Vector3(1, 0, 0), 0.2)
  const knee = new Group()
  knee.position.set(0, -0.45, 0)
  knee.quaternion.setFromAxisAngle(new Vector3(1, 0, 0), -0.4)
  const ankle = new Group()
  ankle.position.set(0, -0.43, 0)
  model.add(hip)
  hip.add(knee)
  knee.add(ankle)
  model.updateWorldMatrix(true, true)
  return { model, hip, knee, ankle }
}

const at = (node: Group) => node.getWorldPosition(new Vector3())

describe('Two-bone IK', () => {
  it('puts the end on a target it can reach, the bones keeping their lengths', () => {
    const a = new Vector3(0, 1, 0)
    const b = new Vector3(0, 0.55, 0.08)
    const c = new Vector3(0, 0.1, 0)
    const target = new Vector3(0.1, 0.3, 0.15)
    const root = new Quaternion()
    const mid = new Quaternion()
    twoBone(a, b, c, target, new Vector3(0, 0, 1), root, mid)
    const { knee, end } = solved(a, b, c, root, mid)
    expect(end.distanceTo(target)).toBeLessThan(1e-4)
    expect(knee.distanceTo(a)).toBeCloseTo(a.distanceTo(b), 5)
    expect(end.distanceTo(knee)).toBeCloseTo(b.distanceTo(c), 5)
  })

  it('reaches straight towards a target too far away', () => {
    const a = new Vector3(0, 1, 0)
    const b = new Vector3(0, 0.5, 0.05)
    const c = new Vector3(0, 0, 0)
    const target = new Vector3(0, -2, 0.5)
    const root = new Quaternion()
    const mid = new Quaternion()
    twoBone(a, b, c, target, new Vector3(0, 0, 1), root, mid)
    const { end } = solved(a, b, c, root, mid)
    const way = target.clone().sub(a).normalize()
    expect(end.clone().sub(a).normalize().dot(way)).toBeGreaterThan(0.9999)
    expect(end.distanceTo(a)).toBeGreaterThan(a.distanceTo(b) + b.distanceTo(c) - 0.002)
  })

  it('bends a straight chain the way it is told', () => {
    const a = new Vector3(0, 1, 0)
    const b = new Vector3(0, 0.5, 0)
    const c = new Vector3(0, 0, 0)
    const root = new Quaternion()
    const mid = new Quaternion()
    twoBone(a, b, c, new Vector3(0, 0.3, 0), new Vector3(0, 0, -1), root, mid)
    const { knee, end } = solved(a, b, c, root, mid)
    expect(knee.z).toBeLessThan(-0.1)
    expect(end.distanceTo(new Vector3(0, 0.3, 0))).toBeLessThan(1e-4)
  })

  it('turns the bones of a posed, turned model so its ankle lands on the target', () => {
    const { hip, knee, ankle } = leg()
    const target = at(ankle).add(new Vector3(0.05, 0.12, -0.04))
    const root = new Quaternion()
    const mid = new Quaternion()
    twoBone(at(hip), at(knee), at(ankle), target, new Vector3(0, 0, -1), root, mid)
    turnWorld(knee, mid)
    turnWorld(hip, root)
    expect(at(ankle).distanceTo(target)).toBeLessThan(1e-4)
  })
})

describe('Rotations from world matrices', () => {
  it('reads the rotation out of a scaled matrix', () => {
    const q = new Quaternion().setFromAxisAngle(new Vector3(1, 2, 3).normalize(), 2.4)
    const m = new Matrix4().compose(new Vector3(1, 2, 3), q, new Vector3(0.5, 2, 3))
    const out = rotationOf(m.elements, new Quaternion())
    expect(Math.abs(out.dot(q))).toBeCloseTo(1, 5)
  })

  it('agrees with a node chain', () => {
    const { model, knee } = leg()
    const want = new Quaternion().copy(model.quaternion).multiply(knee.parent!.quaternion).multiply(knee.quaternion)
    expect(Math.abs(worldRotation(knee, new Quaternion()).dot(want))).toBeCloseTo(1, 5)
  })
})

describe('Inertia', () => {
  it('eases a bone out of a snap instead of jumping, and settles on the new pose', () => {
    const bone = new Group()
    const inertia = new Inertia([bone])
    const before = new Quaternion()
    for (let i = 0; i < 5; i++) {
      bone.quaternion.copy(before)
      inertia.apply(1 / 60)
    }
    const after = new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), Math.PI / 2)
    const angle = (q: { x: number; y: number; z: number; w: number }, to: Quaternion) => 2 * Math.acos(Math.min(1, Math.abs(q.x * to.x + q.y * to.y + q.z * to.z + q.w * to.w)))
    bone.quaternion.copy(after)
    inertia.apply(1 / 60)
    expect(inertia.easing).toBe(true)
    // The first frame after the snap still shows almost the old pose.
    expect(angle(bone.quaternion, before)).toBeLessThan(0.05)
    let last = angle(bone.quaternion, after)
    for (let i = 0; i < 30; i++) {
      bone.quaternion.copy(after)
      inertia.apply(1 / 60)
      const now = angle(bone.quaternion, after)
      expect(now).toBeLessThanOrEqual(last + 1e-6)
      last = now
    }
    expect(last).toBeLessThan(0.02)
  })

  it('leaves a pose that moves smoothly alone', () => {
    const bone = new Group()
    const inertia = new Inertia([bone])
    for (let i = 0; i < 60; i++) {
      const q = new Quaternion().setFromAxisAngle(new Vector3(1, 0, 0), Math.sin(i / 10))
      bone.quaternion.copy(q)
      inertia.apply(1 / 60)
      expect(Math.abs(bone.quaternion.dot(q))).toBeCloseTo(1, 6)
    }
    expect(inertia.easing).toBe(false)
  })

  it('does not ease from a pose it was told to forget', () => {
    const bone = new Group()
    const inertia = new Inertia([bone])
    inertia.apply(1 / 60)
    inertia.forget()
    const q = new Quaternion().setFromAxisAngle(new Vector3(0, 0, 1), 2)
    bone.quaternion.copy(q)
    inertia.apply(1 / 60)
    expect(Math.abs(bone.quaternion.dot(q))).toBeCloseTo(1, 6)
  })
})

describe('Follow', () => {
  it('closes half the gap in its half-life and settles without overshooting', () => {
    const follow = new Follow(0.1)
    let max = 0
    let at = 0
    for (let t = 0; t < 2; t += 1 / 120) {
      at = follow.step(1, 1 / 120)
      max = Math.max(max, at)
      if (Math.abs(t - 0.1) < 1 / 240) expect(at).toBeGreaterThan(0.3)
    }
    expect(max).toBeLessThanOrEqual(1 + 1e-9)
    expect(at).toBeCloseTo(1, 4)
  })
})
