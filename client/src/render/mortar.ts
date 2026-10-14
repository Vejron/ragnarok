import * as THREE from 'three'
import { toonMaterial } from './toon'
import { colorFor } from './character'
import type { VehicleModel } from './car'
import type { Turret } from '@/net/codec'
import { MORTAR_BARREL_LENGTH, MORTAR_GUN_PIVOT, MORTAR_TRACK_HALF, MORTAR_WHEEL_RADIUS } from '@/sim/consts'

/** Fore and aft of each axle, metres (`axles` in `spec.rs`). */
const AXLE_Z = 0.9
/** Wheel centres under the hull (the hard point less the suspension's rest). */
const WHEEL_Y = -0.55
/** The barrel slides back and eases home over this long, seconds and metres. */
const RECOIL_SECONDS = 0.6
const RECOIL_STROKE = 0.35
const PAINT = 0x5d6b3c
const STEEL = 0x4a4d50

// Shared by every mortar: parts only ever move, so none of these is disposed.
const HULL = new THREE.BoxGeometry(2.0, 1.0, 3.0)
const TONGUE = new THREE.BoxGeometry(0.2, 0.2, 1.2)
const BASE = new THREE.CylinderGeometry(0.42, 0.5, 0.25, 14)
const BARREL = new THREE.CylinderGeometry(0.09, 0.12, MORTAR_BARREL_LENGTH, 10)
// The barrel's own axis runs along -Z from the mount, so it lies along the gun's aim at zero angles.
BARREL.rotateX(-Math.PI / 2)
BARREL.translate(0, 0, -MORTAR_BARREL_LENGTH / 2)
const WHEEL = new THREE.CylinderGeometry(MORTAR_WHEEL_RADIUS, MORTAR_WHEEL_RADIUS, 0.3, 14)
// Turned onto its axle, which runs across the mortar, so the wheel spins about X.
WHEEL.rotateZ(Math.PI / 2)
const HEAD = new THREE.SphereGeometry(0.2, 12, 10)

const WHEEL_SPOTS: [number, number][] = [
  [-MORTAR_TRACK_HALF, -AXLE_Z],
  [MORTAR_TRACK_HALF, -AXLE_Z],
  [-MORTAR_TRACK_HALF, AXLE_Z],
  [MORTAR_TRACK_HALF, AXLE_Z],
]

/**
 * The field mortar: a towed two-axle gun carriage with a short steel tongue at the front, a base plate
 * and a tube on a mount. Its gunner stands behind the mount. The mount turns with the gun's traverse and
 * the tube lifts with its elevation, so the barrel's line is the server's aim.
 */
export class MortarModel implements VehicleModel {
  readonly root = new THREE.Group()
  /** Turns with the traverse: the hull-relative yaw. */
  private readonly mount = new THREE.Group()
  /** Lifts with the elevation, about the mount's trunnions. */
  private readonly elevation = new THREE.Group()
  private readonly tube: THREE.Mesh
  private readonly head: THREE.Mesh
  private readonly spinners: THREE.Object3D[] = []
  private readonly paint = toonMaterial({ color: PAINT })
  private readonly steel = toonMaterial({ color: STEEL })
  private readonly headMaterial = toonMaterial({ color: 0xffffff })
  private roll = 0
  private placed = false
  /** `performance.now()` of the last shot, for the barrel's recoil stroke. */
  private firedAt = -Infinity
  private readonly last = new THREE.Vector3()
  private readonly forward = new THREE.Vector3()
  private readonly pivot = new THREE.Vector3()
  private readonly aim = new THREE.Vector3()

  constructor(_id: number) {
    const hull = new THREE.Mesh(HULL, this.paint)
    hull.castShadow = true
    this.root.add(hull)
    const tongue = new THREE.Mesh(TONGUE, this.steel)
    tongue.position.set(0, -0.1, -2.1)
    this.root.add(tongue)

    for (const [x, z] of WHEEL_SPOTS) {
      const wheel = new THREE.Mesh(WHEEL, this.steel)
      wheel.position.set(x, WHEEL_Y, z)
      wheel.castShadow = true
      this.spinners.push(wheel)
      this.root.add(wheel)
    }

    this.mount.position.set(...MORTAR_GUN_PIVOT)
    const base = new THREE.Mesh(BASE, this.steel)
    base.position.set(0, -0.6, 0)
    this.root.add(base)
    this.root.add(this.mount)
    this.mount.add(this.elevation)
    this.tube = new THREE.Mesh(BARREL, this.paint)
    this.tube.castShadow = true
    this.elevation.add(this.tube)

    this.head = new THREE.Mesh(HEAD, this.headMaterial)
    this.head.position.set(0, 1.2, 0.6)
    this.head.castShadow = true
    this.head.visible = false
    this.root.add(this.head)
  }

  /**
   * Places the mortar and aims it. `turret` is the mount's yaw (hull frame) and the elevation; `seats`
   * the occupant ids, 0 for empty. The gunner's head shows when someone works it.
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
    if (this.placed) {
      // Wheels turn with the ground they roll over.
      const ahead = this.forward.dot(this.root.position.clone().sub(this.last))
      this.roll -= ahead / MORTAR_WHEEL_RADIUS
    }
    this.last.copy(this.root.position)
    this.placed = true
    for (const wheel of this.spinners) wheel.rotation.x = this.roll

    this.mount.rotation.y = turret?.yaw ?? 0
    this.elevation.rotation.x = turret?.pitch ?? 0
    // The tube slides back along its own line and eases home.
    const since = (performance.now() - this.firedAt) / 1000
    this.tube.position.z = since < RECOIL_SECONDS ? RECOIL_STROKE * (1 - since / RECOIL_SECONDS) ** 2 : 0

    const gunner = seats[0] ?? 0
    this.head.visible = gunner !== 0
    if (gunner !== 0) this.headMaterial.color.setHex(colorFor(gunner))
  }

  /** A bomb was just fired: the tube kicks back. */
  recoil() {
    this.firedAt = performance.now()
  }

  /** The tube's root in world space as of the last `update`, and its unit aim direction. */
  barrel() {
    this.root.updateMatrixWorld(true)
    this.elevation.getWorldPosition(this.pivot)
    this.aim.set(0, 0, -1).transformDirection(this.elevation.matrixWorld)
    return { origin: this.pivot.clone(), direction: this.aim.clone() }
  }

  /** Frees what this mortar owns; the shared geometry stays. */
  dispose() {
    this.paint.dispose()
    this.steel.dispose()
    this.headMaterial.dispose()
    this.root.removeFromParent()
  }
}
