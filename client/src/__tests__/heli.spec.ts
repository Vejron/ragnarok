import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { beforeAll, describe, expect, it } from 'vitest'
import { boxOf, triangleCount } from './helpers'
import { HeliModel, parseHeliModel } from '@/render/heli'
import { HELI_GUN_LENGTH, HELI_GUN_PIVOT } from '@/sim/consts'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/heli.glb'))
  // A copy made here, so it is an ArrayBuffer of the realm the loader checks against.
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseHeliModel(copy.buffer)
})

function named(heli: HeliModel, name: string) {
  return heli.root.getObjectByName(name)!
}

describe('HeliModel', () => {
  it('has a rotor, a tail rotor and a gun mount on the game’s pivot', () => {
    const heli = new HeliModel(1)
    expect(named(heli, 'rotor')).toBeTruthy()
    expect(named(heli, 'tail_rotor')).toBeTruthy()
    const turret = named(heli, 'turret')
    HELI_GUN_PIVOT.forEach((v, i) => expect(turret.position.toArray()[i]).toBeCloseTo(v, 4))
    expect(named(heli, 'gun').parent).toBe(turret)
    heli.dispose()
  })

  it('stays low-poly', () => {
    const heli = new HeliModel(1)
    const triangles = triangleCount(heli.root)
    expect(triangles).toBeLessThan(3000)
    heli.dispose()
  })

  it('swings the gun with the turret angles and ends the barrel at the muzzle', () => {
    const heli = new HeliModel(1)
    heli.update([10, 2, 5], IDENTITY, 0, [0, 0], { yaw: 0, pitch: 0, shots: 0, range: 0, stowed: false })
    const level = heli.barrel()
    expect(level.direction.z).toBeCloseTo(-1, 3)
    expect(level.origin.y).toBeCloseTo(2 + HELI_GUN_PIVOT[1], 3)
    const bounds = boxOf(named(heli, 'gun'))
    expect(bounds.min.z).toBeCloseTo(5 + HELI_GUN_PIVOT[2] - HELI_GUN_LENGTH, 1)
    expect(heli.muzzle().z).toBeCloseTo(5 + HELI_GUN_PIVOT[2] - HELI_GUN_LENGTH, 3)

    heli.update([10, 2, 5], IDENTITY, 0, [0, 0], { yaw: Math.PI / 4, pitch: -0.4, shots: 0, range: 0, stowed: false })
    const turned = heli.barrel().direction
    // A turn to the left points towards -X, and the nod lowers it.
    expect(turned.x).toBeLessThan(-0.5)
    expect(turned.y).toBeLessThan(-0.2)
    heli.dispose()
  })

  it('spins the rotors up once there is a pilot and tells who is at the gun', () => {
    const heli = new HeliModel(1)
    const rotor = named(heli, 'rotor')
    heli.update([0, 0, 0], IDENTITY, 0, [0, 0], null)
    expect(rotor.rotation.y).toBe(0)
    expect(heli.gunner).toBe(0)
    const start = performance.now()
    while (performance.now() - start < 30) heli.update([0, 0, 0], IDENTITY, 0, [3, 7], null)
    expect(Math.abs(rotor.rotation.y)).toBeGreaterThan(0)
    expect(heli.gunner).toBe(7)
    heli.dispose()
  })
})
