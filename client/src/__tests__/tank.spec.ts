import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Mesh } from '@ragnarok/engine'
import { beforeAll, describe, expect, it } from 'vitest'
import { boxOf, triangleCount } from './helpers'
import { parseTankModel, TankModel } from '@/render/tank'
import { TANK_BARREL_LENGTH, TANK_TURRET_PIVOT } from '@/sim/consts'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/tank.glb'))
  // A copy made here, so it is an ArrayBuffer of the realm the loader checks against.
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseTankModel(copy.buffer)
})

function named(tank: TankModel, name: string) {
  return tank.root.getObjectByName(name)!
}

describe('TankModel', () => {
  it('has seven wheels a side and a turret on the game’s ring', () => {
    const tank = new TankModel(1)
    for (const side of ['L', 'R']) for (let n = 0; n < 7; n++) expect(named(tank, `wheel_${side}_${n}`)).toBeTruthy()
    const turret = named(tank, 'turret')
    TANK_TURRET_PIVOT.forEach((v, i) => expect(turret.position.toArray()[i]).toBeCloseTo(v, 4))
    expect(named(tank, 'gun').parent).toBe(turret)
    tank.dispose()
  })

  it('stays low-poly', () => {
    const tank = new TankModel(1)
    const triangles = triangleCount(tank.root)
    expect(triangles).toBeLessThan(2500)
    tank.dispose()
  })

  it('points the barrel where the turret says, and ends it at the shell’s range', () => {
    const tank = new TankModel(1)
    tank.update([10, 2, 5], IDENTITY, 0, [0, 0], { yaw: 0, pitch: 0, shots: 0, range: 0 })
    const level = tank.barrel()
    expect(level.direction.z).toBeCloseTo(-1, 3)
    expect(level.origin.y).toBeCloseTo(2 + TANK_TURRET_PIVOT[1], 3)
    const bounds = boxOf(named(tank, 'gun'))
    expect(bounds.min.z).toBeCloseTo(5 + TANK_TURRET_PIVOT[2] - TANK_BARREL_LENGTH, 1)

    tank.update([10, 2, 5], IDENTITY, 0, [0, 0], { yaw: Math.PI / 2, pitch: 0.3, shots: 0, range: 0 })
    const turned = tank.barrel().direction
    // A quarter turn to the left points at -X, and the nod lifts it.
    expect(turned.x).toBeLessThan(-0.9)
    expect(turned.y).toBeGreaterThan(0.2)
    tank.dispose()
  })

  it('rolls both tracks the same way when driving and opposite ways when pivoting', () => {
    const tank = new TankModel(1)
    const wheel = (side: string) => named(tank, `wheel_${side}_2`).rotation.x
    tank.update([0, 1, 0], IDENTITY, 0, [0, 0])
    tank.update([0, 1, -1], IDENTITY, 0, [0, 0])
    expect(wheel('L')).toBeCloseTo(wheel('R'), 6)
    expect(Math.abs(wheel('L'))).toBeGreaterThan(1)

    const spun = new TankModel(2)
    const yawed = (angle: number): [number, number, number, number] => [0, Math.sin(angle / 2), 0, Math.cos(angle / 2)]
    spun.update([0, 1, 0], yawed(0), 0, [0, 0])
    spun.update([0, 1, 0], yawed(0.2), 0, [0, 0])
    const [left, right] = ['L', 'R'].map((side) => spun.root.getObjectByName(`wheel_${side}_2`)!.rotation.x)
    expect(left! * right!).toBeLessThan(0)
    tank.dispose()
    spun.dispose()
  })

  it('shows a head for each occupied seat, tinted by player', () => {
    const tank = new TankModel(1)
    const visible = () => tank.root.children.filter((c) => c instanceof Mesh && c.visible).length
    tank.update([0, 1, 0], IDENTITY, 0, [0, 0])
    expect(visible()).toBe(0)
    tank.update([0, 1, 0], IDENTITY, 0, [7, 0])
    expect(visible()).toBe(1)
    tank.update([0, 1, 0], IDENTITY, 0, [7, 9])
    expect(visible()).toBe(1)
    expect(named(tank, 'turret').children.some((c) => c instanceof Mesh && c.visible)).toBe(true)
    tank.dispose()
  })
})
