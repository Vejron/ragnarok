import * as THREE from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { PLANE_BOMBS, PLANE_REAR_LENGTH, PLANE_SEATS } from '@/sim/consts'
import { ARMS_CUT, type Arms, type Turret } from '@/net/codec'
import { colorFor } from './character'
import type { VehicleModel } from './car'

export const PLANE_MODEL_URL = `${import.meta.env.BASE_URL}models/plane.glb`

/**
 * Propeller speed with the engine running, and windmilling with the blip switch in, rad/s, and how fast
 * it gets there, rad/s². A real rotary turns twenty times a second; drawn at that it strobes.
 */
const PROP_SPEED = 45
const PROP_CUT_SPEED = 14
const PROP_ACCEL = 30
const RECOIL_STROKE = 0.05
const RECOIL_SECONDS = 0.05
/** How far the control surfaces swing at full deflection, radians, and the turn rates that give it, rad/s. */
const SURFACE_THROW = 0.45
const PITCH_FULL = 1.0
const ROLL_FULL = 2.0
const YAW_FULL = 0.8
/** How quickly the surfaces follow the measured turn rates, per second. */
const SURFACE_EASE = 10
/** Paint handed out by vehicle id: Richthofen's red, then a Jasta's yellow, so the two planes are told apart. */
const PAINTS = [0x8f1a14, 0xc99a1e, 0x2f4d7a, 0x3c5a2e] as const
const GLASS_OPACITY = 0.45
/** Each seat's occupant in the hull frame: the pilot behind the wings, the gunner standing in the ring. */
const SEATS: readonly (readonly [number, number, number])[] = [
  [0, 0.98, -0.45],
  [0, 1.15, 0.7],
]

let asset: THREE.Object3D | null = null

/**
 * Parses `plane.glb` (built by `tools/vehicles/build_plane.py`) for every `PlaneModel` to share. Its
 * materials become flat toon ones to match the rest of the scene; `paint` is replaced per plane.
 */
export async function parsePlaneModel(data: ArrayBuffer) {
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
          mesh.material = new THREE.MeshToonMaterial({
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

export async function loadPlaneModel(url = PLANE_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parsePlaneModel(await response.arrayBuffer())
}

/**
 * A Fokker Dr.I stretched into a two-seater: the pilot behind the triplane's wings with two guns on the
 * cowling, a gunner on a ring behind, and four small bombs under the belly. The propeller spins while
 * seat 0 is taken and slows to a windmill on the blip switch; the ring and its gun follow the snapshot's
 * `Turret`; the control surfaces swing with how the plane is turning, so everyone sees them work.
 */
export class PlaneModel implements VehicleModel {
  readonly root = new THREE.Group()
  private readonly prop: THREE.Object3D
  private readonly guns: THREE.Object3D[]
  private readonly gunRests: number[]
  private readonly ring: THREE.Object3D
  private readonly ringGun: THREE.Object3D
  private readonly ringGunRest: number
  private readonly elevator: THREE.Object3D
  private readonly rudder: THREE.Object3D
  private readonly ailerons: [THREE.Object3D, THREE.Object3D]
  private readonly bombs: THREE.Object3D[]
  private readonly heads: THREE.Mesh[] = []
  private readonly paint: THREE.MeshToonMaterial
  private propSpeed = 0
  private lastUpdate = performance.now()
  /** `performance.now()` of each forward gun's last round, and the ring gun's. */
  private readonly firedAt = [-Infinity, -Infinity]
  private ringFiredAt = -Infinity
  /** The orientation at the last update, to measure how fast it turns. */
  private readonly lastRotation = new THREE.Quaternion()
  private hasRotation = false
  /** Smoothed turn rates in the plane's own frame, rad/s: + pitches up, + yaws left, + rolls right wing up. */
  readonly rates = new THREE.Vector3()
  /** Whoever sits at the ring, 0 for nobody. */
  gunner = 0
  private readonly pivot = new THREE.Vector3()
  private readonly aim = new THREE.Vector3()

  constructor(id: number) {
    if (!asset) throw new Error('loadPlaneModel() must finish before planes are created')
    const model = asset.clone(true)
    this.root.add(model)
    this.paint = new THREE.MeshToonMaterial({ color: PAINTS[id % PAINTS.length]! })
    model.traverse((node) => {
      const mesh = node as THREE.Mesh
      if (mesh.isMesh && mesh.material instanceof THREE.Material && mesh.material.name === 'paint') mesh.material = this.paint
    })
    const part = (name: string) => model.getObjectByName(name)!
    this.prop = part('prop')
    this.guns = [part('gun_L'), part('gun_R')]
    this.gunRests = this.guns.map((gun) => gun.position.z)
    this.ring = part('turret')
    this.ringGun = part('gun')
    this.ringGunRest = this.ringGun.position.z
    this.elevator = part('elevator')
    this.rudder = part('rudder')
    this.ailerons = [part('aileron_L'), part('aileron_R')]
    this.bombs = Array.from({ length: PLANE_BOMBS }, (_, k) => part(`bomb_${k + 1}`))

    const headGeometry = new THREE.SphereGeometry(0.2, 12, 10)
    for (let seat = 0; seat < PLANE_SEATS; seat++) {
      const head = new THREE.Mesh(headGeometry, new THREE.MeshToonMaterial({ color: 0xffffff }))
      head.position.set(...SEATS[seat]!)
      head.castShadow = true
      head.visible = false
      this.root.add(head)
      this.heads.push(head)
    }
  }

  /**
   * Places the plane and animates it. `turret` is the ring gun's angles in the hull frame; `arms` says
   * which bombs are still on the rack and whether the blip switch is in. `steer` is the front wheels'.
   */
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    _steer: number,
    seats: readonly number[],
    turret?: Turret | null,
    arms?: Arms | null,
  ) {
    const now = performance.now()
    const dt = Math.min(0.1, (now - this.lastUpdate) / 1000)
    this.lastUpdate = now
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    this.measureRates(dt)

    const running = (seats[0] ?? 0) !== 0
    const cut = ((arms?.flags ?? 0) & ARMS_CUT) !== 0
    const target = !running ? 0 : cut ? PROP_CUT_SPEED : PROP_SPEED
    const step = PROP_ACCEL * dt
    this.propSpeed += Math.max(-step, Math.min(step, target - this.propSpeed))
    this.prop.rotation.z += this.propSpeed * dt

    // The surfaces hang behind their hinges (+Z), so a + turn about X drops a trailing edge and one about Y
    // swings it right. Elevator up to pitch up, rudder to the side of the turn, and to roll the right wing
    // up its aileron goes down and the left one up.
    const ease = Math.min(1, SURFACE_EASE * dt)
    const throw_ = (rate: number, full: number) => Math.max(-1, Math.min(1, rate / full)) * SURFACE_THROW
    this.elevator.rotation.x += (-throw_(this.rates.x, PITCH_FULL) - this.elevator.rotation.x) * ease
    this.rudder.rotation.y += (-throw_(this.rates.y, YAW_FULL) - this.rudder.rotation.y) * ease
    const roll = throw_(this.rates.z, ROLL_FULL)
    this.ailerons[0].rotation.x += (-roll - this.ailerons[0].rotation.x) * ease
    this.ailerons[1].rotation.x += (roll - this.ailerons[1].rotation.x) * ease

    const bombs = arms?.bombs ?? PLANE_BOMBS
    this.bombs.forEach((bomb, k) => (bomb.visible = k < bombs))

    this.ring.rotation.y = turret?.yaw ?? 0
    this.ringGun.rotation.x = turret?.pitch ?? 0
    const kick = (since: number) => (since < RECOIL_SECONDS ? RECOIL_STROKE * (1 - since / RECOIL_SECONDS) : 0)
    this.ringGun.position.z = this.ringGunRest + kick((now - this.ringFiredAt) / 1000)
    this.guns.forEach((gun, side) => (gun.position.z = this.gunRests[side]! + kick((now - this.firedAt[side]!) / 1000)))
    this.gunner = seats[1] ?? 0
    this.heads.forEach((head, seat) => {
      const occupant = seats[seat] ?? 0
      head.visible = occupant !== 0
      if (occupant !== 0) (head.material as THREE.MeshToonMaterial).color.setHex(colorFor(occupant))
    })
  }

  /** The turn rates from how the orientation changed since the last update, smoothed. */
  private measureRates(dt: number) {
    const now = this.root.quaternion
    if (this.hasRotation && dt > 1e-4) {
      // The change in the plane's own frame: last⁻¹ · now, as an axis and angle.
      const delta = this.lastRotation.clone().invert().multiply(now)
      if (delta.w < 0) delta.set(-delta.x, -delta.y, -delta.z, -delta.w)
      const angle = 2 * Math.acos(Math.min(1, delta.w))
      const sin = Math.sqrt(Math.max(0, 1 - delta.w * delta.w))
      const measured = sin > 1e-6 ? new THREE.Vector3(delta.x, delta.y, delta.z).multiplyScalar(angle / sin / dt) : new THREE.Vector3()
      this.rates.lerp(measured, Math.min(1, dt * 8))
    }
    this.lastRotation.copy(now)
    this.hasRotation = true
  }

  /** A forward gun fired a round: the left one (0) fires the even rounds, the right one (1) the odd. */
  recoil(side: number) {
    this.firedAt[side % 2] = performance.now()
  }

  /** The ring gun fired a round. */
  recoilRing() {
    this.ringFiredAt = performance.now()
  }

  /** A forward gun in world space as of the last `update`: its muzzle and the unit direction it fires along. */
  forwardGun(side: number) {
    this.root.updateMatrixWorld(true)
    const gun = this.guns[side % 2]!
    gun.getWorldPosition(this.pivot)
    this.aim.set(0, 0, -1).transformDirection(this.root.matrixWorld)
    return { origin: this.pivot.clone(), direction: this.aim.clone() }
  }

  /** The ring gun in world space as of the last `update`: its pivot and the unit aim direction. */
  barrel() {
    this.root.updateMatrixWorld(true)
    this.ringGun.getWorldPosition(this.pivot)
    this.aim.set(0, 0, -1).transformDirection(this.ringGun.matrixWorld)
    return { origin: this.pivot.clone(), direction: this.aim.clone() }
  }

  /** Where the ring gun's rounds leave it in world space. */
  muzzle() {
    const { origin, direction } = this.barrel()
    return origin.addScaledVector(direction, PLANE_REAR_LENGTH)
  }

  /** Frees what this plane owns; the model's geometry and other materials are shared by every one. */
  dispose() {
    this.paint.dispose()
    for (const head of this.heads) {
      ;(head.material as THREE.Material).dispose()
    }
    this.heads[0]?.geometry.dispose()
    this.root.removeFromParent()
  }
}
