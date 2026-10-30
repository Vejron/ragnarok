import { Vector3 } from '@ragnarok/engine'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { boxOf, triangleCount } from './helpers'
import { PakModel, STOW_SECONDS } from '@/render/pak'
import { PAK_AXLES_Z, PAK_BARREL_LENGTH, PAK_GUN_PIVOT, PAK_TONGUE_Z, PAK_WHEEL_RADIUS } from '@/sim/consts'
import { TURRET_STOWED } from '@/net/codec'
import type { Turret } from '@/entities/units'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]
const AIM: Turret = { yaw: 0.3, pitch: 0.2, shots: 0, range: 0, flags: 0 }
const named = (pak: PakModel, name: string) => pak.root.getObjectByName(name)!
const world = (pak: PakModel, name: string) => {
  pak.root.updateMatrixWorld(true)
  return named(pak, name).getWorldPosition(new Vector3())
}
/** The hull frame's ground: the wheels' bottom (`WHEEL_Y` in `pak.ts`). */
const GROUND_Y = -0.3 - PAK_WHEEL_RADIUS

afterEach(() => vi.restoreAllMocks())

describe('PakModel', () => {
  it('has two wheels, two trails and the gun on its trunnions, and stays low-poly', () => {
    const pak = new PakModel(1)
    for (const part of ['wheel_L', 'wheel_R', 'trail_L', 'trail_R', 'gun']) expect(named(pak, part)).toBeTruthy()
    PAK_GUN_PIVOT.forEach((v, i) => expect(named(pak, 'mount').position.toArray()[i]).toBeCloseTo(v, 4))
    expect(triangleCount(pak.root)).toBeLessThan(3000)
    pak.dispose()
  })

  it('lays the barrel along the turret angles, as long as the real one', () => {
    const pak = new PakModel(1)
    pak.update([0, 1, 0], IDENTITY, 0, [7], AIM)
    const { origin, direction } = pak.barrel()
    expect(origin.y).toBeCloseTo(1 + PAK_GUN_PIVOT[1], 3)
    expect(direction.x).toBeCloseTo(-Math.cos(AIM.pitch) * Math.sin(AIM.yaw), 3)
    expect(direction.y).toBeCloseTo(Math.sin(AIM.pitch), 3)
    expect(direction.z).toBeCloseTo(-Math.cos(AIM.pitch) * Math.cos(AIM.yaw), 3)

    pak.update([0, 1, 0], IDENTITY, 0, [7], { ...AIM, yaw: 0, pitch: 0 })
    pak.root.updateMatrixWorld(true)
    expect(boxOf(named(pak, 'gun')).min.z).toBeCloseTo(PAK_GUN_PIVOT[2] - PAK_BARREL_LENGTH, 1)
    pak.dispose()
  })

  it('stands set up with its trails spread and their spades down by the ground, behind the wheels', () => {
    const pak = new PakModel(1)
    pak.update([0, 0, 0], IDENTITY, 0, [0], AIM)
    const left = world(pak, 'tip_L')
    const right = world(pak, 'tip_R')
    expect(left.x).toBeLessThan(-0.6)
    expect(right.x).toBeGreaterThan(0.6)
    expect(left.y).toBeCloseTo(GROUND_Y + 0.08, 1)
    // About where the physics rests it on the spades.
    expect(left.z).toBeCloseTo(PAK_AXLES_Z[1], 0)
    pak.dispose()
  })

  it('is drawn closed up straight away when first seen hitched: barrel level and centred, trails shut at the towing eye', () => {
    const pak = new PakModel(1)
    pak.update([0, 0, 0], IDENTITY, 0, [0], { ...AIM, flags: TURRET_STOWED })
    expect(pak.barrel().direction.z).toBeCloseTo(-1, 3)
    const left = world(pak, 'tip_L')
    const right = world(pak, 'tip_R')
    expect(left.distanceTo(right)).toBeLessThan(0.05)
    expect(left.z).toBeCloseTo(PAK_TONGUE_Z, 2)
    expect(left.y).toBeCloseTo(0, 2)
    pak.dispose()
  })

  it('opens over STOW_SECONDS once let go, and the barrel goes back on its aim', () => {
    let now = 1000
    vi.spyOn(performance, 'now').mockImplementation(() => now)
    const pak = new PakModel(1)
    pak.update([0, 0, 0], IDENTITY, 0, [0], { ...AIM, flags: TURRET_STOWED })
    now += STOW_SECONDS * 500
    pak.update([0, 0, 0], IDENTITY, 0, [0], AIM)
    const half = world(pak, 'tip_R').x
    expect(half).toBeGreaterThan(0.05)
    now += STOW_SECONDS * 1000
    pak.update([0, 0, 0], IDENTITY, 0, [0], AIM)
    expect(world(pak, 'tip_R').x).toBeGreaterThan(half)
    expect(pak.barrel().direction.y).toBeCloseTo(Math.sin(AIM.pitch), 3)
    pak.dispose()
  })
})
