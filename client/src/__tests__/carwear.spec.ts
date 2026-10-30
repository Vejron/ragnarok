import { describe, expect, it } from 'vitest'
import { Geometry, Group, Material, Mesh, Node, boxGeometry, cylinderGeometry, Vector3 } from '@ragnarok/engine'
import { CarWear, sideOf } from '@/render/carwear'
import { Debris } from '@/render/debris'
import { DAMAGE_FRONT, DAMAGE_LEFT, DAMAGE_REAR, DAMAGE_RIGHT } from '@/net/codec'

const HALF = [1, 0.4, 2] as const

/** A box of a car: a finely divided painted body, a headlight, a windscreen and four wheels. */
function model() {
  const root = new Group()
  const car = new Group()
  root.add(car)
  const body = new Group()
  body.name = 'body'
  car.add(body)
  const paint = new Material({ color: 0xff0000, name: 'paint' })
  const lamp = new Material({ name: 'lamp', lit: false })
  const glass = new Material({ name: 'glass' })
  const shell = new Mesh(subdivided(2, 0.8, 4, 8, 4, 16), paint)
  const light = new Mesh(boxGeometry(0.3, 0.1, 0.05).translate(0, 0, -2), lamp)
  const screen = new Mesh(boxGeometry(1.6, 0.4, 0.05).translate(0, 0.5, -0.8), glass)
  body.add(shell, light, screen)
  for (const name of ['wheel_FL', 'wheel_FR', 'wheel_RL', 'wheel_RR']) {
    const wheel = new Mesh(cylinderGeometry(0.4, 0.4, 0.3), paint)
    wheel.name = name
    wheel.position.set(name.endsWith('L') ? -0.9 : 0.9, -0.4, name[6] === 'F' ? -1.3 : 1.3)
    car.add(wheel)
  }
  const wear = new CarWear(root, car, HALF, 0.4, paint)
  return { root, car, shell, light, screen, paint, wear }
}

/** A box whose faces are cut into a grid of quads, so a dent has vertices to move. */
function subdivided(w: number, h: number, d: number, sx: number, sy: number, sz: number): Geometry {
  const positions: number[] = []
  const indices: number[] = []
  // Each face: origin corner, the two edge vectors, and how many cells along each.
  const faces: [number[], number[], number[], number, number][] = [
    [[-w / 2, -h / 2, d / 2], [w, 0, 0], [0, h, 0], sx, sy],
    [[w / 2, -h / 2, -d / 2], [-w, 0, 0], [0, h, 0], sx, sy],
    [[w / 2, -h / 2, d / 2], [0, 0, -d], [0, h, 0], sz, sy],
    [[-w / 2, -h / 2, -d / 2], [0, 0, d], [0, h, 0], sz, sy],
    [[-w / 2, h / 2, d / 2], [w, 0, 0], [0, 0, -d], sx, sz],
    [[-w / 2, -h / 2, -d / 2], [w, 0, 0], [0, 0, d], sx, sz],
  ]
  for (const [o, u, v, nu, nv] of faces) {
    const base = positions.length / 3
    for (let j = 0; j <= nv; j++) {
      for (let i = 0; i <= nu; i++) {
        positions.push(o[0]! + (u[0]! * i) / nu + (v[0]! * j) / nv, o[1]! + (u[1]! * i) / nu + (v[1]! * j) / nv, o[2]! + (u[2]! * i) / nu + (v[2]! * j) / nv)
      }
    }
    for (let j = 0; j < nv; j++) {
      for (let i = 0; i < nu; i++) {
        const a = base + j * (nu + 1) + i
        indices.push(a, a + 1, a + nu + 2, a, a + nu + 2, a + nu + 1)
      }
    }
  }
  const geometry = new Geometry({ positions: new Float32Array(positions), indices: Uint32Array.from(indices) })
  geometry.computeNormals()
  return geometry
}

const damage = (sides: [number, number, number, number], wheels: [number, number, number, number] = [0, 0, 0, 0]) => [...sides, ...wheels]
const still = new Vector3()

/** How far each vertex of `mesh` has moved from where the shared geometry has it. */
function moved(mesh: Mesh, shared: Geometry) {
  const now = mesh.geometry.positions
  const was = shared.positions
  const out: { at: Vector3; by: number }[] = []
  for (let i = 0; i < was.length / 3; i++) {
    const at = new Vector3().fromArray(was, i * 3)
    out.push({ at, by: at.distanceTo(new Vector3().fromArray(now, i * 3)) })
  }
  return out
}

describe('CarWear', () => {
  it('names the side a point is on', () => {
    expect(sideOf(new Vector3(0, 0, -2), HALF)).toBe(DAMAGE_FRONT)
    expect(sideOf(new Vector3(0.2, 0, 1.9), HALF)).toBe(DAMAGE_REAR)
    expect(sideOf(new Vector3(-1, 0, 0.5), HALF)).toBe(DAMAGE_LEFT)
    expect(sideOf(new Vector3(1, 0, -0.5), HALF)).toBe(DAMAGE_RIGHT)
  })

  it('dents the body where the crash event says, and nowhere far from it', () => {
    const { shell, wear } = model()
    const shared = shell.geometry
    wear.noteImpact(new Vector3(0.6, 0, -2), 0)
    wear.update(damage([120, 0, 0, 0]), 100, null, still, 10)
    expect(shell.geometry).not.toBe(shared)
    const offsets = moved(shell, shared)
    const near = offsets.filter(({ at }) => at.distanceTo(new Vector3(0.6, 0, -2)) < 0.3)
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
    expect(screen.material.name).toBe('cracked')
  })

  it('throws off a bumper and a lost wheel, then comes back whole', () => {
    const { car, shell, wear } = model()
    const shared = shell.geometry
    const sharedTriangles = shared.indexCount
    const debris = new Debris()
    wear.update(damage([200, 0, 0, 0], [0, 0, 0, 255]), 40, debris, new Vector3(0, 0, -10), 5000)
    expect(wear.state.gone).toContain('bumper_F')
    expect(wear.state.wheelsGone).toEqual([false, false, false, true])
    expect(car.getObjectByName('wheel_RR')!.visible).toBe(false)
    expect(shell.geometry.indexCount).toBeLessThan(sharedTriangles)
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
    expect(paint.color.r === fresh.r && paint.color.g === fresh.g && paint.color.b === fresh.b).toBe(false)
    wear.update(damage([0, 0, 0, 0], [128, 0, 0, 200]), 0, null, still, 0)
    expect(paint.color.r).toBeLessThan(0.2)
  })
})

describe('Debris', () => {
  it('drops the oldest piece when full', () => {
    const debris = new Debris(2)
    for (let i = 0; i < 3; i++) debris.throw(new Node(), still, still, 0.1)
    expect(debris.count).toBe(2)
  })
})
