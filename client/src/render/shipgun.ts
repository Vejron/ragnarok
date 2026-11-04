import { boxGeometry, cylinderGeometry, sphereGeometry } from '@ragnarok/engine'
import { TowedGun } from './towedgun'
import type { VehicleModel } from './car'
import type { Turret } from '@/entities/units'
import { SHIP_GUN_BARREL_LENGTH, SHIP_GUN_HALF_EXTENTS, SHIP_GUN_PIVOT, SHIP_GUN_SEAT_HEAD } from '@/sim/consts'

/** The gun and its carriage run back on the deck when it fires and are hauled out again, seconds and metres. */
const RECOIL_SECONDS = 0.9
const RECOIL_STROKE = 0.45
const OAK = 0x6b4a2b
const IRON = 0x26282a
const ROPE = 0xb89d6c

/** The carriage's floor, from the trunnions: everything below the barrel is placed from here. */
const FLOOR = -SHIP_GUN_PIVOT[1] - SHIP_GUN_HALF_EXTENTS[1]
const TRUCK_RADIUS = 0.15
const CHEEK_LENGTH = 0.95

// Shared by every ship's gun: parts only ever move, so none of these is disposed.
const CHEEK = boxGeometry(0.08, 0.4, CHEEK_LENGTH)
const BED = boxGeometry(0.34, 0.08, CHEEK_LENGTH)
const AXLETREE = boxGeometry(0.56, 0.1, 0.12)
const TRUCK = cylinderGeometry(TRUCK_RADIUS, TRUCK_RADIUS, 0.08, 12)
TRUCK.rotateZ(Math.PI / 2)
/** A long gun: thick at the breech, tapering to the muzzle, with a swell at the mouth and a knob behind. */
const TUBE = cylinderGeometry(0.08, 0.13, SHIP_GUN_BARREL_LENGTH, 14)
TUBE.rotateX(-Math.PI / 2)
TUBE.translate(0, 0, -SHIP_GUN_BARREL_LENGTH / 2 + 0.25)
const SWELL = cylinderGeometry(0.11, 0.11, 0.1, 14)
SWELL.rotateX(-Math.PI / 2)
const REINFORCE = cylinderGeometry(0.14, 0.14, 0.08, 14)
REINFORCE.rotateX(-Math.PI / 2)
const CASCABEL = sphereGeometry(0.07, 10, 8)
const TRUNNION = cylinderGeometry(0.05, 0.05, 0.36, 8)
TRUNNION.rotateZ(Math.PI / 2)
const QUOIN = boxGeometry(0.18, 0.08, 0.3)
const BREECHING = cylinderGeometry(0.02, 0.02, 1.0, 6)
BREECHING.rotateZ(Math.PI / 2)

/**
 * A ship's long gun on its truck carriage: two oak cheeks on four small wheels, an iron barrel resting
 * in them on its trunnions, laid by its gunner within its port's travel. The whole carriage traverses
 * on the deck and the barrel elevates on its quoin, so its line is the server's aim; it fixes nothing
 * to the ship, which it is drawn on as its own vehicle. Fired, gun and carriage run back on their trucks
 * against the breeching and are run out again.
 */
export class ShipGunModel extends TowedGun implements VehicleModel {
  constructor(_id: number) {
    super(OAK, IRON, ROPE)
    this.mount.name = 'mount'
    this.mount.position.set(...SHIP_GUN_PIVOT)
    this.root.add(this.mount)
    // Everything runs back together: the carriage under the gun and the gun in it.
    this.mount.add(this.recoiling)
    this.buildCarriage()
    this.elevation.name = 'gun'
    this.recoiling.add(this.elevation)
    this.part(TUBE, this.steel, this.elevation, 0, 0, 0)
    this.part(SWELL, this.steel, this.elevation, 0, 0, -SHIP_GUN_BARREL_LENGTH + 0.3)
    for (const z of [0.15, -0.25]) this.part(REINFORCE, this.steel, this.elevation, 0, 0, z)
    this.part(CASCABEL, this.steel, this.elevation, 0, 0, 0.3)
    this.part(TRUNNION, this.steel, this.elevation, 0, 0, 0)

    this.seat.position.set(...SHIP_GUN_SEAT_HEAD)
    this.mount.add(this.seat)
  }

  /** The cheeks, the bed between them, the axletrees and their trucks, the quoin under the breech and the breeching rope. */
  private buildCarriage() {
    const z = 0.18
    for (const side of [-1, 1]) {
      this.part(CHEEK, this.paint, this.recoiling, side * 0.19, FLOOR + TRUCK_RADIUS + 0.2, z)
      for (const end of [-0.33, 0.33]) {
        const truck = this.part(TRUCK, this.paint, this.recoiling, side * 0.25, FLOOR + TRUCK_RADIUS, z + end)
        this.spinners.push(truck)
      }
    }
    this.part(BED, this.paint, this.recoiling, 0, FLOOR + TRUCK_RADIUS + 0.04, z)
    for (const end of [-0.33, 0.33]) this.part(AXLETREE, this.paint, this.recoiling, 0, FLOOR + TRUCK_RADIUS, z + end)
    this.part(QUOIN, this.paint, this.recoiling, 0, -0.14, 0.4)
    this.part(BREECHING, this.rubber, this.recoiling, 0, -0.05, 0.38, false)
  }

  /** Places the gun on the deck and lays it: `turret` is its traverse (hull frame) and its elevation. */
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    _steer: number,
    _seats: readonly number[],
    turret?: Turret | null,
  ) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    this.mount.rotation.y = turret?.yaw ?? 0
    this.elevation.rotation.x = turret?.pitch ?? 0
    // It runs back fast, and is hauled out again slowly.
    const since = (performance.now() - this.firedAt) / 1000
    const back = since < RECOIL_SECONDS ? RECOIL_STROKE * (since < 0.12 ? since / 0.12 : 1 - (since - 0.12) / (RECOIL_SECONDS - 0.12)) : 0
    this.recoiling.position.z = back
    for (const truck of this.spinners) truck.rotation.x = -back / TRUCK_RADIUS
  }
}
