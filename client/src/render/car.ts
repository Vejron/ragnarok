import { Group, Mesh, type Material, type Node } from '@ragnarok/engine'
import * as THREE from 'three'
import { parseToonModel, unlitMaterial } from './models'
import { toonMaterial } from './toon'
import { JEEP_MAX_STEER, JEEP_WHEEL_RADIUS, ROADSTER_MAX_STEER, ROADSTER_SEATS, ROADSTER_WHEEL_RADIUS, SPORTS_MAX_STEER, SPORTS_SEATS, SPORTS_WHEEL_RADIUS, CAR_SEATS } from '@/sim/consts'
import { KIND_JEEP, KIND_ROADSTER, KIND_SPORTS, THRUST_BOOST, THRUST_LIFT } from '@/net/codec'
import type { Arms, Turret } from '@/entities/units'
import { CarWear } from './carwear'
import type { Debris } from './debris'
import { halfExtentsOf } from '@/sim/kinds'

/** What `Game` needs from a vehicle's model, whatever the kind. */
export interface VehicleModel {
  readonly root: Node
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    steer: number,
    seats: readonly number[],
    turret?: Turret | null,
    arms?: Arms | null,
  ): void
  dispose(): void
}

export type CarKind = 'jeep' | 'sports' | 'roadster'

/** Per-body facts the model needs: its vehicle kind, where its mesh is, how big its wheels are and how far they steer. */
const KINDS: Record<CarKind, { kind: number; url: string; wheelRadius: number; maxSteer: number; seats: number }> = {
  jeep: { kind: KIND_JEEP, url: `${import.meta.env.BASE_URL}models/jeep.glb`, wheelRadius: JEEP_WHEEL_RADIUS, maxSteer: JEEP_MAX_STEER, seats: CAR_SEATS },
  sports: { kind: KIND_SPORTS, url: `${import.meta.env.BASE_URL}models/sports.glb`, wheelRadius: SPORTS_WHEEL_RADIUS, maxSteer: SPORTS_MAX_STEER, seats: SPORTS_SEATS },
  roadster: { kind: KIND_ROADSTER, url: `${import.meta.env.BASE_URL}models/roadster.glb`, wheelRadius: ROADSTER_WHEEL_RADIUS, maxSteer: ROADSTER_MAX_STEER, seats: ROADSTER_SEATS },
}

const BODY_COLORS = [0xff9f1c, 0x2ec4b6, 0xe71d36, 0x5aa9e6, 0xb5e48c, 0xf15bb5, 0xffd23f, 0x9d4edd]

const assets = new Map<CarKind, Node>()

/**
 * Parses `jeep.glb`, `sports.glb` or `roadster.glb` (built by `tools/vehicles/build_car.py`) for every `CarModel` of
 * that kind to share. Materials become flat toon ones to match the rest of the scene (`lamp` ones are
 * unlit, like a headlight); `paint` is replaced per car.
 */
export async function parseCarModel(kind: CarKind, data: ArrayBuffer) {
  assets.set(kind, parseToonModel(data, (source) => (source.name === 'lamp' ? unlitMaterial(source) : null)))
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

/** A cold-gas jet leaving one of a Roadster's nozzles: where, and which way it blows, in world space. */
export interface Jet {
  at: THREE.Vector3
  direction: THREE.Vector3
}

/**
 * An open jeep, a low sports car or a Roadster, from the glTF the Blender script builds. Wheels roll with the
 * speed the interpolator reports and the front pair steers. Its riders are drawn sitting by `Game`,
 * their heads on the `seat_N` markers of the model (`VehicleDef.seatHeads`).
 */
export class CarModel implements VehicleModel {
  readonly root = new Group()
  private readonly frontPivots: Node[] = []
  private readonly spinners: Node[] = []
  /** Each seat's head point in the car's frame, from the model's `seat_N` markers. */
  readonly seatMarkers: THREE.Vector3[] = []
  private readonly paint: Material
  private readonly wheelRadius: number
  private readonly maxSteer: number
  private roll = 0
  private placed = false
  private readonly last = new THREE.Vector3()
  private readonly forward = new THREE.Vector3()
  private readonly damage: CarWear
  /** A Roadster's nozzles in the car's frame (`nozzle_rear_N` push it forward, `nozzle_lift_N` up). */
  private readonly nozzles: { at: THREE.Vector3; lift: boolean }[] = []

  /** `name` is which of the car bodies it is. */
  constructor(name: CarKind, id: number) {
    const asset = assets.get(name)
    if (!asset) throw new Error('loadCarModels() must finish before cars are created')
    const { kind, wheelRadius, maxSteer, seats } = KINDS[name]
    this.wheelRadius = wheelRadius
    this.maxSteer = maxSteer
    const model = asset.clone()
    this.root.add(model)
    const paint = (this.paint = toonMaterial({ color: BODY_COLORS[id % BODY_COLORS.length]! }))
    model.traverse((node) => {
      if (node instanceof Mesh && node.material.name === 'paint') node.material = paint
      if (/^wheel_[FR][LR]$/.test(node.name)) this.spinners.push(node)
      if (/^pivot_F[LR]$/.test(node.name)) this.frontPivots.push(node)
      const nozzle = /^nozzle_(rear|lift)_\d+$/.exec(node.name)
      if (nozzle) this.nozzles.push({ at: new THREE.Vector3(node.position.x, node.position.y, node.position.z), lift: nozzle[1] === 'lift' })
    })

    this.damage = new CarWear(this.root, model, halfExtentsOf(kind), wheelRadius, paint)

    for (let seat = 0; seat < seats; seat++) {
      const marker = model.getObjectByName(`seat_${seat}`)!.position
      this.seatMarkers.push(new THREE.Vector3(marker.x, marker.y, marker.z))
    }
  }

  /**
   * Places the car and animates it. The wheels roll by however far the car moved along its nose
   * since the last call (so reversing spins them backwards), `steer` is the front wheel fraction
   * of full lock.
   */
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    steer: number,
  ) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    this.forward.set(0, 0, -1).applyQuaternion(this.root.quaternion)
    if (this.placed) this.roll -= (this.forward.x * (position[0] - this.last.x) + this.forward.y * (position[1] - this.last.y) + this.forward.z * (position[2] - this.last.z)) / this.wheelRadius
    this.last.set(position[0], position[1], position[2])
    this.placed = true
    for (const spinner of this.spinners) spinner.rotation.x = this.roll
    // Turning right (positive) is a clockwise yaw, which is a negative rotation about +Y.
    for (const pivot of this.frontPivots) pivot.rotation.y = -steer * this.maxSteer
  }

  /**
   * The jets of the nozzles firing for `flags` (`THRUST_*`), where the car was last placed: the rear
   * ones blow out behind it, the lift ones straight down.
   */
  jets(flags: number): Jet[] {
    const jets: Jet[] = []
    for (const nozzle of this.nozzles) {
      if (!(flags & (nozzle.lift ? THRUST_LIFT : THRUST_BOOST))) continue
      const at = nozzle.at.clone().applyQuaternion(this.root.quaternion).add(this.root.position)
      const direction = new THREE.Vector3(0, nozzle.lift ? -1 : 0, nozzle.lift ? 0 : 1).applyQuaternion(this.root.quaternion)
      jets.push({ at, direction })
    }
    return jets
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
    this.root.removeFromParent()
  }
}
