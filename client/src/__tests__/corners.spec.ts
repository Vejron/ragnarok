import { describe, expect, it } from 'vitest'
import { WallRuns, endsAt } from '@/render/corners'
import type { P, Poly } from '@/render/geom'

/** An upright face of the wall from `(x0, z0)` to `(x1, z1)`, from `y0` up to `y1`, metres. */
function face(x0: number, z0: number, x1: number, z1: number, y0 = 0, y1 = 3): Poly {
  return [[x0, y0, z0], [x1, y0, z1], [x1, y1, z1], [x0, y1, z0]]
}

/** The normal of a wall running from `(x0, z0)` to `(x1, z1)`, and a point on it. */
function on(x0: number, z0: number, x1: number, z1: number, along = 0.5): [P, P] {
  const length = Math.hypot(x1 - x0, z1 - z0)
  return [[(z0 - z1) / length, 0, (x1 - x0) / length], [x0 + (x1 - x0) * along, 1, z0 + (z1 - z0) * along]]
}

describe('the runs of wall a building\'s faces make up', () => {
  it('makes one run of a wall its windows cut into faces, and measures a point from both its ends', () => {
    // A wall 8 m long with a window from 3 to 5 m: the faces left of it, right of it, under and over it.
    const runs = new WallRuns([face(0, 0, 3, 0), face(5, 0, 8, 0), face(3, 0, 5, 0, 0, 1), face(3, 0, 5, 0, 2, 3), face(8, 0, 8, 6), face(0, 6, 0, 0)])
    const run = runs.of(...on(3, 0, 5, 0))!
    expect(run.to - run.from).toBeCloseTo(8, 6)
    const ends = endsAt(run, 2, 0)
    expect(Math.min(...ends)).toBeCloseTo(2, 6)
    expect(Math.max(...ends)).toBeCloseTo(6, 6)
    // Every face of the wall is part of the same run.
    expect(runs.of(...on(0, 0, 3, 0))).toBe(run)
    expect(runs.of(...on(5, 0, 8, 0))).toBe(run)
    // The wall round the corner is another.
    const side = runs.of(...on(8, 0, 8, 6))!
    expect(side).not.toBe(run)
    expect(side.to - side.from).toBeCloseTo(6, 6)
  })

  it('ends a run where a wing stands out of the wall, though the wall goes on in the same plane past it', () => {
    const runs = new WallRuns([face(0, 0, 4, 0), face(7, 0, 12, 0)])
    const [left, right] = [runs.of(...on(0, 0, 4, 0))!, runs.of(...on(7, 0, 12, 0))!]
    expect(left).not.toBe(right)
    expect(left.to - left.from).toBeCloseTo(4, 6)
    expect(right.to - right.from).toBeCloseTo(5, 6)
  })

  it('finds no corner where a wall only bends a little, as round a silo', () => {
    // Three facets turning 15° each, then a square corner.
    const turn = (angle: number): [number, number] => [Math.cos((angle * Math.PI) / 180), Math.sin((angle * Math.PI) / 180)]
    const points: [number, number][] = [[0, 0]]
    for (const angle of [0, 15, 30]) {
      const [x, z] = points[points.length - 1]!
      const [dx, dz] = turn(angle)
      points.push([x + dx * 2, z + dz * 2])
    }
    const [x, z] = points[3]!
    const [dx, dz] = turn(120)
    points.push([x + dx * 2, z + dz * 2])
    const walls = points.slice(1).map((p, i) => face(points[i]![0], points[i]![1], p[0], p[1]))
    const runs = new WallRuns(walls)
    const at = (i: number) => on(points[i]![0], points[i]![1], points[i + 1]![0], points[i + 1]![1])
    // The middle facet bends at both ends: it is part of no run with a corner.
    expect(runs.of(...at(1))).toBeNull()
    // The first has its free end, the third the square corner: a point in the middle of each is 1 m from that end and far from the other.
    for (const i of [0, 2]) {
      const [normal, p] = at(i)
      const ends = endsAt(runs.of(normal, p)!, p[0], p[2])
      expect(Math.min(...ends)).toBeCloseTo(1, 4)
      expect(Math.max(...ends)).toBeGreaterThan(500)
    }
  })

  it('takes a roof or a floor for no wall', () => {
    const runs = new WallRuns([[[0, 3, 0], [4, 3, 0], [4, 3, 4], [0, 3, 4]]])
    expect(runs.of([0, 1, 0], [1, 3, 1])).toBeNull()
  })
})
