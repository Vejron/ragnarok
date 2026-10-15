import { describe, expect, it } from 'vitest'
import { RangeAllocator } from '../src/gpu/allocator'
import { Frustum } from '../src/math/frustum'
import * as mat4 from '../src/math/mat4'
import { Quat } from '../src/math/vec'
import { Node } from '../src/scene/node'

const transform = (m: Float32Array, x: number, y: number, z: number) => {
  const w = m[3]! * x + m[7]! * y + m[11]! * z + m[15]!
  return [
    (m[0]! * x + m[4]! * y + m[8]! * z + m[12]!) / w,
    (m[1]! * x + m[5]! * y + m[9]! * z + m[13]!) / w,
    (m[2]! * x + m[6]! * y + m[10]! * z + m[14]!) / w,
  ]
}

describe('mat4', () => {
  it('inverts a composed transform', () => {
    const q = new Quat().setFromEuler(0.3, -1.1, 0.7)
    const m = mat4.compose(mat4.create(), 1, 2, 3, q.x, q.y, q.z, q.w, 2, 0.5, 3)
    const inv = mat4.invert(mat4.create(), m)!
    const id = mat4.multiply(mat4.create(), m, inv)
    for (let i = 0; i < 16; i++) expect(id[i]).toBeCloseTo(i % 5 === 0 ? 1 : 0, 5)
  })

  it('maps near to depth 1 and far to depth 0 with the reversed perspective', () => {
    const p = mat4.perspectiveReversed(mat4.create(), Math.PI / 2, 1, 0.5, 100)
    expect(transform(p, 0, 0, -0.5)[2]).toBeCloseTo(1, 5)
    expect(transform(p, 0, 0, -100)[2]).toBeCloseTo(0, 5)
  })

  it('maps near to 0 and far to 1 with the orthographic projection', () => {
    const o = mat4.orthographic(mat4.create(), -1, 1, -1, 1, 2, 10)
    expect(transform(o, 0, 0, -2)[2]).toBeCloseTo(0, 6)
    expect(transform(o, 0, 0, -10)[2]).toBeCloseTo(1, 6)
    expect(transform(o, 1, -1, -5).slice(0, 2)).toEqual([1, -1])
  })

  it('puts the target straight ahead with lookAt', () => {
    const v = mat4.lookAt(mat4.create(), 10, 5, 3, 0, 1, 0)
    const [x, y, z] = transform(v, 0, 1, 0)
    expect(x).toBeCloseTo(0, 5)
    expect(y).toBeCloseTo(0, 5)
    expect(z).toBeCloseTo(-Math.hypot(10, 4, 3), 5)
  })
})

describe('Node', () => {
  it('composes world matrices down the tree and only recomputes when moved', () => {
    const root = new Node()
    const child = new Node()
    root.add(child)
    root.position.set(10, 0, 0)
    child.position.set(0, 2, 0)
    root.updateWorld()
    expect(child.getWorldPosition({ x: 0, y: 0, z: 0 })).toEqual({ x: 10, y: 2, z: 0 })
    const version = child.worldVersion
    root.updateWorld()
    expect(child.worldVersion).toBe(version)
    root.position.x = 0
    root.updateWorld()
    expect(child.worldVersion).toBe(version + 1)
    expect(child.matrixWorld[12]).toBe(0)
  })

  it('clones a subtree with names', () => {
    const root = new Node()
    const arm = new Node()
    arm.name = 'arm'
    arm.position.set(1, 2, 3)
    root.add(arm)
    const copy = root.clone()
    const found = copy.getObjectByName('arm')!
    expect(found).not.toBe(arm)
    expect(found.position.y).toBe(2)
  })
})

describe('Frustum', () => {
  it('keeps what is in front of a camera and drops what is behind', () => {
    const proj = mat4.perspectiveReversed(mat4.create(), Math.PI / 3, 1, 0.1, 100)
    const f = new Frustum().setFromMatrix(proj)
    expect(f.intersectsSphere(0, 0, -10, 1)).toBe(true)
    expect(f.intersectsSphere(0, 0, 10, 1)).toBe(false)
    expect(f.intersectsSphere(0, 0, -200, 1)).toBe(false)
    expect(f.intersectsSphere(50, 0, -10, 1)).toBe(false)
  })
})

describe('RangeAllocator', () => {
  it('reuses and merges freed ranges', () => {
    const a = new RangeAllocator(100)
    const x = a.alloc(30)
    const y = a.alloc(30)
    const z = a.alloc(30)
    expect([x, y, z]).toEqual([0, 30, 60])
    expect(a.alloc(20)).toBe(-1)
    a.release(y, 30)
    a.release(x, 30)
    expect(a.alloc(60)).toBe(0)
    a.release(0, 60)
    a.release(z, 30)
    expect(a.used).toBe(0)
    expect(a.alloc(100)).toBe(0)
  })

  it('grows with the new space free', () => {
    const a = new RangeAllocator(10)
    a.alloc(10)
    a.grow(30)
    expect(a.alloc(20)).toBe(10)
  })
})
