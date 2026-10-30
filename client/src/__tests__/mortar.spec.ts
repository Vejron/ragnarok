import { Vector3 } from '@ragnarok/engine'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { boxOf, triangleCount } from './helpers'
import { MortarModel, STOW_SECONDS } from '@/render/mortar'
import { MORTAR_BARREL_LENGTH, MORTAR_GUN_PIVOT, MORTAR_TRACK_HALF } from '@/sim/consts'
import { TURRET_STOWED } from '@/net/codec'
import type { Turret } from '@/entities/units'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]
const AIM: Turret = { yaw: 0.5, pitch: 1.0, shots: 0, range: 120, flags: 0 }
const named = (mortar: MortarModel, name: string) => mortar.root.getObjectByName(name)!
const world = (mortar: MortarModel, name: string) => {
  mortar.root.updateMatrixWorld(true)
  return named(mortar, name).getWorldPosition(new Vector3())
}

afterEach(() => vi.restoreAllMocks())

describe('MortarModel', () => {
  it('has four wheels, two trails and the gun on its pivot', () => {
    const mortar = new MortarModel(1)
    for (const wheel of ['FL', 'FR', 'RL', 'RR']) expect(named(mortar, `wheel_${wheel}`)).toBeTruthy()
    for (const part of ['trail_L', 'trail_R', 'pad', 'gun']) expect(named(mortar, part)).toBeTruthy()
    MORTAR_GUN_PIVOT.forEach((v, i) => expect(named(mortar, 'mount').position.toArray()[i]).toBeCloseTo(v, 4))
    mortar.dispose()
  })

  it('stays low-poly', () => {
    const mortar = new MortarModel(1)
    expect(triangleCount(mortar.root)).toBeLessThan(3000)
    mortar.dispose()
  })

  it('aims the barrel along the turret angles when set up, and draws it as long as the real one', () => {
    const mortar = new MortarModel(1)
    mortar.update([0, 1, 0], IDENTITY, 0, [7], AIM)
    const { origin, direction } = mortar.barrel()
    expect(origin.y).toBeCloseTo(1 + MORTAR_GUN_PIVOT[1], 3)
    expect(direction.x).toBeCloseTo(-Math.cos(AIM.pitch) * Math.sin(AIM.yaw), 3)
    expect(direction.y).toBeCloseTo(Math.sin(AIM.pitch), 3)
    expect(direction.z).toBeCloseTo(-Math.cos(AIM.pitch) * Math.cos(AIM.yaw), 3)

    mortar.update([0, 1, 0], IDENTITY, 0, [7], { ...AIM, yaw: 0, pitch: 0 })
    mortar.root.updateMatrixWorld(true)
    expect(boxOf(named(mortar, 'gun')).min.z).toBeCloseTo(MORTAR_GUN_PIVOT[2] - MORTAR_BARREL_LENGTH, 1)
    mortar.dispose()
  })

  it('is drawn folded straight away when first seen hitched: barrel on the rear lock, trails shut at the hitch', () => {
    const mortar = new MortarModel(1)
    mortar.update([0, 0, 0], IDENTITY, 0, [0], { ...AIM, flags: TURRET_STOWED })
    const { direction } = mortar.barrel()
    expect(direction.z).toBeCloseTo(1, 3)
    const left = world(mortar, 'tip_L')
    const right = world(mortar, 'tip_R')
    expect(left.distanceTo(right)).toBeLessThan(0.05)
    expect(left.z).toBeCloseTo(-1.75, 2)
    mortar.dispose()
  })

  it('unfolds over STOW_SECONDS once let go, trails first and the barrel last', () => {
    let now = 1000
    vi.spyOn(performance, 'now').mockImplementation(() => now)
    const mortar = new MortarModel(1)
    mortar.update([0, 0, 0], IDENTITY, 0, [0], { ...AIM, flags: TURRET_STOWED })
    const padUp = world(mortar, 'pad').y

    now += (STOW_SECONDS * 1000) / 4
    mortar.update([0, 0, 0], IDENTITY, 0, [0], AIM)
    // A quarter in, the trails are opening but the barrel still lies on its lock.
    expect(world(mortar, 'tip_L').x).toBeLessThan(0)
    expect(mortar.barrel().direction.z).toBeCloseTo(1, 3)

    now += STOW_SECONDS * 1000
    mortar.update([0, 0, 0], IDENTITY, 0, [0], AIM)
    expect(mortar.barrel().direction.y).toBeCloseTo(Math.sin(AIM.pitch), 3)
    expect(world(mortar, 'pad').y).toBeLessThan(padUp - 0.4)
    // The trails spread wide, their tips down by the ground, clear of the wheels' track.
    const left = world(mortar, 'tip_L')
    expect(left.x).toBeLessThan(-0.5)
    expect(left.x).toBeGreaterThan(-MORTAR_TRACK_HALF - 0.3)
    expect(left.y).toBeLessThan(-0.8)
    mortar.dispose()
  })
})
