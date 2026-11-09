import { describe, expect, it } from 'vitest'
import { serverOpenings, type Tri } from '@/sim/carve'
import { openingsOf } from '@/render/windows'
import type { CollisionMesh } from '@/world'

/*
 * The server places windows with its own port of `openingsOf` (`shared::windows`), and cuts them out
 * of the walls it collides with: the two must agree to the millimetre, or a round would fly through a
 * pane the client draws shut, or stop on glass that is drawn open.
 */

function face(corners: [number, number, number][], outward: [number, number, number]): Tri[] {
  const [a, b, c] = corners
  const n = [(b![1] - a![1]) * (c![2] - a![2]) - (b![2] - a![2]) * (c![1] - a![1]), (b![2] - a![2]) * (c![0] - a![0]) - (b![0] - a![0]) * (c![2] - a![2]), (b![0] - a![0]) * (c![1] - a![1]) - (b![1] - a![1]) * (c![0] - a![0])]
  const ring = n[0]! * outward[0] + n[1]! * outward[1] + n[2]! * outward[2] > 0 ? [...corners].reverse() : corners
  const out: Tri[] = []
  for (let i = 1; i + 1 < ring.length; i++) out.push([...ring[0]!, ...ring[i]!, ...ring[i + 1]!] as Tri)
  return out
}

/** A gabled house 6 × 4 m with 3 m walls and a ridge 4.5 m up, at an odd offset so nothing lands on a round number. */
function gable(): Tri[] {
  const [W, D, H, R, X, Z] = [600, 400, 300, 450, 137, -2231]
  const p = (x: number, y: number, z: number): [number, number, number] => [X + x, y, Z + z]
  return [
    ...face([p(0, 0, 0), p(W, 0, 0), p(W, H, 0), p(0, H, 0)], [0, 0, -1]),
    ...face([p(0, 0, D), p(W, 0, D), p(W, H, D), p(0, H, D)], [0, 0, 1]),
    ...face([p(0, 0, 0), p(0, 0, D), p(0, H, D), p(0, R, D / 2), p(0, H, 0)], [-1, 0, 0]),
    ...face([p(W, 0, 0), p(W, 0, D), p(W, H, D), p(W, R, D / 2), p(W, H, 0)], [1, 0, 0]),
  ]
}

/** A long low shed 11.3 × 3.7 m turned 25°, so the walls are not along the axes. */
function turned(): Tri[] {
  const [W, D, H] = [1130, 370, 290]
  const [c, s] = [Math.cos(0.436), Math.sin(0.436)]
  const p = (x: number, y: number, z: number): [number, number, number] => [Math.round(x * c - z * s), y, Math.round(x * s + z * c)]
  return [
    ...face([p(0, 0, 0), p(W, 0, 0), p(W, H, 0), p(0, H, 0)], [-s, 0, -c]),
    ...face([p(W, 0, 0), p(W, 0, D), p(W, H, D), p(W, H, 0)], [c, 0, s]),
    ...face([p(W, 0, D), p(0, 0, D), p(0, H, D), p(W, H, D)], [s, 0, c]),
    ...face([p(0, 0, D), p(0, 0, 0), p(0, H, 0), p(0, H, D)], [-c, 0, -s]),
  ]
}

function lookOf(tris: Tri[]): CollisionMesh {
  const vertices = new Float32Array(tris.length * 9)
  tris.forEach((t, i) => t.forEach((v, k) => (vertices[i * 9 + k] = v / 100)))
  return { vertices, triangles: Uint32Array.from(tris.flatMap((_, i) => [i * 3, i * 3 + 1, i * 3 + 2])) }
}

describe('window placement', () => {
  it('is the same on the server as on the client, house for house', () => {
    for (const walls of [gable(), turned()]) {
      const ours = openingsOf(lookOf(walls), { first: 0, walls: walls.length, roof: 0 })
      const theirs = serverOpenings(walls)
      expect(theirs.length).toBe(ours.length)
      expect(ours.length).toBeGreaterThan(3)
      ours.forEach((o, i) => {
        const t = theirs[i]!
        expect(t.door).toBe(o.door)
        for (let k = 0; k < 4; k++) for (let a = 0; a < 3; a++) expect(t.corners[k]![a]).toBeCloseTo(o.corners[k]![a]!, 4)
        for (let a = 0; a < 3; a++) expect(t.normal[a]).toBeCloseTo(o.normal[a]!, 5)
      })
    }
  })
})
