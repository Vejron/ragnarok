import { Group, Material, Mesh, boxGeometry, cylinderGeometry, sphereGeometry } from '@ragnarok/engine'
import { toonMaterial } from './toon'
import type { VehicleModel } from './car'
import { MISSILE_HALF_EXTENTS } from '@/sim/consts'

const [, , HZ] = MISSILE_HALF_EXTENTS
/** The body's radius, metres. */
const BODY_R = 0.17

// Shared by every missile: parts only ever move, so none of these is disposed.
const BODY = cylinderGeometry(BODY_R, BODY_R, HZ * 1.6, 12)
BODY.rotateX(Math.PI / 2)
const NOSE = cylinderGeometry(0.01, BODY_R, 0.45, 12)
NOSE.rotateX(-Math.PI / 2)
const FIN = boxGeometry(0.03, 0.55, 0.4)
const CANARD = boxGeometry(0.03, 0.3, 0.18)
const NOZZLE = cylinderGeometry(BODY_R * 0.7, BODY_R * 0.9, 0.2, 10)
NOZZLE.rotateX(Math.PI / 2)
const FLAME = sphereGeometry(0.22, 8, 6)

/**
 * A submarine's guided missile: a slim grey body with a pointed nose, four small canards behind it and
 * four fins at the tail, and a flame in its nozzle while its motor burns (while its flier still has it).
 */
export class MissileModel implements VehicleModel {
  readonly root = new Group()
  private readonly skin = toonMaterial({ color: 0x9aa0a6 })
  private readonly band = toonMaterial({ color: 0xc0392b })
  private readonly dark = toonMaterial({ color: 0x1b1d20 })
  private readonly fire = new Material({ color: 0xffc46b, emissive: 0xff8a2a, lit: false })
  private readonly flame: Mesh

  constructor(_id: number) {
    const part = (geometry: typeof BODY, material: Material, x: number, y: number, z: number) => {
      const mesh = new Mesh(geometry, material)
      mesh.position.set(x, y, z)
      mesh.castShadow = true
      this.root.add(mesh)
      return mesh
    }
    part(BODY, this.skin, 0, 0, 0)
    part(NOSE, this.band, 0, 0, -HZ * 0.8 - 0.22)
    part(NOZZLE, this.dark, 0, 0, HZ * 0.8 + 0.1)
    for (const turn of [0, Math.PI / 2]) {
      part(FIN, this.dark, 0, 0, HZ * 0.6).rotation.z = turn
      part(CANARD, this.dark, 0, 0, -HZ * 0.35).rotation.z = turn
    }
    this.flame = part(FLAME, this.fire, 0, 0, HZ * 0.8 + 0.3)
    this.flame.castShadow = false
  }

  /** Places the missile. `seats[0]` is its flier, or 0 once the link is lost and its motor has stopped. */
  update(position: readonly [number, number, number], rotation: readonly [number, number, number, number], _steer: number, seats: readonly number[]) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    const burning = (seats[0] ?? 0) !== 0
    this.flame.visible = burning
    if (burning) this.flame.scale.set(1, 1, 1.6 + Math.random() * 0.8)
  }

  /** Frees what this missile owns; the shared geometry stays. */
  dispose() {
    for (const material of [this.skin, this.band, this.dark, this.fire]) material.dispose()
    this.root.removeFromParent()
  }
}
