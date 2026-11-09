import { describe, expect, it } from 'vitest'
import { Shell, type Tri } from '@/sim/carve'
import { lookChunk, lookFracture } from '@/render/lookfracture'
import { EAVE, FASCIA, VERGE, eaveTriangles, eavesOf } from '@/render/eaves'
import { buildingBuffers, Surface } from '@/render/buildingmesh'
import { openingTriangles, openingsOf } from '@/render/windows'
import { facadeOf } from '@/render/facade'
import { ROOF_GABLE } from '@/world'
import type { CollisionMesh } from '@/world'

type P3 = [number, number, number]

/** A face of a house, centimetre corners in order round it, wound inwards as the importer winds them (`outward` says which way is out). */
function face(corners: P3[], outward: P3): Tri[] {
  const [a, b, c] = corners
  const n = [(b![1] - a![1]) * (c![2] - a![2]) - (b![2] - a![2]) * (c![1] - a![1]), (b![2] - a![2]) * (c![0] - a![0]) - (b![0] - a![0]) * (c![2] - a![2]), (b![0] - a![0]) * (c![1] - a![1]) - (b![1] - a![1]) * (c![0] - a![0])]
  const ring = n[0]! * outward[0] + n[1]! * outward[1] + n[2]! * outward[2] > 0 ? [...corners].reverse() : corners
  const out: Tri[] = []
  for (let i = 1; i + 1 < ring.length; i++) out.push([...ring[0]!, ...ring[i]!, ...ring[i + 1]!] as Tri)
  return out
}

/** A gabled house 6 × 4 m, walls 3 m, the ridge along x at z = 2 m, 4.5 m up: eaves along z = 0 and z = 4, verges on the gables at x = 0 and x = 6. */
function gable(): { walls: Tri[]; roof: Tri[] } {
  const [W, D, H, R] = [600, 400, 300, 450]
  return {
    walls: [
      ...face([[0, 0, 0], [W, 0, 0], [W, H, 0], [0, H, 0]], [0, 0, -1]),
      ...face([[0, 0, D], [W, 0, D], [W, H, D], [0, H, D]], [0, 0, 1]),
      ...face([[0, 0, 0], [0, 0, D], [0, H, D], [0, R, D / 2], [0, H, 0]], [-1, 0, 0]),
      ...face([[W, 0, 0], [W, 0, D], [W, H, D], [W, R, D / 2], [W, H, 0]], [1, 0, 0]),
    ],
    roof: [
      ...face([[0, H, 0], [W, H, 0], [W, R, D / 2], [0, R, D / 2]], [0, 1, -1]),
      ...face([[0, R, D / 2], [W, R, D / 2], [W, H, D], [0, H, D]], [0, 1, 1]),
    ],
  }
}

/**
 * Two parts side by side with shed roofs rising towards z = 4 m: the low part (x 0..4) from 3 to 4 m,
 * the high part (x 4..8) from 5 to 6 m, with a step wall between them at x = 4 m.
 */
function stepped(): { walls: Tri[]; roof: Tri[] } {
  const D = 400
  const low = (z: number) => 300 + z / 4
  const high = (z: number) => 500 + z / 4
  return {
    walls: [
      ...face([[0, 0, 0], [400, 0, 0], [400, low(0), 0], [0, low(0), 0]], [0, 0, -1]),
      ...face([[0, 0, D], [400, 0, D], [400, low(D), D], [0, low(D), D]], [0, 0, 1]),
      ...face([[0, 0, 0], [0, 0, D], [0, low(D), D], [0, low(0), 0]], [-1, 0, 0]),
      ...face([[400, 0, 0], [800, 0, 0], [800, high(0), 0], [400, high(0), 0]], [0, 0, -1]),
      ...face([[400, 0, D], [800, 0, D], [800, high(D), D], [400, high(D), D]], [0, 0, 1]),
      ...face([[800, 0, 0], [800, 0, D], [800, high(D), D], [800, high(0), 0]], [1, 0, 0]),
      // The step, seen from either side.
      ...face([[400, low(0), 0], [400, low(D), D], [400, high(D), D], [400, high(0), 0]], [-1, 0, 0]),
      ...face([[400, low(0), 0], [400, low(D), D], [400, high(D), D], [400, high(0), 0]], [1, 0, 0]),
    ],
    roof: [...face([[0, low(0), 0], [400, low(0), 0], [400, low(D), D], [0, low(D), D]], [0, 1, 0]), ...face([[400, high(0), 0], [800, high(0), 0], [800, high(D), D], [400, high(D), D]], [0, 1, 0])],
  }
}

/** A flat-roofed box 5 × 4 × 3 m. */
function box(): { walls: Tri[]; roof: Tri[] } {
  const [W, D, H] = [500, 400, 300]
  return {
    walls: [
      ...face([[0, 0, 0], [W, 0, 0], [W, H, 0], [0, H, 0]], [0, 0, -1]),
      ...face([[W, 0, 0], [W, 0, D], [W, H, D], [W, H, 0]], [1, 0, 0]),
      ...face([[W, 0, D], [0, 0, D], [0, H, D], [W, H, D]], [0, 0, 1]),
      ...face([[0, 0, D], [0, 0, 0], [0, H, 0], [0, H, D]], [-1, 0, 0]),
    ],
    roof: face([[0, H, 0], [W, H, 0], [W, H, D], [0, H, D]], [0, 1, 0]),
  }
}

function lookOf(tris: Tri[]): CollisionMesh {
  const vertices = new Float32Array(tris.length * 9)
  tris.forEach((t, i) => t.forEach((v, k) => (vertices[i * 9 + k] = v / 100)))
  return { vertices, triangles: Uint32Array.from(tris.flatMap((_, i) => [i * 3, i * 3 + 1, i * 3 + 2])) }
}

const rangeOf = ({ walls, roof }: { walls: Tri[]; roof: Tri[] }) => ({ first: 0, walls: walls.length, roof: roof.length })

describe('eavesOf', () => {
  const house = gable()
  const strips = eavesOf(lookOf([...house.walls, ...house.roof]), rangeOf(house))

  it('hangs an overhang off every wall of a gabled house: level eaves along the long walls, sloping verges on the gables', () => {
    expect(strips.length).toBeGreaterThan(0)
    const outs = new Set(strips.map((s) => `${Math.round(s.out[0])},${Math.round(s.out[2])}`))
    expect([...outs].sort()).toEqual(['-1,0', '0,-1', '0,1', '1,0'])
    for (const s of strips) {
      const eave = Math.abs(s.out[2]) > 0.5
      if (eave) expect(Math.abs(s.a[1] - s.b[1])).toBeLessThan(1e-6)
      else expect(Math.abs(s.a[1] - s.b[1])).toBeGreaterThan(0.01)
    }
  })

  it('reaches the eave and verge distances past the walls, continuing the roof\'s slope', () => {
    // The front eave: its outer edge is EAVE out and, with the roof rising 1.5 m over 2 m, 0.3 m lower.
    const front = strips.filter((s) => s.out[2] < -0.5)
    for (const s of front) for (const { corners, part } of s.quads) {
      if (part !== 'top') continue
      const outer = corners.filter((p) => p[2] < -1e-6)
      expect(outer).toHaveLength(2)
      for (const p of outer) {
        expect(p[2]).toBeCloseTo(-EAVE, 6)
        expect(p[1]).toBeCloseTo(3 - EAVE * 0.75, 6)
      }
    }
    // A verge: VERGE out past the gable, at the roof's own height there.
    const verge = strips.filter((s) => s.out[0] < -0.5)
    expect(verge.length).toBeGreaterThan(0)
    for (const s of verge) for (const { corners, part } of s.quads) {
      if (part !== 'top') continue
      for (const p of corners.filter((p) => p[0] < -1e-6)) {
        expect(p[0]).toBeCloseTo(-VERGE, 6)
        // On the roof's own plane: 3 m at the eaves, rising 0.75 a metre to the ridge at z = 2.
        expect(p[1]).toBeCloseTo(3 + 0.75 * Math.min(p[2], 4 - p[2]), 6)
      }
    }
  })

  it('shares one corner where two edges meet: the ridge\'s end, and the eave and verge at a corner of the house', () => {
    const corners = new Set<string>()
    for (const s of strips) for (const { corners: c, part } of s.quads) if (part === 'top') for (const p of c) corners.add(`${p[0].toFixed(4)},${p[1].toFixed(4)},${p[2].toFixed(4)}`)
    // The ridge end on the left gable goes straight out along the ridge.
    expect(corners.has(`${(-VERGE).toFixed(4)},${(4.5).toFixed(4)},${(2).toFixed(4)}`)).toBe(true)
    // The front left corner: out by the verge along x and the eave along z, down the slope accordingly, and no other point out there.
    expect(corners.has(`${(-VERGE).toFixed(4)},${(3 - EAVE * 0.75).toFixed(4)},${(-EAVE).toFixed(4)}`)).toBe(true)
    expect([...corners].filter((k) => k.startsWith(`${(-VERGE).toFixed(4)},`) && k.endsWith(`,${(-EAVE).toFixed(4)}`))).toHaveLength(1)
  })

  it('gives the overhang a fascia down its outer edge and a soffit under it', () => {
    for (const s of strips) {
      const parts = s.quads.map((q) => q.part)
      expect(parts).toContain('fascia')
      expect(parts).toContain('soffit')
      const fascia = s.quads.find((q) => q.part === 'fascia')!
      const ys = fascia.corners.map((p) => p[1])
      expect(Math.max(...ys) - Math.min(...ys)).toBeGreaterThanOrEqual(FASCIA - 1e-6)
      const soffit = s.quads.find((q) => q.part === 'soffit')!
      // The soffit's inner edge is in the wall's plane.
      expect(soffit.corners.filter((p) => Math.abs(p[0] * s.out[0] + p[2] * s.out[2] - (s.a[0] * s.out[0] + s.a[2] * s.out[2])) < 1e-6)).toHaveLength(2)
    }
    // No cap on a closed outline.
    expect(strips.some((s) => s.quads.some((q) => q.part === 'cap'))).toBe(false)
  })

  it('leaves a flat roof flush with its walls', () => {
    const flat = box()
    expect(eavesOf(lookOf([...flat.walls, ...flat.roof]), rangeOf(flat))).toEqual([])
  })

  it('does not hang a lower roof over the step up to a higher one, but hangs the higher one over it', () => {
    const parts = stepped()
    const out = eavesOf(lookOf([...parts.walls, ...parts.roof]), rangeOf(parts))
    const atStep = out.filter((s) => Math.abs(s.a[0] - 4) < 1e-6 && Math.abs(s.b[0] - 4) < 1e-6)
    expect(atStep.length).toBeGreaterThan(0)
    for (const s of atStep) {
      expect(s.a[1]).toBeGreaterThan(4.9)
      expect(s.out[0]).toBeLessThan(-0.5)
    }
    // The strips that end against the step are capped.
    expect(out.some((s) => s.quads.some((q) => q.part === 'cap'))).toBe(true)
  })

  it('is the same every time', () => {
    const again = eavesOf(lookOf([...house.walls, ...house.roof]), rangeOf(house))
    expect(JSON.stringify(again)).toBe(JSON.stringify(strips))
  })
})

describe('a tile\'s buildings with eaves', () => {
  it('draws the overhang with the roof\'s pattern on top and plain boards under and along it', () => {
    const house = gable()
    const look = lookOf([...house.walls, ...house.roof])
    const range = rangeOf(house)
    const tile = {
      buildings: [{ first: 0, walls: 0, roof: 0, roofKind: ROOF_GABLE, cells: null, paint: [0xa8432f, 0xb5543f] as [number, number], look: range }],
      bridges: [],
      collision: { vertices: new Float32Array(), triangles: new Uint32Array() },
      look,
    }
    const buffers = buildingBuffers(tile, 0, new Set())!
    const strips = eavesOf(look, range)
    const facade = facadeOf(look, range)
    const fan = (polys: readonly (readonly unknown[])[]) => polys.reduce((n, poly) => n + poly.length - 2, 0)
    expect(buffers.positions.length / 9).toBe(2 * (fan(facade.walls) + fan(facade.roof)) + fan(facade.floor) + openingTriangles(openingsOf(look, range)) + eaveTriangles(strips))
    // Every vertex outside the walls' box (above the frames) is the overhang: tiles on top, plain elsewhere.
    let top = 0
    let plain = 0
    for (let v = 0; v < buffers.positions.length / 3; v++) {
      const [x, y, z] = [buffers.positions[v * 3]!, buffers.positions[v * 3 + 1]!, buffers.positions[v * 3 + 2]!]
      if ((x >= -1e-6 && x <= 6 + 1e-6 && z >= -1e-6 && z <= 4 + 1e-6) || y < 2.6) continue
      const kind = buffers.custom[v * 4]!
      if (kind === Surface.Tiles) top++
      else {
        expect(kind).toBe(Surface.Plain)
        plain++
      }
      void y
    }
    expect(top).toBeGreaterThan(0)
    expect(plain).toBeGreaterThan(0)
  })
})

describe('a hurt house', () => {
  const house = gable()
  const shell = Shell.of(house.walls, house.roof)!
  const look = lookOf([...house.walls, ...house.roof])
  const range = rangeOf(house)
  const white = [1, 1, 1] as const
  /** Vertices of `positions` hanging out past the front wall (z < -0.3) with x in [x0, x1]. */
  const hanging = (positions: Float32Array, x0: number, x1: number) => {
    let n = 0
    for (let v = 0; v < positions.length; v += 3) if (positions[v + 2]! < -0.3 && positions[v]! >= x0 && positions[v]! <= x1) n++
    return n
  }

  it('loses the stretch of eave over a roof cell that is gone, and keeps the rest', () => {
    const whole = lookFracture(look, range, shell, new Set(), new Map(), white, white)
    const hole = shell.cellAt(3.0, 3.05, 0.15)
    expect(shell.isRoof(hole)).toBe(true)
    const hurt = lookFracture(look, range, shell, new Set([hole]), new Map(), white, white)
    expect(hanging(hurt.positions, 2.6, 3.4)).toBeLessThan(hanging(whole.positions, 2.6, 3.4))
    expect(hanging(hurt.positions, 0.2, 1.4)).toBe(hanging(whole.positions, 0.2, 1.4))
  })

  it('carries the eave along on a section of roof that falls', () => {
    const cells = new Set<number>()
    for (let id = 0; id < shell.dims[0] * shell.dims[1] * shell.dims[2]; id++) {
      const [x, y, z] = shell.centre(id).map((v) => v / 100)
      if (shell.isSolid(id) && shell.isRoof(id) && x! > 2 && x! < 4 && z! < 1 && y! > 2.5) cells.add(id)
    }
    expect(cells.size).toBeGreaterThan(0)
    const chunk = lookChunk(look, range, shell, cells, [0, 0, 0], white, white)
    expect(hanging(chunk.positions, 2, 4)).toBeGreaterThan(0)
  })
})
