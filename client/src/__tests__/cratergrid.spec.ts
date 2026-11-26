import { describe, expect, it } from 'vitest'
import { CraterGrid } from '@/render/cratergrid'

/** A stream of numbers in 0..1 that is the same every run. */
function random(seed: number) {
  return () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296)
}

describe('CraterGrid', () => {
  it('answers as a scan of every crater would, with and without a margin', () => {
    const next = random(7)
    const grid = new CraterGrid()
    const all: { x: number; z: number; radius: number }[] = []
    for (let i = 0; i < 400; i++) {
      const bowl = { x: (next() - 0.5) * 600, z: (next() - 0.5) * 600, radius: 1 + next() * 13 }
      all.push(bowl)
      grid.add(bowl.x, bowl.z, bowl.radius)
    }
    for (let i = 0; i < 4000; i++) {
      const [x, z] = [(next() - 0.5) * 640, (next() - 0.5) * 640]
      const margin = i % 2 === 0 ? 0 : next() * 20
      expect(grid.covers(x, z, margin)).toBe(all.some((c) => Math.hypot(x - c.x, z - c.z) < c.radius + margin))
    }
  })

  it('takes a crater told of twice once, and forgets one that healed', () => {
    const grid = new CraterGrid()
    expect(grid.add(10, -20, 5)).toBe(true)
    expect(grid.add(10, -20, 5)).toBe(false)
    expect(grid.size).toBe(1)
    expect(grid.covers(12, -19)).toBe(true)
    expect(grid.remove(10, -20, 5)).toBe(true)
    expect(grid.remove(10, -20, 5)).toBe(false)
    expect(grid.covers(12, -19)).toBe(false)
    expect(grid.size).toBe(0)
  })

  it('lists the craters reaching a rectangle, each once', () => {
    const next = random(3)
    const grid = new CraterGrid()
    const all: { x: number; z: number; radius: number }[] = []
    for (let i = 0; i < 300; i++) {
      const bowl = { x: (next() - 0.5) * 500, z: (next() - 0.5) * 500, radius: 1 + next() * 13 }
      all.push(bowl)
      grid.add(bowl.x, bowl.z, bowl.radius)
    }
    const [x0, z0, x1, z1] = [-64, 32, 64, 160]
    const expected = all.filter((c) => c.x + c.radius >= x0 && c.x - c.radius <= x1 && c.z + c.radius >= z0 && c.z - c.radius <= z1)
    const found = grid.within(x0, z0, x1, z1)
    expect(found.length).toBe(expected.length)
    expect(new Set(found.map((c) => `${c.x},${c.z}`))).toEqual(new Set(expected.map((c) => `${c.x},${c.z}`)))
  })
})
