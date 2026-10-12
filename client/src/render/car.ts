import * as THREE from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { JEEP_MAX_STEER, JEEP_WHEEL_RADIUS, SPORTS_MAX_STEER, SPORTS_SEATS, SPORTS_WHEEL_RADIUS, CAR_SEATS } from '@/sim/consts'
import { KIND_SPORTS, type Turret } from '@/net/codec'
import { colorFor } from './character'
import { CarWear } from './carwear'
import type { Debris } from './debris'
import { halfExtentsOf } from '@/sim/kinds'

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

type CarKind = 'jeep' | 'sports'

/** Per-kind facts the model needs: where its mesh is, how big its wheels are and how far they steer. */
const KINDS: Record<CarKind, { url: string; wheelRadius: number; maxSteer: number; seats: number }> = {
  jeep: { url: `${import.meta.env.BASE_URL}models/jeep.glb`, wheelRadius: JEEP_WHEEL_RADIUS, maxSteer: JEEP_MAX_STEER, seats: CAR_SEATS },
  sports: { url: `${import.meta.env.BASE_URL}models/sports.glb`, wheelRadius: SPORTS_WHEEL_RADIUS, maxSteer: SPORTS_MAX_STEER, seats: SPORTS_SEATS },
}

const BODY_COLORS = [0xff9f1c, 0x2ec4b6, 0xe71d36, 0x5aa9e6, 0xb5e48c, 0xf15bb5, 0xffd23f, 0x9d4edd]

const assets = new Map<CarKind, THREE.Object3D>()

/**
 * Parses `jeep.glb` or `sports.glb` (built by `tools/vehicles/build_car.py`) for every `CarModel` of
 * that kind to share. Materials become flat toon ones to match the rest of the scene (`lamp` ones are
 * unlit, like a headlight); `paint` is replaced per car.
 */
export async function parseCarModel(kind: CarKind, data: ArrayBuffer) {
  const scene = await new Promise<THREE.Object3D>((resolve, reject) => {
    new GLTFLoader().parse(
      data,
      '',
      (gltf) => {
        gltf.scene.traverse((node) => {
          const mesh = node as THREE.Mesh
          if (!mesh.isMesh) return
          const source = mesh.material as THREE.MeshStandardMaterial
          mesh.material =
            source.name === 'lamp'
              ? new THREE.MeshBasicMaterial({ color: source.color, name: source.name })
              : new THREE.MeshToonMaterial({ color: source.color, name: source.name })
          source.dispose()
          mesh.castShadow = true
        })
        resolve(gltf.scene)
      },
      reject,
    )
  })
  assets.set(kind, scene)
}

export async function loadCarModels() {
  await Promise.all(
    (Object.keys(KINDS) as CarKind[]).map(async (kind) => {
      const { url } = KINDS[kind]
      const response = await fetch(url)
      if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
      await parseCarModel(kind, await response.arrayBuffer())
    }),
  )
}

/**
 * An open jeep or a low sports car, from the glTF the Blender script builds. Wheels roll with the
 * speed the interpolator reports and the front pair steers. Passengers show as coloured heads, one
 * per seat, at the `seat_N` markers of the model.
 */
export class CarModel implements VehicleModel {
  readonly root = new THREE.Group()
  private readonly frontPivots: THREE.Object3D[] = []
  private readonly spinners: THREE.Object3D[] = []
  private readonly heads: THREE.Mesh[] = []
  private readonly paint: THREE.MeshToonMaterial
  private readonly wheelRadius: number
  private readonly maxSteer: number
  private roll = 0
  private placed = false
  private readonly last = new THREE.Vector3()
  private readonly forward = new THREE.Vector3()
  private readonly damage: CarWear

  /** `kind` is a vehicle kind off the wire; anything but the sports car is a jeep. */
  constructor(kind: number, id: number) {
    const name: CarKind = kind === KIND_SPORTS ? 'sports' : 'jeep'
    const asset = assets.get(name)
    if (!asset) throw new Error('loadCarModels() must finish before cars are created')
    const { wheelRadius, maxSteer, seats } = KINDS[name]
    this.wheelRadius = wheelRadius
    this.maxSteer = maxSteer
    const model = asset.clone(true)
    this.root.add(model)
    const paint = (this.paint = new THREE.MeshToonMaterial({ color: BODY_COLORS[id % BODY_COLORS.length]! }))
    model.traverse((node) => {
      const mesh = node as THREE.Mesh
      if (mesh.isMesh && mesh.material instanceof THREE.Material && mesh.material.name === 'paint') mesh.material = paint
      if (/^wheel_[FR][LR]$/.test(node.name)) this.spinners.push(node)
      if (/^pivot_F[LR]$/.test(node.name)) this.frontPivots.push(node)
    })

    this.damage = new CarWear(this.root, model, halfExtentsOf(kind), wheelRadius, paint)

    const headGeometry = new THREE.SphereGeometry(0.2, 12, 10)
    for (let seat = 0; seat < seats; seat++) {
      const head = new THREE.Mesh(headGeometry, new THREE.MeshToonMaterial({ color: 0xffffff }))
      head.position.copy(model.getObjectByName(`seat_${seat}`)!.position)
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
    if (this.placed) this.roll -= this.forward.dot(this.root.position.clone().sub(this.last)) / this.wheelRadius
    this.last.copy(this.root.position)
    this.placed = true
    for (const spinner of this.spinners) spinner.rotation.x = this.roll
    // Turning right (positive) is a clockwise yaw, which is a negative rotation about +Y.
    for (const pivot of this.frontPivots) pivot.rotation.y = -steer * this.maxSteer
    this.heads.forEach((head, seat) => {
      const occupant = seats[seat] ?? 0
      head.visible = occupant !== 0
      if (occupant !== 0) (head.material as THREE.MeshToonMaterial).color.setHex(colorFor(occupant))
    })
  }

  /** The colour it was painted, for flying chips of it. */
  get paintColor(): number {
    return this.paint.color.getHex()
  }

  /** A crash at `world` (from the server's event): the dent it makes goes there. */
  noteImpact(world: THREE.Vector3) {
    this.damage.noteImpact(world)
  }

  /** Shows crash `damage` and `hp` as the snapshot has them; parts that come off go into `debris`. */
  wear(damage: readonly number[], hp: number, debris: Debris | null, velocity: THREE.Vector3) {
    this.damage.update(damage, hp, debris, velocity)
  }

  /** Frees what this car owns; the model's geometry and other materials are shared by every car of its kind. */
  dispose() {
    this.damage.dispose()
    this.paint.dispose()
    for (const head of this.heads) (head.material as THREE.Material).dispose()
    this.heads[0]?.geometry.dispose()
    this.root.removeFromParent()
  }
}
