import { Group, Mesh, sphereGeometry, type Material, type Node } from '@ragnarok/engine'
import * as THREE from 'three'
import { Legs } from './legs'
import { parseToonModel, unlitMaterial } from './models'
import { toonMaterial } from './toon'
import { WALKER_FOOT_HALF, WALKER_GUN_BARREL_DROP, WALKER_GUN_BARREL_X, WALKER_HALF_EXTENTS, WALKER_LEG_LENGTH, WALKER_LEG_SPRING, CAR_GRAVITY } from '@/sim/consts'
import type { Turret } from '@/net/codec'
import { colorFor } from '@/profile'
import type { VehicleModel } from './car'

/** Paint colours handed out by vehicle id, so neighbouring walkers are told apart. */
const PAINTS = [0x7d8a8f, 0x8f8a7d, 0x6e7a73, 0x857c72] as const
export const WALKER_MODEL_URL = `${import.meta.env.BASE_URL}models/walker.glb`
/** Where the driver's head shows, in the upper body's frame: out of the roof hatch. */
const HEAD = [0.25, 1.4, 0.3] as const
/** How far the guns kick back when they fire, metres, and how fast they return, per second. */
const RECOIL = 0.35
const RECOIL_RETURN = 6
/** Hull sway to the planted side while it walks, radians, at the most. */
const SWAY = 0.06
/** Thigh and shin, metres, and the foot pad's thickness. The ankle hangs this far above the sole. */
const THIGH = 2.1
const SHIN = 2.2
/** The foot's thickness in the model (`PAD` in build_walker.py): the ankle hangs this far above the sole. */
const PAD = 0.3
/** How far the hips sit above the ground when it stands: the leg's reach less its spring's sag. */
const STAND = WALKER_LEG_LENGTH - CAR_GRAVITY / WALKER_LEG_SPRING

let asset: Node | null = null

/**
 * Parses `walker.glb` (built by `tools/vehicles/build_walker.py`) for every `WalkerModel` to share. Its
 * materials become flat toon ones to match the rest of the scene (`lamp` ones are unlit); `paint` is
 * replaced per walker.
 */
export async function parseWalkerModel(data: ArrayBuffer) {
  asset = parseToonModel(data, (source) => (source.name === 'lamp' ? unlitMaterial(source) : null), { merge: true })
}

export async function loadWalkerModel(url = WALKER_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parseWalkerModel(await response.arrayBuffer())
}

/**
 * An AT-ST in silhouette: an armoured cab with chin guns, on two long bird-like legs that step with the
 * ground they cover. The cab (the upper body) turns on the hips with the driver's aim, within a
 * turret's travel, and the guns lift in it. The legs are drawn only: the physics is one box
 * (the cab) held up by two spring rays, with a solid box on each leg, so they animate from the ground
 * speed. The mesh is `walker.glb`; its nodes are named in `build_walker.py`.
 */
export class WalkerModel implements VehicleModel {
  readonly root = new Group()
  private readonly paint: Material
  private readonly head = new Mesh(sphereGeometry(0.22, 12, 10), toonMaterial({ color: 0xffffff }))
  /** The upper body, which turns with the aim, and the guns in it, which lift. */
  private readonly torso: Node
  private readonly gun: Node
  private readonly gunRest: number
  private kick = 0
  private readonly legs: Legs
  /** Smoothed sideways sway of the hull, + is toward the right foot. */
  private sway = 0

  constructor(id: number) {
    if (!asset) throw new Error('loadWalkerModel() must finish before walkers are created')
    const model = asset.clone()
    this.root.add(model)
    this.paint = toonMaterial({ color: PAINTS[id % PAINTS.length]! })
    model.traverse((node) => {
      if (node instanceof Mesh && node.material.name === 'paint') node.material = this.paint
    })
    this.torso = model.getObjectByName('torso')!
    this.gun = model.getObjectByName('gun')!
    this.gunRest = this.gun.position.z
    const joint = (name: string) => ['L', 'R'].map((label) => model.getObjectByName(`${name}_${label}`)!)
    this.legs = new Legs(joint('hip'), joint('knee'), joint('ankle'), { thigh: THIGH, shin: SHIN, pad: PAD, stand: STAND, hipDrop: WALKER_HALF_EXTENTS[1] })
    this.head.position.set(...HEAD)
    this.head.castShadow = true
    this.head.visible = false
    this.torso.add(this.head)
  }

  /** Where the guns are and which way they point, in the world, for the muzzle flash and the tracer. */
  barrel(side = 0) {
    this.gun.updateWorldMatrix(true, false)
    // `side` -1 or 1 is one of the two barrels, which hang under the chin; 0 is the pivot between them.
    const matrix = new THREE.Matrix4().fromArray(this.gun.matrixWorld)
    const origin = new THREE.Vector3(side * WALKER_GUN_BARREL_X, side === 0 ? 0 : -WALKER_GUN_BARREL_DROP, 0).applyMatrix4(matrix)
    const direction = new THREE.Vector3(0, 0, -1).transformDirection(matrix)
    return { origin, direction }
  }

  /** The footfalls since the last call, for the thump that goes with each. */
  takeSteps() {
    return this.legs.takeSteps()
  }

  /** The guns kick back as they fire. */
  recoil() {
    this.kick = RECOIL
  }

  /** Places the walker and swings its legs by how far it has moved since the last call. `steer` is unused. */
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    _steer: number,
    seats: readonly number[],
    turret?: Turret | null,
  ) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)

    // The upper body and guns follow the aim the server reports; the legs keep their own heading.
    this.torso.rotation.y = turret?.yaw ?? 0
    this.gun.rotation.x = turret?.pitch ?? 0

    const { dt, speed, spin, lean } = this.legs.update(position, rotation)

    // The hull sways over whichever foot is down, more the faster it goes.
    const target = lean * SWAY * Math.min(1, Math.hypot(speed, Math.abs(spin) * WALKER_FOOT_HALF) / 3)
    this.sway += (target - this.sway) * Math.min(1, dt * 10)
    // Leaning to the right moves the top of the hull to +x, which is a negative turn about the nose axis.
    this.root.quaternion.multiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 0, 1), -this.sway))

    this.kick = Math.max(0, this.kick - this.kick * Math.min(1, dt * RECOIL_RETURN) - dt * 0.2)
    this.gun.position.z = this.gunRest + this.kick

    const driver = seats[0] ?? 0
    this.head.visible = driver !== 0
    if (driver !== 0) {
      this.head.material.color.setHex(colorFor(driver))
      this.head.material.needsUpdate()
    }
  }

  dispose() {
    this.paint.dispose()
    this.head.material.dispose()
    this.head.geometry.dispose()
    this.root.removeFromParent()
  }
}
