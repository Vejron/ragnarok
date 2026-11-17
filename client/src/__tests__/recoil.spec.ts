import { describe, expect, it } from 'vitest'
import { GunClimb, ScopeRecoil } from '@/render/recoil'
import { ITEM_AK5, ITEM_REVOLVER, ITEM_SHOTGUN, ITEM_SNIPER, itemDef } from '@/sim/items'

describe('scope recoil', () => {
  const run = (recoil: ScopeRecoil, seconds: number) => {
    let last = { pitch: 0, yaw: 0 }
    for (let t = 0; t < seconds; t += 1 / 120) last = recoil.step(1 / 120)
    return last
  }

  it('stays still with no shot', () => {
    expect(run(new ScopeRecoil(), 1)).toEqual({ pitch: 0, yaw: 0 })
  })

  it('throws the view up within a few hundredths of a second and settles back in about half a second', () => {
    const recoil = new ScopeRecoil()
    recoil.kick(0.03, 0.01)
    const top = run(recoil, 0.05)
    expect(top.pitch).toBeCloseTo(0.03, 3)
    expect(top.yaw).toBeCloseTo(0.01, 3)
    const settled = run(recoil, 0.55)
    expect(Math.abs(settled.pitch)).toBeLessThan(0.003)
    expect(Math.abs(settled.yaw)).toBeLessThan(0.001)
  })

  it('a second shot kicks on from wherever the first left the view', () => {
    const recoil = new ScopeRecoil()
    recoil.kick(0.03, 0)
    const between = run(recoil, 0.1).pitch
    recoil.kick(0.03, 0)
    expect(run(recoil, 0.05).pitch).toBeCloseTo(between + 0.03, 3)
  })

  it('the AK5 and the revolver climb up and to the left, the revolver harder a shot; the sniper kicks, the shotgun not', () => {
    const ak = itemDef(ITEM_AK5)!, revolver = itemDef(ITEM_REVOLVER)!
    for (const gun of [ak, revolver]) {
      expect(gun.recoilRise).toBeGreaterThan(0)
      // Left whatever the jitter draws.
      expect(gun.recoilLeft).toBeGreaterThan(gun.recoilJitter)
    }
    expect(revolver.recoilRise).toBeGreaterThan(ak.recoilRise)
    expect(itemDef(ITEM_SNIPER)!.recoilRise).toBeGreaterThan(0)
    expect(itemDef(ITEM_SHOTGUN)!.recoilRise).toBe(0)
  })
})

describe('gun climb', () => {
  it('hands all of a shot to the aim within 0.15 s, a little each frame', () => {
    const climb = new GunClimb()
    climb.add(0.001, 0.004)
    const first = climb.step(1 / 60)
    expect(first.pitch).toBeGreaterThan(0)
    expect(first.pitch).toBeLessThan(0.004)
    let pitch = first.pitch, yaw = first.yaw
    for (let frame = 1; frame < 9; frame++) {
      const step = climb.step(1 / 60)
      pitch += step.pitch
      yaw += step.yaw
    }
    expect(pitch).toBeCloseTo(0.004, 4)
    expect(yaw).toBeCloseTo(0.001, 4)
  })

  it('a burst adds up, and a reset drops what was still to come', () => {
    const climb = new GunClimb()
    for (let shot = 0; shot < 10; shot++) climb.add(0, 0.004)
    let pitch = 0
    for (let frame = 0; frame < 30; frame++) pitch += climb.step(1 / 60).pitch
    expect(pitch).toBeCloseTo(0.04, 4)
    climb.add(0, 0.004)
    climb.reset()
    expect(climb.step(1 / 60).pitch).toBe(0)
  })
})
