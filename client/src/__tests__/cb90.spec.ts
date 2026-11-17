import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Mesh } from '@ragnarok/engine'
import { beforeAll, describe, expect, it } from 'vitest'
import { triangleCount } from './helpers'
import { kindView } from '@/kinds'
import { KIND_CB90 } from '@/net/codec'
import { Cb90Model, parseCb90Model } from '@/render/cb90'
import { CB90_CANNON_PIVOT, CB90_MG_BARREL_X, CB90_MG_PIVOT, CB90_SEATS } from '@/sim/consts'
import { crewGun, seatHead, vehicleDef } from '@/sim/vehicles'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]
const LEVEL = { yaw: 0, pitch: 0, shots: 0, range: 0, heat: 0, flags: 0 }

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/cb90.glb'))
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseCb90Model(copy.buffer)
})

const named = (boat: Cb90Model, name: string) => boat.root.getObjectByName(name)!

describe('Cb90Model', () => {
  it('has its cannon on the game’s pivot and its machine guns on theirs, on the hull', () => {
    const boat = new Cb90Model(1)
    const turret = named(boat, 'turret')
    CB90_CANNON_PIVOT.forEach((v, i) => expect(turret.position.toArray()[i]).toBeCloseTo(v, 4))
    expect(named(boat, 'gun').parent).toBe(turret)
    const mg = named(boat, 'mg')
    CB90_MG_PIVOT.forEach((v, i) => expect(mg.position.toArray()[i]).toBeCloseTo(v, 4))
    expect(named(boat, 'mg_gun').parent).toBe(mg)
    let underTurret = false
    for (let node = mg.parent; node; node = node.parent) underTurret ||= node === turret
    expect(underTurret).toBe(false)
    boat.dispose()
  })

  it('stays low-poly', () => {
    const boat = new Cb90Model(1)
    expect(triangleCount(boat.root)).toBeLessThan(2500)
    boat.dispose()
  })

  it('tilts its machine guns, whatever the cannon does, and fires them from either side', () => {
    const boat = new Cb90Model(1)
    boat.update([0, 0.2, 0], IDENTITY, 0, [0, 0, 0, 0], { ...LEVEL, yaw: Math.PI })
    boat.setPintle({ yaw: 0, pitch: 0.2, shots: 0, heat: 0, flags: 0 })
    const { direction, origin } = boat.pintleBarrel()!
    expect(direction.x).toBeCloseTo(0, 4)
    expect(direction.y).toBeCloseTo(Math.sin(0.2), 3)
    expect(origin.z).toBeCloseTo(CB90_MG_PIVOT[2], 3)
    // Looking ahead, the left barrel is to the left of the right one, a barrel's spacing either side.
    boat.setPintle({ yaw: 0, pitch: 0, shots: 0, heat: 0, flags: 0 })
    expect(boat.machineGun(-1)!.origin.x).toBeCloseTo(-CB90_MG_BARREL_X, 3)
    expect(boat.machineGun(1)!.origin.x).toBeCloseTo(CB90_MG_BARREL_X, 3)
    boat.dispose()
  })

  it('draws no heads in hatches: its crew are soldiers', () => {
    const boat = new Cb90Model(1)
    boat.update([0, 0.2, 0], IDENTITY, 0, [5, 6, 7, 8], LEVEL)
    let shown = 0
    boat.root.traverse((node) => {
      if (node instanceof Mesh && node.userData.head && node.visible) shown++
    })
    expect(shown).toBe(0)
    boat.dispose()
  })
})

describe('the Stridsbåt 90 as a kind', () => {
  it('seats a driver on the machine guns, a gunner standing at the cannon and two riders who fire their own guns', () => {
    const { seats } = kindView(KIND_CB90).ride
    expect(seats).toHaveLength(CB90_SEATS)
    expect(seats.map((seat) => seat.role)).toEqual(['coxswain', 'gunner', 'passenger', 'passenger'])
    expect(seats[0]!.rig.name).toBe('tanker')
    expect(seats[1]!.stand).toBe(true)
    expect(seats[0]!.gauges).toEqual(['pintle'])
    expect(seats[1]!.gauges).toEqual(['heat'])
    expect([0, 1, 2, 3].map((seat) => crewGun(KIND_CB90, seat))).toEqual([false, false, true, true])
    expect(vehicleDef(KIND_CB90).crew).toBe('glazed')
    for (let seat = 0; seat < CB90_SEATS; seat++) expect(seatHead(KIND_CB90, seat)).not.toBeNull()
    expect(kindView(KIND_CB90).fx.floats).toBe(true)
  })
})
