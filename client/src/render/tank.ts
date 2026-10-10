import * as THREE from 'three'
import {
  CAR_SEATS, TANK_AXLES_Z, TANK_BARREL_LENGTH, TANK_HALF_EXTENTS, TANK_HARD_POINT_Y, TANK_SUSPENSION_REST, TANK_TRACK_HALF,
  TANK_TURRET_PIVOT, TANK_WHEEL_RADIUS,
} from '@/sim/consts'
import { wrapAngle, type Turret } from '@/net/codec'
import { colorFor } from './character'
import type { VehicleModel } from './car'

const [HALF_X, HALF_Y, HALF_Z] = TANK_HALF_EXTENTS
const RECOIL_STROKE = 0.5
const RECOIL_SECONDS = 0.35
const WHEEL_Y = TANK_HARD_POINT_Y - TANK_SUSPENSION_REST * 0.7
/** Each seat's occupant, in the hull frame (driver) or the turret frame (gunner). */
const DRIVER_HEAD: readonly [number, number, number] = [0, HALF_Y + 0.12, -1.5]
const GUNNER_HEAD: readonly [number, number, number] = [-0.35, 0.62, 0.25]

/**
 * A chunky toy tank: a hull on two track runs with three road wheels each, and a turret that
 * swings on the hull with a barrel that nods. The driver's head shows in a hatch, the gunner's on
 * the turret. The hull is the physics box; wheels roll with the movement of each track (so turning
 * on the spot rolls them in opposite directions).
 */
export class TankModel implements VehicleModel {
  readonly root = new THREE.Group()
  private readonly turret = new THREE.Group()
  private readonly gun = new THREE.Group()
  private readonly spinners: { group: THREE.Group; side: number }[] = []
  private readonly heads: THREE.Mesh[] = []
  private roll = { left: 0, right: 0 }
  private placed = false
  /** `performance.now()` of the last shot, for the barrel's recoil stroke. */
  private firedAt = -Infinity
  private lastYaw = 0
  private readonly last = new THREE.Vector3()
  private readonly forward = new THREE.Vector3()
  private readonly pivot = new THREE.Vector3()
  private readonly aim = new THREE.Vector3()

  constructor(id: number) {
    const olive = new THREE.MeshToonMaterial({ color: [0x6b8e23, 0x8a7f4a, 0x5f7d5a, 0x7a6f3d][id % 4]! })
    const dark = new THREE.MeshToonMaterial({ color: 0x2b2b2b })
    const metal = new THREE.MeshToonMaterial({ color: 0x4d5448 })

    const part = (parent: THREE.Object3D, geometry: THREE.BufferGeometry, material: THREE.Material, x: number, y: number, z: number) => {
      const mesh = new THREE.Mesh(geometry, material)
      mesh.position.set(x, y, z)
      mesh.castShadow = true
      parent.add(mesh)
      return mesh
    }
    // Hull, a sloped glacis plate in front, and the two track runs.
    part(this.root, new THREE.BoxGeometry(HALF_X * 2 - 0.5, HALF_Y * 2, HALF_Z * 2), olive, 0, 0, 0)
    part(this.root, new THREE.BoxGeometry(HALF_X * 2 - 0.5, 0.25, 1.4), olive, 0, HALF_Y - 0.2, -HALF_Z - 0.2).rotation.x = 0.5
    for (const side of [-1, 1]) {
      part(this.root, new THREE.BoxGeometry(0.62, 0.9, HALF_Z * 2 + 0.5), dark, side * (TANK_TRACK_HALF + 0.05), WHEEL_Y + 0.15, 0)
      part(this.root, new THREE.BoxGeometry(0.7, 0.12, HALF_Z * 2 + 0.7), metal, side * (TANK_TRACK_HALF + 0.05), WHEEL_Y + 0.65, 0)
    }
    // Road wheels, which roll with their track.
    const wheelGeometry = new THREE.CylinderGeometry(TANK_WHEEL_RADIUS, TANK_WHEEL_RADIUS, 0.2, 14)
    wheelGeometry.rotateZ(Math.PI / 2)
    const hubGeometry = new THREE.BoxGeometry(0.24, TANK_WHEEL_RADIUS * 1.2, 0.09)
    const light = new THREE.MeshToonMaterial({ color: 0xbbbbbb })
    for (const z of TANK_AXLES_Z) {
      for (const side of [-1, 1]) {
        const spinner = new THREE.Group()
        spinner.position.set(side * (TANK_TRACK_HALF + 0.38), WHEEL_Y, z)
        spinner.add(new THREE.Mesh(wheelGeometry, metal), new THREE.Mesh(hubGeometry, light))
        this.root.add(spinner)
        this.spinners.push({ group: spinner, side })
      }
    }

    // Turret ring, with the turret body turning on it and the gun nodding in the turret.
    this.turret.position.set(...TANK_TURRET_PIVOT)
    this.root.add(this.turret)
    part(this.turret, new THREE.BoxGeometry(1.7, 0.7, 2.1), olive, 0, 0, 0.1)
    part(this.turret, new THREE.BoxGeometry(1.5, 0.2, 0.5), olive, 0, 0.1, -1.1).rotation.x = -0.4
    this.turret.add(this.gun)
    const barrel = new THREE.CylinderGeometry(0.12, 0.14, TANK_BARREL_LENGTH - 0.5, 10)
    barrel.rotateX(Math.PI / 2)
    part(this.gun, barrel, metal, 0, 0, -(TANK_BARREL_LENGTH + 0.5) / 2)
    const brake = new THREE.CylinderGeometry(0.2, 0.2, 0.35, 10)
    brake.rotateX(Math.PI / 2)
    part(this.gun, brake, dark, 0, 0, -TANK_BARREL_LENGTH + 0.17)

    const headGeometry = new THREE.SphereGeometry(0.2, 12, 10)
    for (let seat = 0; seat < CAR_SEATS; seat++) {
      const head = new THREE.Mesh(headGeometry, new THREE.MeshToonMaterial({ color: 0xffffff }))
      head.visible = false
      head.castShadow = true
      this.heads.push(head)
      if (seat === 0) {
        head.position.set(...DRIVER_HEAD)
        this.root.add(head)
      } else if (seat === 1) {
        head.position.set(...GUNNER_HEAD)
        this.turret.add(head)
      }
    }
  }

  /**
   * Places the tank and animates it. `turret` is the gun's angles in the hull frame; `seats` the
   * occupant ids, 0 for empty (seat 0 drives, seat 1 works the gun). `steer` is unused: a tank
   * turns by the difference between its tracks, which is read off the movement.
   */
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    _steer: number,
    seats: readonly number[],
    turret?: Turret | null,
  ) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    this.forward.set(0, 0, -1).applyQuaternion(this.root.quaternion)
    const yaw = Math.atan2(-this.forward.x, -this.forward.z)
    if (this.placed) {
      const ahead = this.forward.dot(this.root.position.clone().sub(this.last))
      const turned = wrapAngle(yaw - this.lastYaw) * TANK_TRACK_HALF
      // Turning left (+yaw) holds the left track back and drives the right one on.
      this.roll.left -= (ahead - turned) / TANK_WHEEL_RADIUS
      this.roll.right -= (ahead + turned) / TANK_WHEEL_RADIUS
    }
    this.last.copy(this.root.position)
    this.lastYaw = yaw
    this.placed = true
    for (const { group, side } of this.spinners) group.rotation.x = side < 0 ? this.roll.left : this.roll.right

    this.turret.rotation.y = turret?.yaw ?? 0
    this.gun.rotation.x = turret?.pitch ?? 0
    // The barrel slides back and eases home over a third of a second.
    const since = (performance.now() - this.firedAt) / 1000
    this.gun.position.z = since < RECOIL_SECONDS ? RECOIL_STROKE * (1 - since / RECOIL_SECONDS) ** 2 : 0
    this.heads.forEach((head, seat) => {
      const occupant = seats[seat] ?? 0
      head.visible = occupant !== 0
      if (occupant !== 0) (head.material as THREE.MeshToonMaterial).color.setHex(colorFor(occupant))
    })
  }

  /** A shell was just fired: the barrel kicks back. */
  recoil() {
    this.firedAt = performance.now()
  }

  /** The barrel in world space as of the last `update`: the turret ring and the unit aim direction. */
  barrel() {
    this.root.updateMatrixWorld(true)
    this.gun.getWorldPosition(this.pivot)
    this.aim.set(0, 0, -1).transformDirection(this.gun.matrixWorld)
    return { origin: this.pivot.clone(), direction: this.aim.clone() }
  }

  dispose() {
    this.root.traverse((object) => {
      if (object instanceof THREE.Mesh) object.geometry.dispose()
    })
    this.root.removeFromParent()
  }
}
