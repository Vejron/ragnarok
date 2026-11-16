import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Mesh } from '@ragnarok/engine'
import { beforeAll, describe, expect, it } from 'vitest'
import { boxOf, triangleCount } from './helpers'
import { kindView } from '@/kinds'
import { KIND_PBV, KIND_STRF } from '@/net/codec'
import { parsePbvModel, PbvModel } from '@/render/pbv'
import { PBV_BARREL_LENGTH, PBV_SEATS, PBV_TURRET_PIVOT } from '@/sim/consts'
import { vehicleDef } from '@/sim/vehicles'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/pbv.glb'))
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parsePbvModel(copy.buffer)
})

const named = (pbv: PbvModel, name: string) => pbv.root.getObjectByName(name)!

describe('PbvModel', () => {
  it('has seven wheels a side and a turret on the game’s ring', () => {
    const pbv = new PbvModel(1)
    for (const side of ['L', 'R']) for (let n = 0; n < 7; n++) expect(named(pbv, `wheel_${side}_${n}`)).toBeTruthy()
    const turret = named(pbv, 'turret')
    PBV_TURRET_PIVOT.forEach((v, i) => expect(turret.position.toArray()[i]).toBeCloseTo(v, 4))
    expect(named(pbv, 'gun').parent).toBe(turret)
    pbv.dispose()
  })

  it('stays low-poly', () => {
    const pbv = new PbvModel(1)
    expect(triangleCount(pbv.root)).toBeLessThan(3000)
    pbv.dispose()
  })

  it('points the barrel where the turret says, out to the gun’s length', () => {
    const pbv = new PbvModel(1)
    pbv.update([10, 2, 5], IDENTITY, 0, [0, 0, 0, 0], { yaw: 0, pitch: 0, shots: 0, range: 0, heat: 0, flags: 0 })
    const level = pbv.barrel()
    expect(level.direction.z).toBeCloseTo(-1, 3)
    expect(level.origin.y).toBeCloseTo(2 + PBV_TURRET_PIVOT[1], 3)
    expect(boxOf(named(pbv, 'gun')).min.z).toBeCloseTo(5 + PBV_TURRET_PIVOT[2] - PBV_BARREL_LENGTH, 1)

    pbv.update([10, 2, 5], IDENTITY, 0, [0, 0, 0, 0], { yaw: Math.PI / 2, pitch: 0.6, shots: 0, range: 0, heat: 0, flags: 0 })
    const turned = pbv.barrel().direction
    expect(turned.x).toBeLessThan(-0.7)
    expect(turned.y).toBeCloseTo(Math.sin(0.6), 3)
    pbv.dispose()
  })

  it('shows the driver and the gunner, never the troops in the back', () => {
    const pbv = new PbvModel(1)
    const shown = () => {
      let count = 0
      pbv.root.traverse((node) => {
        if (node instanceof Mesh && node.userData.head && node.visible) count++
      })
      return count
    }
    pbv.update([0, 1, 0], IDENTITY, 0, [0, 0, 7, 8])
    expect(shown()).toBe(0)
    pbv.update([0, 1, 0], IDENTITY, 0, [5, 6, 7, 8])
    expect(shown()).toBe(2)
    pbv.dispose()
  })
})

describe('the Pbv 302 as a kind', () => {
  it('seats a driver, a gunner with the heat gauge, and two troops, as the Strf 90 does', () => {
    const { seats } = kindView(KIND_PBV).ride
    expect(seats).toHaveLength(PBV_SEATS)
    expect(seats.map((seat) => seat.role)).toEqual(['driver', 'gunner', 'passenger', 'passenger'])
    expect(seats[1]!.gauges).toEqual(['heat'])
    expect(vehicleDef(KIND_PBV).crew).toBe('sealed')
    expect(vehicleDef(KIND_PBV).seats).toBe(vehicleDef(KIND_STRF).seats)
  })
})
