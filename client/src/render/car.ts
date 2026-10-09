import * as THREE from 'three'
import {
  CAR_AXLE_Z, CAR_HALF_EXTENTS, CAR_HARD_POINT_Y, CAR_MAX_STEER, CAR_SEATS, CAR_SUSPENSION_REST, CAR_TRACK_HALF,
  CAR_WHEEL_RADIUS,
} from '@/sim/consts'
import type { Turret } from '@/net/codec'
import { colorFor } from './stickman'

/** What `Game` needs from a vehicle's model, whatever the kind. */
export interface VehicleModel {
  readonly root: THREE.Object3D
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    steer: number,
    seats: readonly number[],
    turret?: Turret | null,
  ): void
  dispose(): void
}

const [HALF_X, , HALF_Z] = CAR_HALF_EXTENTS
/** Wheel centre height in the chassis frame once the suspension has settled under the car's weight. */
const WHEEL_Y = CAR_HARD_POINT_Y - CAR_SUSPENSION_REST * 0.7
/** Where each seat's occupant sits in the chassis frame: driver front left, then front right, then the back row. */
const SEATS: readonly [number, number, number][] = [
  [-0.4, 0.58, -0.35],
  [0.4, 0.58, -0.35],
  [-0.4, 0.58, 0.75],
  [0.4, 0.58, 0.75],
]

const BODY_COLORS = [0xff9f1c, 0x2ec4b6, 0xe71d36, 0x5aa9e6, 0xb5e48c, 0xf15bb5, 0xffd23f, 0x9d4edd]

/**
 * A goofy open-top toy car. The chassis is the physics box; wheels roll with the speed the
 * interpolator reports and the front pair steers. Passengers show as coloured heads, one per seat.
 */
export class CarModel implements VehicleModel {
  readonly root = new THREE.Group()
  private readonly frontPivots: THREE.Group[] = []
  private readonly spinners: THREE.Group[] = []
  private readonly heads: THREE.Mesh[] = []
  private roll = 0
  private placed = false
  private readonly last = new THREE.Vector3()
  private readonly forward = new THREE.Vector3()

  constructor(id: number) {
    const paint = new THREE.MeshToonMaterial({ color: BODY_COLORS[id % BODY_COLORS.length]! })
    const dark = new THREE.MeshToonMaterial({ color: 0x222222 })
    const glass = new THREE.MeshToonMaterial({ color: 0xbfe9ff })

    const part = (geometry: THREE.BufferGeometry, material: THREE.Material, x: number, y: number, z: number) => {
      const mesh = new THREE.Mesh(geometry, material)
      mesh.position.set(x, y, z)
      mesh.castShadow = true
      this.root.add(mesh)
      return mesh
    }
    // Tub, front bumper, hood and a windscreen: all in the chassis frame, -Z forward.
    part(new THREE.BoxGeometry(HALF_X * 2, 0.5, HALF_Z * 2), paint, 0, 0.0, 0)
    part(new THREE.BoxGeometry(HALF_X * 2 + 0.1, 0.22, 0.2), dark, 0, -0.1, -HALF_Z)
    part(new THREE.BoxGeometry(HALF_X * 2 - 0.1, 0.18, 1.3), paint, 0, 0.32, -1.4)
    part(new THREE.BoxGeometry(HALF_X * 2 - 0.2, 0.5, 0.06), glass, 0, 0.5, -0.85).rotation.x = -0.35
    for (const x of [-0.45, 0.45]) {
      part(new THREE.SphereGeometry(0.12, 8, 6), new THREE.MeshBasicMaterial({ color: 0xfff6a8 }), x, 0.1, -HALF_Z - 0.02)
    }

    const wheelGeometry = new THREE.CylinderGeometry(CAR_WHEEL_RADIUS, CAR_WHEEL_RADIUS, 0.28, 14)
    wheelGeometry.rotateZ(Math.PI / 2)
    const hubGeometry = new THREE.BoxGeometry(0.3, CAR_WHEEL_RADIUS * 1.2, 0.08)
    const hub = new THREE.MeshToonMaterial({ color: 0xdddddd })
    for (const front of [true, false]) {
      for (const side of [-1, 1]) {
        const pivot = new THREE.Group()
        pivot.position.set(side * (CAR_TRACK_HALF + 0.05), WHEEL_Y, front ? -CAR_AXLE_Z : CAR_AXLE_Z)
        const spinner = new THREE.Group()
        const tyre = new THREE.Mesh(wheelGeometry, dark)
        tyre.castShadow = true
        spinner.add(tyre, new THREE.Mesh(hubGeometry, hub))
        pivot.add(spinner)
        this.root.add(pivot)
        this.spinners.push(spinner)
        if (front) this.frontPivots.push(pivot)
      }
    }

    const headGeometry = new THREE.SphereGeometry(0.2, 12, 10)
    for (let seat = 0; seat < CAR_SEATS; seat++) {
      const head = new THREE.Mesh(headGeometry, new THREE.MeshToonMaterial({ color: 0xffffff }))
      head.position.set(...SEATS[seat]!)
      head.castShadow = true
      head.visible = false
      this.root.add(head)
      this.heads.push(head)
    }
  }

  /**
   * Places the car and animates it. The wheels roll by however far the car moved along its nose
   * since the last call (so reversing spins them backwards), `steer` is the front wheel fraction
   * of full lock and `seats` the occupant ids (0 for an empty seat).
   */
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    steer: number,
    seats: readonly number[],
  ) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    this.forward.set(0, 0, -1).applyQuaternion(this.root.quaternion)
    if (this.placed) this.roll -= this.forward.dot(this.root.position.clone().sub(this.last)) / CAR_WHEEL_RADIUS
    this.last.copy(this.root.position)
    this.placed = true
    for (const spinner of this.spinners) spinner.rotation.x = this.roll
    // Turning right (positive) is a clockwise yaw, which is a negative rotation about +Y.
    for (const pivot of this.frontPivots) pivot.rotation.y = -steer * CAR_MAX_STEER
    this.heads.forEach((head, seat) => {
      const occupant = seats[seat] ?? 0
      head.visible = occupant !== 0
      if (occupant !== 0) (head.material as THREE.MeshToonMaterial).color.setHex(colorFor(occupant))
    })
  }

  dispose() {
    this.root.traverse((object) => {
      if (object instanceof THREE.Mesh) object.geometry.dispose()
    })
    this.root.removeFromParent()
  }
}
