import * as THREE from 'three'
import { toonMaterial } from './toon'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { HELI_GUN_LENGTH, HELI_SEATS } from '@/sim/consts'
import type { Turret } from '@/net/codec'
import { colorFor } from './character'
import type { VehicleModel } from './car'

export const HELI_MODEL_URL = `${import.meta.env.BASE_URL}models/heli.glb`

/** Main rotor speed with the pilot aboard, rad/s, and how fast it spins up or down, rad/s². */
const ROTOR_SPEED = 28
const ROTOR_ACCEL = 14
const TAIL_ROTOR_RATIO = 1.6
const RECOIL_STROKE = 0.08
const RECOIL_SECONDS = 0.06
/** Paint colours handed out by vehicle id, so neighbouring helicopters are told apart. */
const PAINTS = [0x6b7a4f, 0x8a8456, 0x5d6b52, 0x7a7048] as const
const GLASS_OPACITY = 0.45
/** Each seat's occupant in the hull frame: the pilot raised behind and above the gunner in the nose. */
const SEATS: readonly (readonly [number, number, number])[] = [
  [0, 0.45, -0.45],
  [0, 0.1, -1.55],
]

let asset: THREE.Object3D | null = null

/**
 * Parses `heli.glb` (built by `tools/vehicles/build_heli.py`) for every `HeliModel` to share. Its
 * materials become flat toon ones to match the rest of the scene; `paint` is replaced per helicopter.
 */
export async function parseHeliModel(data: ArrayBuffer) {
  asset = await new Promise<THREE.Object3D>((resolve, reject) => {
    new GLTFLoader().parse(
      data,
      '',
      (gltf) => {
        gltf.scene.traverse((node) => {
          const mesh = node as THREE.Mesh
          if (!mesh.isMesh) return
          const source = mesh.material as THREE.MeshStandardMaterial
          const glass = source.name === 'glass'
          mesh.material = toonMaterial({
            color: source.color,
            name: source.name,
            transparent: glass,
            opacity: glass ? GLASS_OPACITY : 1,
            depthWrite: !glass,
          })
          source.dispose()
          mesh.castShadow = !glass
        })
        resolve(gltf.scene)
      },
      reject,
    )
  })
}

export async function loadHeliModel(url = HELI_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parseHeliModel(await response.arrayBuffer())
}

/**
 * A Mi-24 in silhouette: a double cockpit with a pilot's head behind and above the gunner's, stub
 * wings with pods, a five-blade main rotor that spins up while someone sits in the pilot's seat, and a
 * chin gun that the gunner (seat 1) swings and nods. The airframe box is the physics hull; the rotor
 * and the wings are bigger than it, as a cartoon's are.
 */
export class HeliModel implements VehicleModel {
  readonly root = new THREE.Group()
  private readonly rotor: THREE.Object3D
  private readonly tailRotor: THREE.Object3D
  private readonly turret: THREE.Object3D
  private readonly gun: THREE.Object3D
  private readonly gunRest: number
  private readonly heads: THREE.Mesh[] = []
  private readonly paint: THREE.MeshToonMaterial
  private rotorSpeed = 0
  private lastUpdate = performance.now()
  /** `performance.now()` of the last round, for the barrels' kick. */
  private firedAt = -Infinity
  /** Whoever sits at the gun, 0 for nobody. */
  gunner = 0
  private readonly pivot = new THREE.Vector3()
  private readonly aim = new THREE.Vector3()

  constructor(id: number) {
    if (!asset) throw new Error('loadHeliModel() must finish before helicopters are created')
    const model = asset.clone(true)
    this.root.add(model)
    this.paint = toonMaterial({ color: PAINTS[id % PAINTS.length]! })
    model.traverse((node) => {
      const mesh = node as THREE.Mesh
      if (mesh.isMesh && mesh.material instanceof THREE.Material && mesh.material.name === 'paint') mesh.material = this.paint
    })
    this.rotor = model.getObjectByName('rotor')!
    this.tailRotor = model.getObjectByName('tail_rotor')!
    this.turret = model.getObjectByName('turret')!
    this.gun = model.getObjectByName('gun')!
    this.gunRest = this.gun.position.z

    const headGeometry = new THREE.SphereGeometry(0.2, 12, 10)
    for (let seat = 0; seat < HELI_SEATS; seat++) {
      const head = new THREE.Mesh(headGeometry, toonMaterial({ color: 0xffffff }))
      head.position.set(...SEATS[seat]!)
      head.castShadow = true
      head.visible = false
      this.root.add(head)
      this.heads.push(head)
    }
  }

  /**
   * Places the helicopter and animates it. The rotor spins up while seat 0 is taken. `turret` is the nose
   * gun's angles in the hull frame. `steer` is unused: its lean and turn are in the orientation itself.
   */
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    _steer: number,
    seats: readonly number[],
    turret?: Turret | null,
  ) {
    const now = performance.now()
    const dt = Math.min(0.1, (now - this.lastUpdate) / 1000)
    this.lastUpdate = now
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    const target = (seats[0] ?? 0) !== 0 ? ROTOR_SPEED : 0
    const step = ROTOR_ACCEL * dt
    this.rotorSpeed += Math.max(-step, Math.min(step, target - this.rotorSpeed))
    this.rotor.rotation.y += this.rotorSpeed * dt
    this.tailRotor.rotation.x += this.rotorSpeed * TAIL_ROTOR_RATIO * dt

    this.turret.rotation.y = turret?.yaw ?? 0
    this.gun.rotation.x = turret?.pitch ?? 0
    const since = (now - this.firedAt) / 1000
    this.gun.position.z = this.gunRest + (since < RECOIL_SECONDS ? RECOIL_STROKE * (1 - since / RECOIL_SECONDS) : 0)
    this.gunner = seats[1] ?? 0
    this.heads.forEach((head, seat) => {
      const occupant = seats[seat] ?? 0
      head.visible = occupant !== 0
      if (occupant !== 0) (head.material as THREE.MeshToonMaterial).color.setHex(colorFor(occupant))
    })
  }

  /** A round was just fired: the barrels kick back. */
  recoil() {
    this.firedAt = performance.now()
  }

  /** The gun in world space as of the last `update`: its pivot and the unit aim direction. */
  barrel() {
    this.root.updateMatrixWorld(true)
    this.gun.getWorldPosition(this.pivot)
    this.aim.set(0, 0, -1).transformDirection(this.gun.matrixWorld)
    return { origin: this.pivot.clone(), direction: this.aim.clone() }
  }

  /** Where rounds leave the gun in world space. */
  muzzle() {
    const { origin, direction } = this.barrel()
    return origin.addScaledVector(direction, HELI_GUN_LENGTH)
  }

  /** Frees what this helicopter owns; the model's geometry and other materials are shared by every one. */
  dispose() {
    this.paint.dispose()
    for (const head of this.heads) {
      ;(head.material as THREE.Material).dispose()
    }
    this.heads[0]?.geometry.dispose()
    this.root.removeFromParent()
  }
}
