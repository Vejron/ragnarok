import { describe, expect, it } from 'vitest'
import { LID_HEIGHT, lidGeometry } from '@/render/croplid'
import { CROP_NONE, CROP_WHEAT } from '@/world'

describe('the lid over a far field', () => {
  // A 16 m tile of 8 × 8 crop cells of 2 m.
  const field = (grown: (i: number, j: number) => boolean) => Uint8Array.from({ length: 64 }, (_, k) => (grown(k % 8, Math.floor(k / 8)) ? CROP_WHEAT : CROP_NONE))
  const flat = () => 5
  type Lid = NonNullable<ReturnType<typeof lidGeometry>>
  const lifted = (lid: Lid, v: number) => lid.custom![v * 4]! > 0
  const at = (lid: Lid, v: number) => [lid.positions[v * 3]!, lid.positions[v * 3 + 2]!] as const
  /** The area of the lid's top, square metres: its triangles whose corners are all lifted. */
  const topArea = (lid: Lid) => {
    let area = 0
    const index = lid.indices!
    for (let t = 0; t < index.length; t += 3) {
      const [a, b, c] = [index[t]!, index[t + 1]!, index[t + 2]!]
      if (!lifted(lid, a) || !lifted(lid, b) || !lifted(lid, c)) continue
      const [ax, az] = at(lid, a), [bx, bz] = at(lid, b), [cx, cz] = at(lid, c)
      area += Math.abs((bx - ax) * (cz - az) - (cx - ax) * (bz - az)) / 2
    }
    return area
  }
  /** The skirts' feet that are not on the tile's own edge: the field's outline. */
  const outline = (lid: Lid) => {
    const feet: (readonly [number, number])[] = []
    for (let v = 0; v < lid.vertexCount; v++) {
      const [x, z] = at(lid, v)
      if (!lifted(lid, v) && x > 0 && x < 16 && z > 0 && z < 16) feet.push([x, z])
    }
    return feet
  }

  it('covers the sown ground to the tile\'s edge, every vertex on the ground and the top lifted by the shader', () => {
    const lid = lidGeometry(field(() => true), 100, 200, 16, flat)!
    expect(topArea(lid)).toBeCloseTo(16 * 16)
    const lifts = new Set<number>()
    for (let v = 0; v < lid.vertexCount; v++) {
      expect(lid.positions[v * 3 + 1]).toBe(5)
      lifts.add(lid.custom![v * 4]!)
    }
    expect([...lifts].sort()).toEqual([0, Math.fround(LID_HEIGHT)])
    // A skirt all round the tile's edge and nowhere inside it.
    expect(outline(lidGeometry(field(() => true), 0, 0, 16, flat)!)).toEqual([])
  })

  it('ends where the crop does, along a straight edge', () => {
    // Wheat over the western 8 m.
    const lid = lidGeometry(field((i) => i < 4), 0, 0, 16, flat)!
    expect(topArea(lid)).toBeCloseTo(8 * 16)
    const feet = outline(lid)
    expect(feet.length).toBeGreaterThan(4)
    for (const [x] of feet) expect(x).toBeCloseTo(8)
  })

  it('follows a slanting edge as a line, not in the steps of its cells', () => {
    // Wheat north-west of the diagonal from (16, 0) to (0, 16).
    const lid = lidGeometry(field((i, j) => i + j < 7), 0, 0, 16, flat)!
    const feet = outline(lid)
    expect(feet.length).toBeGreaterThan(8)
    // The cells' own outline strays a metre and a half either side of a line; this keeps within half a metre of one.
    const off = feet.map(([x, z]) => (x + z) / Math.SQRT2)
    const middle = off.reduce((sum, d) => sum + d, 0) / off.length
    for (const d of off) expect(Math.abs(d - middle)).toBeLessThan(0.5)
    expect(topArea(lid)).toBeGreaterThan(90)
    expect(topArea(lid)).toBeLessThan(125)
  })

  it('is open over a crater, to where the stalks stop round its bowl', () => {
    const whole = lidGeometry(field(() => true), 0, 0, 16, flat)!
    const holed = lidGeometry(field(() => true), 0, 0, 16, flat, [{ x: 8, z: 8, radius: 5 }])!
    // Bare to 0.8 of the radius: a hole of about 50 m², with a skirt round it some 4 m from the middle.
    const hole = topArea(whole) - topArea(holed)
    expect(hole).toBeGreaterThan(35)
    expect(hole).toBeLessThan(65)
    const rim = outline(holed)
    expect(rim.length).toBeGreaterThan(8)
    for (const [x, z] of rim) {
      expect(Math.hypot(x - 8, z - 8)).toBeGreaterThan(3)
      expect(Math.hypot(x - 8, z - 8)).toBeLessThan(5)
    }
    // A crater outside the field changes nothing.
    expect(topArea(lidGeometry(field(() => true), 0, 0, 16, flat, [{ x: 60, z: 8, radius: 5 }])!)).toBeCloseTo(topArea(whole))
  })

  it('is nothing over a tile with no crops, or whose ground is not there', () => {
    expect(lidGeometry(field(() => false), 0, 0, 16, flat)).toBeNull()
    expect(lidGeometry(field(() => true), 0, 0, 16, () => null)).toBeNull()
  })
})
