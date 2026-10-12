import { describe, expect, it } from 'vitest'
import * as THREE from 'three'
import { CarWear, sideOf } from '@/render/carwear'
import { Debris } from '@/render/debris'
import { DAMAGE_FRONT, DAMAGE_LEFT, DAMAGE_REAR, DAMAGE_RIGHT } from '@/net/codec'

const HALF = [1, 0.4, 2] as const

/** A box of a car: a finely divided painted body, a headlight, a windscreen and four wheels. */
function model() {
  const root = new THREE.Group()
  const car = new THREE.Group()
  root.add(car)
  const body = new THREE.Group()
  body.name = 'body'
  car.add(body)
  const paint = new THREE.MeshToonMaterial({ color: 0xff0000, name: 'paint' })
  const lamp = new THREE.MeshBasicMaterial({ name: 'lamp' })
  const glass = new THREE.MeshToonMaterial({ name: 'glass' })
  const shell = new THREE.Mesh(new THREE.BoxGeometry(2, 0.8, 4, 8, 4, 16), paint)
  const light = new THREE.Mesh(new THREE.BoxGeometry(0.3, 0.1, 0.05).translate(0, 0, -2), lamp)
  const screen = new THREE.Mesh(new THREE.BoxGeometry(1.6, 0.4, 0.05).translate(0, 0.5, -0.8), glass)
  body.add(shell, light, screen)
  for (const name of ['wheel_FL', 'wheel_FR', 'wheel_RL', 'wheel_RR']) {
    const wheel = new THREE.Mesh(new THREE.CylinderGeometry(0.4, 0.4, 0.3), paint)
    wheel.name = name
    wheel.position.set(name.endsWith('L') ? -0.9 : 0.9, -0.4, name[6] === 'F' ? -1.3 : 1.3)
    car.add(wheel)
  }
  const wear = new CarWear(root, car, HALF, 0.4, paint)
  return { root, car, shell, light, screen, paint, wear }
}

const damage = (sides: [number, number, number, number], wheels: [number, number, number, number] = [0, 0, 0, 0]) => [...sides, ...wheels]
const still = new THREE.Vector3()

/** How far each vertex of `mesh` has moved from where the shared geometry has it. */
function moved(mesh: THREE.Mesh, shared: THREE.BufferGeometry) {
  const now = mesh.geometry.getAttribute('position')
  const was = shared.getAttribute('position')
  const out: { at: THREE.Vector3; by: number }[] = []
  for (let i = 0; i < was.count; i++) {
    const at = new THREE.Vector3().fromBufferAttribute(was, i)
    out.push({ at, by: at.distanceTo(new THREE.Vector3().fromBufferAttribute(now, i)) })
  }
  return out
}

describe('CarWear', () => {
  it('names the side a point is on', () => {
    expect(sideOf(new THREE.Vector3(0, 0, -2), HALF)).toBe(DAMAGE_FRONT)
    expect(sideOf(new THREE.Vector3(0.2, 0, 1.9), HALF)).toBe(DAMAGE_REAR)
    expect(sideOf(new THREE.Vector3(-1, 0, 0.5), HALF)).toBe(DAMAGE_LEFT)
    expect(sideOf(new THREE.Vector3(1, 0, -0.5), HALF)).toBe(DAMAGE_RIGHT)
  })

  it('dents the body where the crash event says, and nowhere far from it', () => {
    const { shell, wear } = model()
    const shared = shell.geometry
    wear.noteImpact(new THREE.Vector3(0.6, 0, -2), 0)
    wear.update(damage([120, 0, 0, 0]), 100, null, still, 10)
    expect(shell.geometry).not.toBe(shared)
    const offsets = moved(shell, shared)
    const near = offsets.filter(({ at }) => at.distanceTo(new THREE.Vector3(0.6, 0, -2)) < 0.3)
    const far = offsets.filter(({ at }) => at.z > 0)
    expect(Math.max(...near.map((o) => o.by))).toBeGreaterThan(0.08)
    expect(Math.max(...far.map((o) => o.by))).toBe(0)
  })

  it('dents mid-side when no crash event comes', () => {
    const { shell, wear } = model()
    const shared = shell.geometry
    wear.update(damage([0, 0, 180, 0]), 100, null, still, 0)
    expect(shell.geometry).toBe(shared)
    wear.update(damage([0, 0, 180, 0]), 100, null, still, 500)
    const left = moved(shell, shared).filter(({ at }) => at.x < -0.9 && Math.abs(at.z) < 0.3)
    expect(Math.max(...left.map((o) => o.by))).toBeGreaterThan(0.12)
    expect(Math.min(...left.map((o) => o.by))).toBeGreaterThan(0.02)
  })

  it('breaks the lights and cracks the glass of a battered front', () => {
    const { light, screen, wear } = model()
    const lamp = light.material
    wear.update(damage([60, 0, 0, 0]), 100, null, still, 1000)
    expect(light.material).toBe(lamp)
    wear.update(damage([130, 0, 0, 0]), 100, null, still, 2000)
    expect(light.material).not.toBe(lamp)
    expect((screen.material as THREE.Material).name).toBe('cracked')
  })

  it('throws off a bumper and a lost wheel, then comes back whole', () => {
    const { car, shell, wear } = model()
    const shared = shell.geometry
    const sharedTriangles = shared.index!.count
    const debris = new Debris()
    wear.update(damage([200, 0, 0, 0], [0, 0, 0, 255]), 40, debris, new THREE.Vector3(0, 0, -10), 5000)
    expect(wear.state.gone).toContain('bumper_F')
    expect(wear.state.wheelsGone).toEqual([false, false, false, true])
    expect(car.getObjectByName('wheel_RR')!.visible).toBe(false)
    expect(shell.geometry.index!.count).toBeLessThan(sharedTriangles)
    expect(debris.count).toBe(2)
    // The pieces fly, fall and settle.
    for (let i = 0; i < 300; i++) debris.update(1 / 60)
    expect(debris.count).toBe(2)
    // A fresh car after a respawn.
    wear.update(damage([0, 0, 0, 0]), 100, debris, still, 9000)
    expect(shell.geometry).toBe(shared)
    expect(car.getObjectByName('wheel_RR')!.visible).toBe(true)
    expect(wear.state).toEqual({ gone: [], wheelsGone: [false, false, false, false], owned: 0 })
    for (let i = 0; i < 600; i++) debris.update(1 / 60)
    expect(debris.count).toBe(0)
  })

  it('leans a bent wheel out and dulls the paint', () => {
    const { car, paint, wear } = model()
    const fresh = paint.color.clone()
    wear.update(damage([0, 0, 0, 0], [128, 0, 0, 200]), 100, null, still, 0)
    expect(car.getObjectByName('wheel_FL')!.rotation.z).toBeGreaterThan(0.1)
    expect(car.getObjectByName('wheel_RR')!.rotation.z).toBeLessThan(-0.2)
    expect(car.getObjectByName('wheel_FR')!.rotation.z).toBeCloseTo(0)
    expect(paint.color.equals(fresh)).toBe(false)
    wear.update(damage([0, 0, 0, 0], [128, 0, 0, 200]), 0, null, still, 0)
    expect(paint.color.r).toBeLessThan(0.2)
  })
})

describe('Debris', () => {
  it('drops the oldest piece when full', () => {
    const debris = new Debris(2)
    for (let i = 0; i < 3; i++) debris.throw(new THREE.Object3D(), still, still, 0.1)
    expect(debris.count).toBe(2)
  })
})
