import * as THREE from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { CAR_SEATS, TANK_TRACK_HALF } from '@/sim/consts'
import { wrapAngle, type Turret } from '@/net/codec'
import { colorFor } from './character'
import type { VehicleModel } from './car'

export const TANK_MODEL_URL = `${import.meta.env.BASE_URL}models/tank.glb`

/** Radius of the road wheels in the model (`WHEEL_RADIUS` in build_tank.py): the tracks' speed over them is what rolls them. */
const WHEEL_RADIUS = 0.4
const RECOIL_STROKE = 0.5
const RECOIL_SECONDS = 0.35
/** Paint colours handed out by vehicle id, so neighbouring tanks are told apart. */
const PAINTS = [0x6b8e23, 0x8a7f4a, 0x5f7d5a, 0x7a6f3d] as const
/** Each seat's occupant, in the hull frame (driver) or the turret frame (gunner). */
const DRIVER_HEAD: readonly [number, number, number] = [0, 0.55, -1.55]
const GUNNER_HEAD: readonly [number, number, number] = [-0.45, 0.5, 0.3]

let asset: THREE.Object3D | null = null

/**
 * Parses `tank.glb` (built by `tools/vehicles/build_tank.py`) for every `TankModel` to share. Its
 * materials become flat toon ones to match the rest of the scene; `paint` is replaced per tank.
 */
export async function parseTankModel(data: ArrayBuffer) {
  asset = await new Promise<THREE.Object3D>((resolve, reject) => {
    new GLTFLoader().parse(
      data,
      '',
      (gltf) => {
        gltf.scene.traverse((node) => {
          const mesh = node as THREE.Mesh
          if (!mesh.isMesh) return
          const source = mesh.material as THREE.MeshStandardMaterial
          mesh.material = new THREE.MeshToonMaterial({ color: source.color, name: source.name })
          source.dispose()
          mesh.castShadow = true
        })
        resolve(gltf.scene)
      },
      reject,
    )
  })
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
  readonly root = new THREE.Group()
  private readonly turret: THREE.Object3D
  private readonly gun: THREE.Object3D
  private readonly gunRest: number
  private readonly spinners: { node: THREE.Object3D; side: number }[] = []
  private readonly heads: THREE.Mesh[] = []
  private readonly paint: THREE.MeshToonMaterial
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
    if (!asset) throw new Error('loadTankModel() must finish before tanks are created')
    const model = asset.clone(true)
    this.root.add(model)
    this.paint = new THREE.MeshToonMaterial({ color: PAINTS[id % PAINTS.length]! })
    model.traverse((node) => {
      const mesh = node as THREE.Mesh
      if (mesh.isMesh && mesh.material instanceof THREE.Material && mesh.material.name === 'paint') mesh.material = this.paint
      const wheel = /^wheel_([LR])_/.exec(node.name)
      if (wheel) this.spinners.push({ node, side: wheel[1] === 'L' ? -1 : 1 })
    })
    this.turret = model.getObjectByName('turret')!
    this.gun = model.getObjectByName('gun')!
    this.gunRest = this.gun.position.z

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
      this.roll.left -= (ahead - turned) / WHEEL_RADIUS
      this.roll.right -= (ahead + turned) / WHEEL_RADIUS
    }
    this.last.copy(this.root.position)
    this.lastYaw = yaw
    this.placed = true
    for (const { node, side } of this.spinners) node.rotation.x = side < 0 ? this.roll.left : this.roll.right

    this.turret.rotation.y = turret?.yaw ?? 0
    this.gun.rotation.x = turret?.pitch ?? 0
    // The barrel slides back and eases home over a third of a second.
    const since = (performance.now() - this.firedAt) / 1000
    this.gun.position.z = this.gunRest + (since < RECOIL_SECONDS ? RECOIL_STROKE * (1 - since / RECOIL_SECONDS) ** 2 : 0)
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

  /** Frees what this tank owns; the model's geometry and other materials are shared by every tank. */
  dispose() {
    this.paint.dispose()
    for (const head of this.heads) {
      ;(head.material as THREE.Material).dispose()
    }
    this.heads[0]?.geometry.dispose()
    this.root.removeFromParent()
  }
}
