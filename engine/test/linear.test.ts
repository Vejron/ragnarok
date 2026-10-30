import { describe, expect, it } from 'vitest'
import { Matrix4, Quaternion, Vector3 } from '../src/math/linear'
import { Node } from '../src/scene/node'

const near = (a: ArrayLike<number>, b: ArrayLike<number>, digits = 9) => Array.from(a).forEach((v, i) => expect(v).toBeCloseTo(b[i]!, digits))
const xyz = (v: { x: number; y: number; z: number }) => [v.x, v.y, v.z]
const quarterY = () => new Quaternion().setFromAxisAngle({ x: 0, y: 1, z: 0 }, Math.PI / 2)

describe('Vector3', () => {
  it('adds, scales and measures', () => {
    const v = new Vector3(1, 2, 3).addScaledVector({ x: 1, y: 0, z: -1 }, 2).sub({ x: 3, y: 2, z: 1 })
    expect(xyz(v)).toEqual([0, 0, 0])
    expect(new Vector3(3, 4, 0).length()).toBe(5)
    expect(new Vector3(3, 4, 0).distanceToSquared({ x: 0, y: 0, z: 0 })).toBe(25)
    expect(xyz(new Vector3().normalize())).toEqual([0, 0, 0])
  })

  it('crosses right-handed, also with itself as an argument', () => {
    expect(xyz(new Vector3(1, 0, 0).cross({ x: 0, y: 1, z: 0 }))).toEqual([0, 0, 1])
    const v = new Vector3(1, 2, 3)
    expect(xyz(v.cross(v))).toEqual([0, 0, 0])
  })

  it('turns by a quaternion: a quarter about Y takes +X to -Z', () => {
    near(xyz(new Vector3(1, 0, 0).applyQuaternion(quarterY())), [0, 0, -1])
  })

  it('goes through a matrix as a point and as a direction', () => {
    const m = new Matrix4().compose({ x: 10, y: 0, z: 0 }, quarterY(), { x: 2, y: 2, z: 2 })
    near(xyz(new Vector3(1, 0, 0).applyMatrix4(m)), [10, 0, -2])
    near(xyz(new Vector3(1, 0, 0).transformDirection(m)), [0, 0, -1])
  })

  it('keeps plain data fields, so nothing fires on a write', () => {
    expect(Object.keys(new Vector3())).toEqual(['x', 'y', 'z'])
    expect(Object.keys(new Quaternion())).toEqual(['x', 'y', 'z', 'w'])
  })
})

describe('Quaternion', () => {
  it('composes as a matrix product: multiply applies the argument first', () => {
    const a = quarterY(), b = new Quaternion().setFromAxisAngle({ x: 1, y: 0, z: 0 }, Math.PI / 2)
    const point = new Vector3(0, 1, 0)
    near(xyz(point.clone().applyQuaternion(a.clone().multiply(b))), xyz(point.clone().applyQuaternion(b).applyQuaternion(a)))
    near(xyz(point.clone().applyQuaternion(a.clone().premultiply(b))), xyz(point.clone().applyQuaternion(a).applyQuaternion(b)))
  })

  it('undoes itself when inverted', () => {
    near(xyz(new Vector3(1, 2, 3).applyQuaternion(quarterY()).applyQuaternion(quarterY().invert())), [1, 2, 3])
  })

  it('takes one unit vector to another, even its opposite', () => {
    const to = new Vector3(0, 0, 1)
    near(xyz(new Vector3(1, 0, 0).applyQuaternion(new Quaternion().setFromUnitVectors({ x: 1, y: 0, z: 0 }, to))), xyz(to))
    near(xyz(new Vector3(1, 0, 0).applyQuaternion(new Quaternion().setFromUnitVectors({ x: 1, y: 0, z: 0 }, { x: -1, y: 0, z: 0 }))), [-1, 0, 0])
  })

  it('slerps the short way, at an even rate, and lerps when nearly there', () => {
    near(xyz(new Vector3(1, 0, 0).applyQuaternion(new Quaternion().slerp(quarterY(), 0.5))), [Math.SQRT1_2, 0, -Math.SQRT1_2])
    const flipped = quarterY()
    flipped.set(-flipped.x, -flipped.y, -flipped.z, -flipped.w)
    near(xyz(new Vector3(1, 0, 0).applyQuaternion(new Quaternion().slerp(flipped, 0.5))), [Math.SQRT1_2, 0, -Math.SQRT1_2])
    const close = new Quaternion().setFromAxisAngle({ x: 0, y: 1, z: 0 }, 0.001)
    expect(new Quaternion().slerp(close, 0.5).dot(new Quaternion().slerp(close, 0.5))).toBeCloseTo(1, 12)
  })
})

describe('Matrix4', () => {
  const trs = () => new Matrix4().compose({ x: 1, y: 2, z: 3 }, quarterY(), { x: 2, y: 3, z: 4 })

  it('is the same matrix a node composes', () => {
    const node = new Node()
    node.position.set(1, 2, 3)
    node.quaternion.copy(quarterY())
    node.scale.set(2, 3, 4)
    node.updateWorld()
    near(trs().elements, node.matrixWorld, 5)
  })

  it('takes itself apart again', () => {
    const [at, turn, size] = [new Vector3(), new Quaternion(), new Vector3()]
    trs().decompose(at, turn, size)
    near(xyz(at), [1, 2, 3])
    near(xyz(size), [2, 3, 4])
    expect(Math.abs(turn.dot(quarterY()))).toBeCloseTo(1, 9)
  })

  it('finds a mirrored scale in a negative determinant', () => {
    const [at, turn, size] = [new Vector3(), new Quaternion(), new Vector3()]
    new Matrix4().compose({ x: 0, y: 0, z: 0 }, new Quaternion(), { x: -2, y: 1, z: 1 }).decompose(at, turn, size)
    near(xyz(size), [-2, 1, 1])
  })

  it('inverts, and multiplies back to the identity', () => {
    near(trs().multiply(trs().invert()).elements, new Matrix4().elements)
    near(trs().premultiply(trs().invert()).elements, new Matrix4().elements)
    expect(trs().determinant()).toBeCloseTo(24, 9)
  })

  it('is all zeros inverted when it has no inverse', () => {
    const singular = new Matrix4()
    singular.elements[0] = 0
    expect(singular.invert().elements.every((v) => v === 0)).toBe(true)
  })

  it('multiplies safely into either of its operands', () => {
    const expected = new Matrix4().multiplyMatrices(trs(), trs())
    expect(trs().multiply(trs()).elements).toEqual(expected.elements)
  })
})
