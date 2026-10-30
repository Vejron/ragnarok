import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Quat, Vector3 } from '@ragnarok/engine'
import { beforeAll, describe, expect, it } from 'vitest'
import { boxOf, triangleCount } from './helpers'
import { parsePlaneModel, PlaneModel } from '@/render/plane'
import { PLANE_BOMBS, PLANE_GUN_PIVOTS, PLANE_REAR_LENGTH, PLANE_REAR_PIVOT } from '@/sim/consts'
import { ARMS_CUT } from '@/net/codec'
import type { Arms } from '@/entities/units'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]
const ARMS: Arms = { shots: 0, heat: 0, rear_heat: 0, bombs: PLANE_BOMBS, flags: 0 }

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/plane.glb'))
  // A copy made here, so it is an ArrayBuffer of the realm the loader checks against.
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parsePlaneModel(copy.buffer)
})

function named(plane: PlaneModel, name: string) {
  return plane.root.getObjectByName(name)!
}

/** Lets a little time pass, so the next update has a frame's worth of it. */
function wait(ms: number) {
  const start = performance.now()
  while (performance.now() - start < ms);
}

describe('PlaneModel', () => {
  it('has its moving parts on the game’s pivots', () => {
    const plane = new PlaneModel(1)
    for (const name of ['prop', 'elevator', 'rudder', 'aileron_L', 'aileron_R', ...Array.from({ length: PLANE_BOMBS }, (_, k) => `bomb_${k + 1}`)]) {
      expect(named(plane, name), name).toBeTruthy()
    }
    PLANE_GUN_PIVOTS.forEach((pivot, side) => {
      const gun = named(plane, side === 0 ? 'gun_L' : 'gun_R')
      pivot.forEach((v, i) => expect(gun.getWorldPosition(new Vector3()).toArray()[i]).toBeCloseTo(v, 3))
    })
    const ring = named(plane, 'turret')
    PLANE_REAR_PIVOT.forEach((v, i) => expect(ring.position.toArray()[i]).toBeCloseTo(v, 4))
    expect(named(plane, 'gun').parent).toBe(ring)
    plane.dispose()
  })

  it('stays low-poly', () => {
    const plane = new PlaneModel(1)
    const triangles = triangleCount(plane.root)
    expect(triangles).toBeLessThan(3000)
    plane.dispose()
  })

  it('fires the forward guns along the nose from their muzzles', () => {
    const plane = new PlaneModel(1)
    plane.update([10, 2, 5], IDENTITY, 0, [3, 0], null, ARMS)
    const left = plane.forwardGun(0)
    const right = plane.forwardGun(1)
    expect(left.direction.z).toBeCloseTo(-1, 4)
    expect(left.origin.toArray()).toEqual([10 + PLANE_GUN_PIVOTS[0][0], 2 + PLANE_GUN_PIVOTS[0][1], 5 + PLANE_GUN_PIVOTS[0][2]].map((v) => expect.closeTo(v, 3)))
    expect(right.origin.x).toBeCloseTo(10 + PLANE_GUN_PIVOTS[1][0], 3)
    plane.dispose()
  })

  it('swings the ring gun all the way round and ends its barrel at the muzzle', () => {
    const plane = new PlaneModel(1)
    plane.update([0, 0, 0], IDENTITY, 0, [0, 4], { yaw: 0, pitch: 0, shots: 0, range: 0, flags: 0 }, ARMS)
    expect(plane.barrel().direction.z).toBeCloseTo(-1, 3)
    plane.update([0, 0, 0], IDENTITY, 0, [0, 4], { yaw: Math.PI, pitch: 0.5, shots: 0, range: 0, flags: 0 }, ARMS)
    const { origin, direction } = plane.barrel()
    expect(direction.z).toBeGreaterThan(0.8)
    expect(direction.y).toBeCloseTo(Math.sin(0.5), 3)
    expect(plane.muzzle().distanceTo(origin)).toBeCloseTo(PLANE_REAR_LENGTH, 4)
    const bounds = boxOf(named(plane, 'gun'))
    expect(bounds.max.z).toBeCloseTo(plane.muzzle().z, 1)
    expect(plane.gunner).toBe(4)
    plane.dispose()
  })

  it('spins the propeller with a pilot and slows it on the blip switch', () => {
    const plane = new PlaneModel(1)
    const prop = named(plane, 'prop')
    plane.update([0, 0, 0], IDENTITY, 0, [0, 0], null, ARMS)
    expect(prop.rotation.z).toBe(0)
    const advance = (flags: number) => {
      // Long enough for it to run up or down to its new speed.
      for (let k = 0; k < 75; k++) {
        wait(20)
        plane.update([0, 0, 0], IDENTITY, 0, [3, 0], null, { ...ARMS, flags })
      }
      const before = prop.rotation.z
      wait(20)
      plane.update([0, 0, 0], IDENTITY, 0, [3, 0], null, { ...ARMS, flags })
      return prop.rotation.z - before
    }
    const running = advance(0)
    const cut = advance(ARMS_CUT)
    expect(running).toBeGreaterThan(0)
    expect(cut).toBeLessThan(running * 0.7)
    plane.dispose()
  })

  it('shows the bombs still on the rack', () => {
    const plane = new PlaneModel(1)
    plane.update([0, 0, 0], IDENTITY, 0, [0, 0], null, { ...ARMS, bombs: 1 })
    expect([1, 2, 3, 4].map((k) => named(plane, `bomb_${k}`).visible)).toEqual([true, false, false, false])
    plane.dispose()
  })

  it('puts the rudder over and the ailerons opposite each other in a turn', () => {
    const plane = new PlaneModel(1)
    // Turning left and rolling the right wing up, a frame at a time.
    for (let k = 0; k < 30; k++) {
      wait(8)
      const q = new Quat().setFromEuler(0, k * 0.02, k * 0.03, 'YXZ')
      plane.update([0, 10, 0], [q.x, q.y, q.z, q.w], 0, [3, 0], null, ARMS)
    }
    expect(plane.rates.y).toBeGreaterThan(0.5)
    expect(plane.rates.z).toBeGreaterThan(0.5)
    // The rudder's trailing edge swings left (towards -X), the right aileron drops and the left one rises.
    expect(named(plane, 'rudder').rotation.y).toBeLessThan(-0.05)
    expect(named(plane, 'aileron_R').rotation.x).toBeGreaterThan(0.05)
    expect(named(plane, 'aileron_L').rotation.x).toBeLessThan(-0.05)
    plane.dispose()
  })
})
