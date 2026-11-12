import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Mesh } from '@ragnarok/engine'
import { beforeAll, describe, expect, it } from 'vitest'
import { boxOf, triangleCount } from './helpers'
import { kindView } from '@/kinds'
import { KIND_STRF } from '@/net/codec'
import { parseStrfModel, StrfModel } from '@/render/strf'
import { STRF_BARREL_LENGTH, STRF_SEATS, STRF_TURRET_PIVOT } from '@/sim/consts'
import { vehicleDef } from '@/sim/vehicles'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/strf.glb'))
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseStrfModel(copy.buffer)
})

const named = (strf: StrfModel, name: string) => strf.root.getObjectByName(name)!

describe('StrfModel', () => {
  it('has nine wheels a side and a turret on the game’s ring', () => {
    const strf = new StrfModel(1)
    for (const side of ['L', 'R']) for (let n = 0; n < 9; n++) expect(named(strf, `wheel_${side}_${n}`)).toBeTruthy()
    const turret = named(strf, 'turret')
    STRF_TURRET_PIVOT.forEach((v, i) => expect(turret.position.toArray()[i]).toBeCloseTo(v, 4))
    expect(named(strf, 'gun').parent).toBe(turret)
    strf.dispose()
  })

  it('stays low-poly', () => {
    const strf = new StrfModel(1)
    expect(triangleCount(strf.root)).toBeLessThan(2500)
    strf.dispose()
  })

  it('points the barrel where the turret says, out to the gun’s length', () => {
    const strf = new StrfModel(1)
    strf.update([10, 2, 5], IDENTITY, 0, [0, 0, 0, 0], { yaw: 0, pitch: 0, shots: 0, range: 0, heat: 0, flags: 0 })
    const level = strf.barrel()
    expect(level.direction.z).toBeCloseTo(-1, 3)
    expect(level.origin.y).toBeCloseTo(2 + STRF_TURRET_PIVOT[1], 3)
    expect(boxOf(named(strf, 'gun')).min.z).toBeCloseTo(5 + STRF_TURRET_PIVOT[2] - STRF_BARREL_LENGTH, 1)

    strf.update([10, 2, 5], IDENTITY, 0, [0, 0, 0, 0], { yaw: Math.PI / 2, pitch: 0.6, shots: 0, range: 0, heat: 0, flags: 0 })
    const turned = strf.barrel().direction
    expect(turned.x).toBeLessThan(-0.7)
    expect(turned.y).toBeCloseTo(Math.sin(0.6), 3)
    strf.dispose()
  })

  it('shows the driver and the gunner, never the troops in the back', () => {
    const strf = new StrfModel(1)
    const shown = () => {
      let count = 0
      strf.root.traverse((node) => {
        if (node instanceof Mesh && node.userData.head && node.visible) count++
      })
      return count
    }
    strf.update([0, 1, 0], IDENTITY, 0, [0, 0, 7, 8])
    expect(shown()).toBe(0)
    strf.update([0, 1, 0], IDENTITY, 0, [5, 6, 7, 8])
    expect(shown()).toBe(2)
    strf.dispose()
  })
})

describe('the Strf 90 as a kind', () => {
  it('seats a driver, a gunner with the round and heat gauges, and two troops', () => {
    const { seats } = kindView(KIND_STRF).ride
    expect(seats).toHaveLength(STRF_SEATS)
    expect(seats.map((seat) => seat.role)).toEqual(['driver', 'gunner', 'passenger', 'passenger'])
    expect(seats[1]!.gauges).toEqual(['ammo', 'heat'])
    expect(vehicleDef(KIND_STRF).crew).toBe('sealed')
  })
})
