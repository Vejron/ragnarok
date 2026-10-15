import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Mesh } from '@ragnarok/engine'
import { beforeAll, describe, expect, it } from 'vitest'
import { boxOf, triangleCount } from './helpers'
import { parseStrvModel, StrvModel } from '@/render/strvs'
import { STRV_BARREL_LENGTH, STRV_GUN_PIVOT } from '@/sim/consts'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/strvs.glb'))
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseStrvModel(copy.buffer)
})

const named = (tank: StrvModel, name: string) => tank.root.getObjectByName(name)!

describe('StrvModel', () => {
  it('has seven wheels a side, a gun on the trunnion and no turret', () => {
    const tank = new StrvModel(1)
    for (const side of ['L', 'R']) for (let n = 0; n < 7; n++) expect(named(tank, `wheel_${side}_${n}`)).toBeTruthy()
    expect(tank.root.getObjectByName('turret')).toBeUndefined()
    const gun = named(tank, 'gun')
    STRV_GUN_PIVOT.forEach((v, i) => expect(gun.position.toArray()[i]).toBeCloseTo(v, 4))
    expect(tank.turretOn).toBe(false)
    tank.dispose()
  })

  it('stays low-poly', () => {
    const tank = new StrvModel(1)
    const triangles = triangleCount(tank.root)
    expect(triangles).toBeLessThan(3000)
    tank.dispose()
  })

  it('nods the gun with the pitch and never turns it sideways', () => {
    const tank = new StrvModel(1)
    tank.update([0, 1, 0], IDENTITY, 0, [0], { yaw: 1.2, pitch: 0.25, shots: 0, range: 0 })
    const { origin, direction } = tank.barrel()
    expect(origin.y).toBeCloseTo(1 + STRV_GUN_PIVOT[1], 3)
    expect(direction.y).toBeCloseTo(Math.sin(0.25), 3)
    // A turret yaw from the wire is ignored: the gun is fixed in the hull.
    expect(direction.x).toBeCloseTo(0, 3)
    const bounds = boxOf(named(tank, 'gun'))
    expect(bounds.min.z).toBeLessThan(STRV_GUN_PIVOT[2] - STRV_BARREL_LENGTH * 0.9 * Math.cos(0.25))
    tank.dispose()
  })

  it('shows the driver only', () => {
    const tank = new StrvModel(1)
    const heads = () => tank.root.children.filter((c) => c instanceof Mesh && c.visible).length
    tank.update([0, 1, 0], IDENTITY, 0, [0])
    expect(heads()).toBe(0)
    tank.update([0, 1, 0], IDENTITY, 0, [5])
    expect(heads()).toBe(1)
    tank.dispose()
  })
})
