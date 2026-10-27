import { describe, expect, it } from 'vitest'
import { CRATER_BUCKETS, CRATER_CAPACITY, CRATER_CELL, CRATER_REACH, CRATER_SLOTS, Craters, craterBucket } from '../src/scene/craters'

/** The bucket a shader looking at ground point `(x, z)` reads. */
function bucketAt(x: number, z: number) {
  return craterBucket(Math.floor(x / CRATER_CELL), Math.floor(z / CRATER_CELL))
}

describe('Craters', () => {
  it('hashes cells as the shader does, negative cells included', () => {
    // u32 maths: (x * 73856093) ^ (z * 19349663), wrapped, masked.
    const wgsl = (cx: number, cz: number) => Number(((BigInt.asUintN(32, BigInt(cx) * 73856093n) ^ BigInt.asUintN(32, BigInt(cz) * 19349663n)) & BigInt(CRATER_BUCKETS - 1)))
    for (const [cx, cz] of [[0, 0], [1, -1], [-300, 41], [-1, -1], [9999, -12345]] as const) expect(craterBucket(cx, cz)).toBe(wgsl(cx, cz))
  })

  it('lists a crater in every cell its marks reach, and nowhere else', () => {
    const craters = new Craters()
    craters.add(-20, 37, 8, 1.6, 0)
    const reach = 8 * CRATER_REACH
    for (let z = 37 - reach; z <= 37 + reach; z += 2) {
      for (let x = -20 - reach; x <= -20 + reach; x += 2) expect(craters.listed(bucketAt(x, z))).toContain(0)
    }
    const listedIn = craters.grid.reduce((n, _, i) => (i % CRATER_SLOTS === 0 && craters.grid[i] ? n + 1 : n), 0)
    expect(listedIn).toBeLessThanOrEqual(9)
    const { slots, buckets } = craters.takeDirty()
    expect(slots).toEqual([0])
    expect(buckets.length).toBe(listedIn)
    expect(craters.takeDirty()).toEqual({ all: false, slots: [], buckets: [] })
  })

  it('keeps the newest craters of a full bucket', () => {
    const craters = new Craters()
    const n = CRATER_SLOTS + 5
    for (let i = 0; i < n; i++) craters.add(3, 3, 2, 1, i)
    const listed = craters.listed(bucketAt(3, 3))
    expect(listed.length).toBe(CRATER_SLOTS - 1)
    expect([...listed].sort((a, b) => a - b)).toEqual(Array.from({ length: CRATER_SLOTS - 1 }, (_, i) => n - (CRATER_SLOTS - 1) + i))
  })

  it('drops a crater pushed out of a full bucket from every cell it reaches, not only that one', () => {
    const craters = new Craters()
    // Wide enough to reach the cells round (8, 8), then that cell filled with small ones.
    craters.add(16, 16, 10, 1, 0)
    const elsewhere = bucketAt(16 + 14, 16 + 14)
    expect(craters.listed(elsewhere)).toContain(0)
    for (let i = 1; i < CRATER_SLOTS; i++) craters.add(8, 8, 1, 0.5, i)
    expect(craters.listed(bucketAt(8, 8))).not.toContain(0)
    expect(craters.listed(elsewhere)).not.toContain(0)
    expect(craters.listed(bucketAt(8, 8)).length).toBe(CRATER_SLOTS - 1)
  })

  it('takes an overwritten crater out of its buckets when the ring wraps', () => {
    const craters = new Craters()
    craters.add(1000, 1000, 5, 1, 0)
    for (let i = 1; i < CRATER_CAPACITY; i++) craters.add(-5000 + (i % 60) * 40, -5000 + Math.floor(i / 60) * 40, 5, 1, i)
    expect(craters.listed(bucketAt(1000, 1000))).toContain(0)
    craters.add(-9000, -9000, 5, 1, CRATER_CAPACITY)
    expect(craters.count).toBe(CRATER_CAPACITY)
    expect(craters.listed(bucketAt(1000, 1000))).not.toContain(0)
    expect(craters.listed(bucketAt(-9000, -9000))).toContain(0)
    expect(craters.data[0]).toBe(-9000)
  })

  it('expires the craters born before a time, oldest first, and keeps the rest listed', () => {
    const craters = new Craters()
    craters.add(0, 0, 5, 1, 10)
    craters.add(40, 0, 5, 1, 20)
    craters.add(80, 0, 5, 1, 30)
    craters.takeDirty()
    craters.expire(25)
    expect(craters.listed(bucketAt(0, 0))).not.toContain(0)
    expect(craters.listed(bucketAt(40, 0))).not.toContain(1)
    expect(craters.listed(bucketAt(80, 0))).toContain(2)
    expect(craters.takeDirty().buckets.length).toBeGreaterThan(0)
    // Nothing more is due: nothing changes.
    const version = craters.version
    craters.expire(25)
    expect(craters.version).toBe(version)
    // A new crater after the expired ones is kept until its own time.
    craters.add(0, 0, 5, 1, 40)
    craters.expire(35)
    expect(craters.listed(bucketAt(0, 0))).toContain(3)
    expect(craters.listed(bucketAt(80, 0))).not.toContain(2)
  })

  it('clears to nothing, and says the whole of it changed', () => {
    const craters = new Craters()
    craters.add(0, 0, 5, 1, 0)
    craters.clear()
    expect(craters.count).toBe(0)
    expect(craters.grid.every((v) => v === 0)).toBe(true)
    expect(craters.takeDirty().all).toBe(true)
  })

  it('keeps adding while hidden, and asks for everything again when shown', () => {
    const craters = new Craters()
    craters.add(0, 0, 5, 1, 0)
    craters.takeDirty()
    craters.visible = false
    craters.add(40, 0, 5, 1, 1)
    expect(craters.count).toBe(2)
    expect(craters.takeDirty().all).toBe(true)
    craters.visible = true
    expect(craters.takeDirty().all).toBe(true)
    expect(craters.listed(bucketAt(40, 0))).toContain(1)
  })
})
