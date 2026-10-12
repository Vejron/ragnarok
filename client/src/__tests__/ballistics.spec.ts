import { describe, expect, it } from 'vitest'
import type { BulletSpawn } from '@/net/codec'
import { bulletLifeTicks, bulletPosition, bulletTicksTo, bulletVelocity, BULLET_GRAVITY_SCALE } from '@/sim/ballistics'
import { GRAVITY, TICK_DT } from '@/sim/consts'
import { ITEM_AK5 } from '@/sim/items'

const spawn: BulletSpawn = { id: 9, shooter: 1, item: ITEM_AK5, x: 10, y: 1.7, z: -4, vx: 3, vy: 1.5, vz: -279, tick: 100 }

/** The server's `step_bullet`, tick by tick: a little gravity on the velocity, then the move. */
const stepped = (b: BulletSpawn, n: number): [number, number, number] => {
  let [x, y, z, vy] = [b.x, b.y, b.z, b.vy]
  for (let i = 0; i < n; i++) {
    vy -= GRAVITY * BULLET_GRAVITY_SCALE * TICK_DT
    x += b.vx * TICK_DT
    y += vy * TICK_DT
    z += b.vz * TICK_DT
  }
  return [x, y, z]
}

describe('ballistics', () => {
  it('matches the server stepping the bullet tick by tick', () => {
    for (const n of [0, 1, 5, 20, 43]) {
      const [x, y, z] = bulletPosition(spawn, n)
      const [sx, sy, sz] = stepped(spawn, n)
      expect(x).toBeCloseTo(sx, 6)
      expect(y).toBeCloseTo(sy, 6)
      expect(z).toBeCloseTo(sz, 6)
    }
  })

  it('drops a little over a gun range and heads along the fall', () => {
    const level = { ...spawn, vy: 0 }
    const [, y] = bulletPosition(level, 43)
    expect(level.y - y).toBeGreaterThan(0.5)
    expect(level.y - y).toBeLessThan(2.5)
    expect(bulletVelocity(level, 43)[1]).toBeLessThan(0)
  })

  it('lives as long as its gun range allows and reports how far along a point is', () => {
    expect(bulletLifeTicks(spawn)).toBeGreaterThan(30)
    const [x, y, z] = bulletPosition(spawn, 12)
    expect(bulletTicksTo(spawn, [x, y, z])).toBeCloseTo(12, 0)
  })
})
