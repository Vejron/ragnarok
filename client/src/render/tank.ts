import { Group, Mesh, sphereGeometry, type Geometry, type Material, type Node } from '@ragnarok/engine'
import * as THREE from 'three'
import { parseToonModel } from './models'
import { toonMaterial } from './toon'
import { boxUvs, camoTexture } from './camo'
import { CAR_SEATS, TANK_TRACK_HALF } from '@/sim/consts'
import { wrapAngle } from '@/net/codec'
import type { Turret } from '@/entities/units'
import { colorFor } from '@/profile'
import type { VehicleModel } from './car'

export const TANK_MODEL_URL = `${import.meta.env.BASE_URL}models/tank.glb`

/** Radius of the road wheels in the model (`WHEEL_RADIUS` in build_tank.py): the tracks' speed over them is what rolls them. */
const WHEEL_RADIUS = 0.4
const RECOIL_STROKE = 0.5
const RECOIL_SECONDS = 0.35
/**
 * WW2 camo schemes handed out by vehicle id, base coat first, so neighbouring tanks are told apart:
 * dark yellow with olive green and red-brown (twice, the second faded), olive green with the same two,
 * and olive drab with black and earth brown.
 */
const PAINTS = [
  [0xc2a868, 0x4e5a38, 0x6e3b2a],
  [0xb3a06e, 0x5d6443, 0x7a4834],
  [0x55603f, 0xc2a868, 0x6e3b2a],
  [0x4f5528, 0x2b2b25, 0x6b5a3a],
] as const

/** What differs between the tanks this model draws: where each seat's head shows and how the tracks turn the wheels. */
export interface TankLayout {
  /** The driver's head in the hull frame. */
  driverHead: readonly [number, number, number]
  /** The gunner's head in the turret frame; none where the driver fires the gun. */
  gunnerHead: readonly [number, number, number] | null
  wheelRadius: number
  trackHalf: number
}

const TANK_LAYOUT: TankLayout = { driverHead: [0, 0.55, -1.55], gunnerHead: [-0.45, 0.5, 0.3], wheelRadius: WHEEL_RADIUS, trackHalf: TANK_TRACK_HALF }

let asset: Node | null = null

/**
 * Parses `tank.glb` (built by `tools/vehicles/build_tank.py`) for every `TankModel` to share. Its
 * materials become flat toon ones to match the rest of the scene; `paint` is replaced per tank by camo.
 */
export async function parseTankModel(data: ArrayBuffer) {
  asset = await parseTankAsset(data)
}

/** Parses a tank-family `.glb` into toon materials, for `TankModel` and its kin to share. */
export async function parseTankAsset(data: ArrayBuffer): Promise<Node> {
  const model = parseToonModel(data, undefined, { merge: true })
  // The model has no uvs: the paint gets them boxed round it, for the camo to lie on.
  const boxed = new Map<Geometry, Geometry>()
  model.traverse((node) => {
    if (!(node instanceof Mesh) || node.material.name !== 'paint') return
    let geometry = boxed.get(node.geometry)
    if (!geometry) boxed.set(node.geometry, (geometry = boxUvs(node.geometry)))
    node.geometry = geometry
  })
  return model
}

export async function loadTankModel(url = TANK_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parseTankModel(await response.arrayBuffer())
}

/**
 * A T-55 in silhouette: a hull on two track runs with seven wheels each, and a turret that swings
 * on the hull with a barrel that nods. The driver's head shows in a hatch, the gunner's in the
 * turret's cupola. Wheels roll with the movement of each track (so turning on the spot rolls them
 * in opposite directions).
 */
export class TankModel implements VehicleModel {
  readonly root = new Group()
  /** The turret node; a turretless tank (the Strv S) has none. */
  private readonly turret: Node | null
  private readonly gun: Node
  private readonly gunRest: number
  private readonly spinners: { node: Node; side: number }[] = []
  private readonly heads: Mesh[] = []
  private readonly paint: Material
  private roll = { left: 0, right: 0 }
  private placed = false
  /** `performance.now()` of the last shot, for the barrel's recoil stroke. */
  private firedAt = -Infinity
  private lastYaw = 0
  private readonly last = new THREE.Vector3()
  private readonly forward = new THREE.Vector3()
  private readonly pivot = new THREE.Vector3()
  private readonly aim = new THREE.Vector3()
  private readonly matrix = new THREE.Matrix4()

  constructor(id: number, source: Node | null = asset, private readonly layout: TankLayout = TANK_LAYOUT) {
    if (!source) throw new Error('loadTankModel() must finish before tanks are created')
    const model = source.clone()
    this.root.add(model)
    this.paint = toonMaterial({ map: camoTexture(PAINTS[id % PAINTS.length]!) })
    model.traverse((node) => {
      if (node instanceof Mesh && node.material.name === 'paint') node.material = this.paint
      const wheel = /^wheel_([LR])_/.exec(node.name)
      if (wheel) this.spinners.push({ node, side: wheel[1] === 'L' ? -1 : 1 })
    })
    this.turret = model.getObjectByName('turret') ?? null
    this.gun = model.getObjectByName('gun')!
    this.gunRest = this.gun.position.z

    const headGeometry = sphereGeometry(0.2, 12, 10)
    for (let seat = 0; seat < CAR_SEATS; seat++) {
      const head = new Mesh(headGeometry, toonMaterial({ color: 0xffffff }))
      head.userData.head = true
      head.visible = false
      head.castShadow = true
      this.heads.push(head)
      if (seat === 0) {
        head.position.set(...layout.driverHead)
        this.root.add(head)
      } else if (seat === 1 && this.turret && layout.gunnerHead) {
        head.position.set(...layout.gunnerHead)
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
      const ahead = this.forward.x * (position[0] - this.last.x) + this.forward.y * (position[1] - this.last.y) + this.forward.z * (position[2] - this.last.z)
      const turned = wrapAngle(yaw - this.lastYaw) * this.layout.trackHalf
      // Turning left (+yaw) holds the left track back and drives the right one on.
      this.roll.left -= (ahead - turned) / this.layout.wheelRadius
      this.roll.right -= (ahead + turned) / this.layout.wheelRadius
    }
    this.last.set(position[0], position[1], position[2])
    this.lastYaw = yaw
    this.placed = true
    for (const { node, side } of this.spinners) node.rotation.x = side < 0 ? this.roll.left : this.roll.right

    // A turretless tank's gun is fixed in the hull, whatever yaw it is sent.
    if (this.turret) this.turret.rotation.y = turret?.yaw ?? 0
    this.gun.rotation.x = turret?.pitch ?? 0
    // The barrel slides back and eases home over a third of a second.
    const since = (performance.now() - this.firedAt) / 1000
    this.gun.position.z = this.gunRest + (since < RECOIL_SECONDS ? RECOIL_STROKE * (1 - since / RECOIL_SECONDS) ** 2 : 0)
    this.heads.forEach((head, seat) => {
      const occupant = seats[seat] ?? 0
      head.visible = occupant !== 0
      if (occupant !== 0) {
        head.material.color.setHex(colorFor(occupant))
        head.material.needsUpdate()
      }
    })
  }

  /** Whether the turret is on, for a tank that has one. */
  get turretOn() {
    return this.turret?.visible ?? false
  }

  /** Shows the turret, or hides it once it has been shot off. */
  setTurret(on: boolean) {
    if (this.turret) this.turret.visible = on
  }

  /**
   * A copy of the turret, with the gun, as it stands now in the world, for flying off as debris; null
   * for a tank with none or one that is already off. Add the copy to the scene and throw it.
   */
  turretCopy(): Node | null {
    if (!this.turret?.visible) return null
    this.root.updateMatrixWorld(true)
    const copy = this.turret.clone()
    // The gunner's head stays with the crew who died with it.
    copy.traverse((node) => {
      if (node.userData.head) node.visible = false
    })
    copy.setFromMatrix(this.turret.matrixWorld)
    return copy
  }

  /** A shell was just fired: the barrel kicks back. */
  recoil() {
    this.firedAt = performance.now()
  }

  /** The barrel in world space as of the last `update`: the turret ring and the unit aim direction. */
  barrel() {
    this.root.updateMatrixWorld(true)
    this.gun.getWorldPosition(this.pivot)
    this.aim.set(0, 0, -1).transformDirection(this.matrix.fromArray(this.gun.matrixWorld))
    return { origin: this.pivot.clone(), direction: this.aim.clone() }
  }

  /** Frees what this tank owns; the model's geometry and other materials are shared by every tank. */
  dispose() {
    this.paint.dispose()
    for (const head of this.heads) head.material.dispose()
    this.heads[0]?.geometry.dispose()
    this.root.removeFromParent()
  }
}
