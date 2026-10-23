import { describe, expect, it } from 'vitest'
import { Shell, type Tri } from '@/sim/carve'
import { voxelBrick, voxelFaces, type Soup } from '@/render/voxels'

const quad = (a: number[], b: number[], c: number[], e: number[]): Tri[] => [[...a, ...b, ...c] as Tri, [...a, ...c, ...e] as Tri]

/** Total area of a triangle soup, and its first-corner sum, to compare two meshings of the same surface. */
function area(soup: Soup): number {
  let sum = 0
  for (let t = 0; t < soup.positions.length; t += 9) {
    const p = soup.positions.slice(t, t + 9)
    const u = [p[3]! - p[0]!, p[4]! - p[1]!, p[5]! - p[2]!]
    const v = [p[6]! - p[0]!, p[7]! - p[1]!, p[8]! - p[2]!]
    sum += Math.hypot(u[1]! * v[2]! - u[2]! * v[1]!, u[2]! * v[0]! - u[0]! * v[2]!, u[0]! * v[1]! - u[1]! * v[0]!) / 2
  }
  return sum
}

describe('bricks', () => {
  it('draw the same surface as the whole building, with a hole in it', () => {
    const [w, d, h] = [3000, 1500, 800]
    const walls = [...quad([0, 0, 0], [w, 0, 0], [w, h, 0], [0, h, 0]), ...quad([w, 0, 0], [w, 0, d], [w, h, d], [w, h, 0]), ...quad([w, 0, d], [0, 0, d], [0, h, d], [w, h, d]), ...quad([0, 0, d], [0, 0, 0], [0, h, 0], [0, h, d])]
    const shell = Shell.of(walls, quad([0, h, 0], [w, h, 0], [w, h, d], [0, h, d]))!
    const removed = new Set<number>()
    for (let y = 4; y < 20; y++) for (let x = 10; x < 70; x++) removed.add(shell.index(x, y, 0))
    const standing = (id: number) => !removed.has(id)
    const whole: Soup = { positions: [], colors: [] }
    voxelFaces(shell, standing, () => 0x888888, whole)
    const bricks: Soup = { positions: [], colors: [] }
    const n = shell.dims.map((v) => Math.ceil(v / 16))
    for (let y = 0; y < n[1]!; y++) for (let z = 0; z < n[2]!; z++) for (let x = 0; x < n[0]!; x++) voxelBrick(shell, [x * 16, y * 16, z * 16], [x * 16 + 16, y * 16 + 16, z * 16 + 16], standing, () => 0x888888, bricks)
    expect(area(bricks)).toBeCloseTo(area(whole), 3)
    expect(bricks.positions.length).toBeGreaterThanOrEqual(whole.positions.length)
  })
})
