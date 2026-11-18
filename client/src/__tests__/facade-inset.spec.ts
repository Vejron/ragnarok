import { describe, expect, it } from 'vitest'
import { WALL_THICKNESS, innerOf, insetOf } from '@/render/facade'
import { dot, keyOf, normalOf, type P, type Poly } from '@/render/geom'

/** The walls, 3 m tall, of a house standing on `plan` (`[x, z]` corners, metres), each split in two halfway up and wound outwards. */
function wallsOf(plan: [number, number][]): Poly[] {
  const walls: Poly[] = []
  for (let i = 0; i < plan.length; i++) {
    const [a, b] = [plan[i]!, plan[(i + 1) % plan.length]!]
    for (const [lo, hi] of [[0, 1.4], [1.4, 3]] as const) {
      let quad: Poly = [[a[0], lo, a[1]], [b[0], lo, b[1]], [b[0], hi, b[1]], [a[0], hi, a[1]]]
      // Outwards: away from the inside, which lies to the left of a counter-clockwise plan walked from a to b.
      const out: P = [b[1] - a[1], 0, -(b[0] - a[0])]
      if (dot(normalOf(quad), out) < 0) quad = [...quad].reverse()
      walls.push(quad)
    }
  }
  return walls
}

/** Every edge of `faces` with how many faces have it. */
function edges(faces: Poly[]): Map<string, number> {
  const count = new Map<string, number>()
  for (const face of faces) {
    for (let i = 0; i < face.length; i++) {
      const [a, b] = [keyOf(face[i]!), keyOf(face[(i + 1) % face.length]!)]
      const key = a < b ? `${a}|${b}` : `${b}|${a}`
      count.set(key, (count.get(key) ?? 0) + 1)
    }
  }
  return count
}

describe('the inner faces of a house', () => {
  // An L: a 10 × 6 m block and a 4 × 4 m wing out of its far side, counter-clockwise seen from above (x right, z towards you).
  const plan: [number, number][] = [[0, 0], [10, 0], [10, 6], [6, 6], [6, 10], [2, 10], [2, 6], [0, 6]].map(([x, z]) => [x!, -z!])

  it('meet edge to edge at every corner of the walls, inner and outer, leaving no slit', () => {
    const walls = wallsOf(plan)
    const inset = insetOf(walls)
    const inner = walls.map((wall) => innerOf(wall, normalOf(wall), inset))
    // Every upright edge is shared by two inner faces; only the top and bottom of the walls are open.
    for (const [key, n] of edges(inner)) {
      const [a, b] = key.split('|').map((k) => k.split(',').map(Number))
      if (a![1] === b![1]) continue
      expect(n, `edge ${key}`).toBe(2)
    }
    // Moved straight in, each face on its own, the inner faces part at the L's inner corners and overlap at its outer ones.
    const straight = walls.map((wall) => innerOf(wall, normalOf(wall)))
    expect([...edges(straight).values()].filter((n) => n === 1).length).toBeGreaterThan(0)
  })

  it('stand a wall thickness in from every wall', () => {
    const walls = wallsOf(plan)
    const inset = insetOf(walls)
    for (const wall of walls) {
      const n = normalOf(wall)
      for (const p of innerOf(wall, n, inset)) {
        // On the plane a wall thickness behind the wall.
        expect(dot(n, p) - dot(n, wall[0]!)).toBeCloseTo(-WALL_THICKNESS, 6)
      }
    }
    // An outer corner moves in diagonally, a wall thickness off both walls.
    const corner = inset.get(keyOf([10, 0, 0]))!
    expect(Math.abs(corner[0])).toBeCloseTo(WALL_THICKNESS, 6)
    expect(Math.abs(corner[2])).toBeCloseTo(WALL_THICKNESS, 6)
  })
})
