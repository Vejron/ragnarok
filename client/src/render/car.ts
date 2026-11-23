import { Group, Mesh, type Material, type Node, Matrix4, Vector3 } from '@ragnarok/engine'
import { parseToonModel, unlitMaterial } from './models'
import { toonMaterial } from './toon'
import {
  AMV_MAX_STEER, AMV_SEATS, AMV_WHEEL_RADIUS, BUS_MAX_STEER, BUS_SEATS, BUS_WHEEL_RADIUS, EPA_MAX_STEER, EPA_SEATS, EPA_WHEEL_RADIUS, GOLF_CART_MAX_STEER, GOLF_CART_SEATS, GOLF_CART_WHEEL_RADIUS, TECHNICAL_MAX_STEER, TECHNICAL_SEATS, TECHNICAL_WHEEL_RADIUS, JEEP_MAX_STEER, JEEP_WHEEL_RADIUS, ROADSTER_MAX_STEER, ROADSTER_SEATS, ROADSTER_WHEEL_RADIUS, SNOWMOBILE_MAX_STEER, SNOWMOBILE_SEATS, SNOWMOBILE_WHEEL_RADIUS, SNOWRACER_MAX_STEER, SNOWRACER_SEATS, SNOWRACER_WHEEL_RADIUS, SPORTS_MAX_STEER, SPORTS_SEATS,
  SPORTS_WHEEL_RADIUS, CAR_SEATS,
  SAAB_900_MAX_STEER, SAAB_900_SEATS, SAAB_900_WHEEL_RADIUS, SAAB_9000_MAX_STEER, SAAB_9000_SEATS, SAAB_9000_WHEEL_RADIUS, VOLVO_240_MAX_STEER, VOLVO_240_SEATS, VOLVO_240_WHEEL_RADIUS, VOLVO_740_MAX_STEER, VOLVO_740_SEATS, VOLVO_740_WHEEL_RADIUS, GTI_MAX_STEER, GTI_SEATS, GTI_WHEEL_RADIUS, TRANSPORTER_MAX_STEER, TRANSPORTER_SEATS, TRANSPORTER_WHEEL_RADIUS, KADETT_MAX_STEER, KADETT_SEATS, KADETT_WHEEL_RADIUS, OMEGA_MAX_STEER, OMEGA_SEATS, OMEGA_WHEEL_RADIUS, SIERRA_MAX_STEER, SIERRA_SEATS, SIERRA_WHEEL_RADIUS, ESCORT_MAX_STEER, ESCORT_SEATS, ESCORT_WHEEL_RADIUS,
} from '@/sim/consts'
import { KIND_AMV, KIND_BUS, KIND_EPA, KIND_SAAB_900, KIND_SAAB_9000, KIND_VOLVO_240, KIND_VOLVO_740, KIND_GTI, KIND_TRANSPORTER, KIND_KADETT, KIND_OMEGA, KIND_SIERRA, KIND_ESCORT, KIND_GOLF_CART, KIND_JEEP, KIND_TECHNICAL, KIND_ROADSTER, KIND_SNOWMOBILE, KIND_SNOWRACER, KIND_SPORTS, THRUST_BOOST, THRUST_LIFT } from '@/net/codec'
import type { Arms, Turret } from '@/entities/units'
import { CarWear } from './carwear'
import type { Debris } from './debris'
import { halfExtentsOf } from '@/sim/kinds'
import { AMV_STEER_SHARES } from '@/sim/slip'

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

export type CarKind = 'jeep' | 'sports' | 'roadster' | 'snowmobile' | 'golf_cart' | 'technical' | 'snowracer' | 'bus' | 'amv' | 'epa' | 'saab_900' | 'saab_9000' | 'volvo_240' | 'volvo_740' | 'gti' | 'transporter' | 'kadett' | 'omega' | 'sierra' | 'escort'

/**
 * Per-body facts the model needs: its vehicle kind, where its mesh is, how big its wheels are and how far
 * they steer (and how far its second axle steers, for one that steers two), the colour it is painted
 * when every one of the kind wears the same or the colours it came in from the factory, and how opaque
 * its glass is when it can be seen through.
 */
const KINDS: Record<CarKind, { kind: number; url: string; wheelRadius: number; maxSteer: number; seats: number; paint?: number; palette?: readonly number[]; secondSteer?: number; glass?: number }> = {
  // The Bronco's two-tones: tan, red, blue, brown and white over the tub.
  jeep: { kind: KIND_JEEP, url: `${import.meta.env.BASE_URL}models/jeep.glb`, wheelRadius: JEEP_WHEEL_RADIUS, maxSteer: JEEP_MAX_STEER, seats: CAR_SEATS, palette: [0xc79a5b, 0x9b2a22, 0x2d4a78, 0x5a3b26, 0xe8e2d2] },
  // Signal yellow, red, white, black and silver, as the Mantas came; clear glass, so its four are seen.
  sports: { kind: KIND_SPORTS, url: `${import.meta.env.BASE_URL}models/sports.glb`, wheelRadius: SPORTS_WHEEL_RADIUS, maxSteer: SPORTS_MAX_STEER, seats: SPORTS_SEATS, palette: [0xf0c419, 0xb3161c, 0xf0eee8, 0x151517, 0xa4a8ab], glass: 0.4 },
  roadster: { kind: KIND_ROADSTER, url: `${import.meta.env.BASE_URL}models/roadster.glb`, wheelRadius: ROADSTER_WHEEL_RADIUS, maxSteer: ROADSTER_MAX_STEER, seats: ROADSTER_SEATS },
  // Its skis steer on the front pivots and do not roll; the idlers in its track do.
  snowmobile: { kind: KIND_SNOWMOBILE, url: `${import.meta.env.BASE_URL}models/snowmobile.glb`, wheelRadius: SNOWMOBILE_WHEEL_RADIUS, maxSteer: SNOWMOBILE_MAX_STEER, seats: SNOWMOBILE_SEATS },
  golf_cart: { kind: KIND_GOLF_CART, url: `${import.meta.env.BASE_URL}models/golf_cart.glb`, wheelRadius: GOLF_CART_WHEEL_RADIUS, maxSteer: GOLF_CART_MAX_STEER, seats: GOLF_CART_SEATS },
  technical: { kind: KIND_TECHNICAL, url: `${import.meta.env.BASE_URL}models/technical.glb`, wheelRadius: TECHNICAL_WHEEL_RADIUS, maxSteer: TECHNICAL_MAX_STEER, seats: TECHNICAL_SEATS },
  // One steering ski on its front pivot, and runners that are part of the body: nothing rolls.
  snowracer: { kind: KIND_SNOWRACER, url: `${import.meta.env.BASE_URL}models/snowracer.glb`, wheelRadius: SNOWRACER_WHEEL_RADIUS, maxSteer: SNOWRACER_MAX_STEER, seats: SNOWRACER_SEATS },
  bus: { kind: KIND_BUS, url: `${import.meta.env.BASE_URL}models/bus.glb`, wheelRadius: BUS_WHEEL_RADIUS, maxSteer: BUS_MAX_STEER, seats: BUS_SEATS },
  // Army green, whoever drives it.
  amv: { kind: KIND_AMV, url: `${import.meta.env.BASE_URL}models/amv.glb`, wheelRadius: AMV_WHEEL_RADIUS, maxSteer: AMV_MAX_STEER, seats: AMV_SEATS, paint: 0x5b6440, secondSteer: AMV_STEER_SHARES[1] },
  // Old white paint, as most of them wear, and clear glass, so the two in its cab are seen.
  epa: { kind: KIND_EPA, url: `${import.meta.env.BASE_URL}models/epa.glb`, wheelRadius: EPA_WHEEL_RADIUS, maxSteer: EPA_MAX_STEER, seats: EPA_SEATS, paint: 0xe9e6dc, glass: 0.4 },
  // Cherry red, silver, dark blue, racing green and beige.
  saab_900: { kind: KIND_SAAB_900, url: `${import.meta.env.BASE_URL}models/saab_900.glb`, wheelRadius: SAAB_900_WHEEL_RADIUS, maxSteer: SAAB_900_MAX_STEER, seats: SAAB_900_SEATS, palette: [0x8c1c22, 0xb9bcbf, 0x1f2a36, 0x2f4a3a, 0xd9cfae], glass: 0.4 },
  // Midnight blue, silver, burgundy, green and black.
  saab_9000: { kind: KIND_SAAB_9000, url: `${import.meta.env.BASE_URL}models/saab_9000.glb`, wheelRadius: SAAB_9000_WHEEL_RADIUS, maxSteer: SAAB_9000_MAX_STEER, seats: SAAB_9000_SEATS, palette: [0x1b2433, 0x9a9ea3, 0x5e1a1f, 0x3c4a3e, 0x111214], glass: 0.4 },
  // Navy, beige, dark red, white, olive and grey.
  volvo_240: { kind: KIND_VOLVO_240, url: `${import.meta.env.BASE_URL}models/volvo_240.glb`, wheelRadius: VOLVO_240_WHEEL_RADIUS, maxSteer: VOLVO_240_MAX_STEER, seats: VOLVO_240_SEATS, palette: [0x1e3252, 0xd8ccab, 0x7a1d1d, 0xe8e4d8, 0x4d5a3a, 0x8b8f92], glass: 0.4 },
  // Black, steel blue, silver, wine red and white.
  volvo_740: { kind: KIND_VOLVO_740, url: `${import.meta.env.BASE_URL}models/volvo_740.glb`, wheelRadius: VOLVO_740_WHEEL_RADIUS, maxSteer: VOLVO_740_MAX_STEER, seats: VOLVO_740_SEATS, palette: [0x2b2d30, 0x2a3b5c, 0xa8abad, 0x6b1e23, 0xe2ddd0], glass: 0.4 },
  // Tornado red, black, white, blue and grey.
  gti: { kind: KIND_GTI, url: `${import.meta.env.BASE_URL}models/gti.glb`, wheelRadius: GTI_WHEEL_RADIUS, maxSteer: GTI_MAX_STEER, seats: GTI_SEATS, palette: [0xb2141b, 0x0f0f10, 0xe9e7e1, 0x24365e, 0x7d8285], glass: 0.4 },
  // Beige, red, blue, white and olive.
  transporter: { kind: KIND_TRANSPORTER, url: `${import.meta.env.BASE_URL}models/transporter.glb`, wheelRadius: TRANSPORTER_WHEEL_RADIUS, maxSteer: TRANSPORTER_MAX_STEER, seats: TRANSPORTER_SEATS, palette: [0xd9cfae, 0x8a2a1d, 0x2f4f7a, 0xe8e4d8, 0x5d6b3a], glass: 0.4 },
  // White, signal yellow, blue, red and grey.
  kadett: { kind: KIND_KADETT, url: `${import.meta.env.BASE_URL}models/kadett.glb`, wheelRadius: KADETT_WHEEL_RADIUS, maxSteer: KADETT_MAX_STEER, seats: KADETT_SEATS, palette: [0xf2f0ea, 0xf0c419, 0x2c4f8c, 0xaa1f24, 0x8d9295], glass: 0.4 },
  // Graphite, dark blue, silver, maroon and white.
  omega: { kind: KIND_OMEGA, url: `${import.meta.env.BASE_URL}models/omega.glb`, wheelRadius: OMEGA_WHEEL_RADIUS, maxSteer: OMEGA_MAX_STEER, seats: OMEGA_SEATS, palette: [0x3a3f45, 0x1d2c4a, 0xa6a9ab, 0x4e1a22, 0xece9e1], glass: 0.4 },
  // Metallic blue, silver, red, white and black.
  sierra: { kind: KIND_SIERRA, url: `${import.meta.env.BASE_URL}models/sierra.glb`, wheelRadius: SIERRA_WHEEL_RADIUS, maxSteer: SIERRA_MAX_STEER, seats: SIERRA_SEATS, palette: [0x2b4f86, 0x9fa3a6, 0x7c1c20, 0xe6e2d6, 0x1a1b1d], glass: 0.4 },
  // Sand, pale blue, red, white and green.
  escort: { kind: KIND_ESCORT, url: `${import.meta.env.BASE_URL}models/escort.glb`, wheelRadius: ESCORT_WHEEL_RADIUS, maxSteer: ESCORT_MAX_STEER, seats: ESCORT_SEATS, palette: [0xc9b98d, 0x6f8e9e, 0xa7201f, 0xeeebe3, 0x3f5b3a], glass: 0.4 },
}

const BODY_COLORS = [0xff9f1c, 0x2ec4b6, 0xe71d36, 0x5aa9e6, 0xb5e48c, 0xf15bb5, 0xffd23f, 0x9d4edd]

const assets = new Map<CarKind, Node>()

/**
 * Parses `jeep.glb`, `sports.glb`, `roadster.glb`, `snowmobile.glb`, `golf_cart.glb`, `technical.glb`, `snowracer.glb`, `bus.glb`, `amv.glb`, `epa.glb` or one of the period cars' (built by `tools/vehicles/build_car.py`) for every `CarModel` of
 * that kind to share. Materials become flat toon ones to match the rest of the scene (`lamp` ones are
 * unlit, like a headlight); `paint` is replaced per car, and `glass` is see-through where the kind gives it an opacity.
 */
export async function parseCarModel(kind: CarKind, data: ArrayBuffer) {
  const { glass } = KINDS[kind]
  assets.set(
    kind,
    parseToonModel(data, (source) => {
      if (source.name === 'lamp') return unlitMaterial(source)
      if (source.name === 'glass' && glass !== undefined) return toonMaterial({ color: source.color, name: source.name, blend: 'alpha', opacity: glass, depthWrite: false })
      return null
    }),
  )
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

/** How far the headlights' beam is dipped below the nose: a rise over a run of 1. */
const BEAM_DIP = 0.08
/** How far ahead of the lamps' faces the beam starts, metres, so it does not light the bonnet behind them. */
const BEAM_AHEAD = 0.15

/**
 * The middle of `model`'s front lamps (`lamp` vertices ahead of its middle) in its frame, nudged forward
 * past their faces, or null if it has none.
 */
function frontLamps(model: Node): Vector3 | null {
  model.updateMatrixWorld(true)
  const toModel = new Matrix4().fromArray(model.matrixWorld).invert()
  const toFrame = new Matrix4()
  const v = new Vector3()
  const sum = new Vector3()
  let count = 0
  let front = Infinity
  model.traverse((node) => {
    if (!(node instanceof Mesh) || node.material.name !== 'lamp') return
    toFrame.multiplyMatrices(toModel, new Matrix4().fromArray(node.matrixWorld))
    const positions = node.geometry.positions
    for (let i = 0; i < positions.length; i += 3) {
      v.fromArray(positions, i).applyMatrix4(toFrame)
      if (v.z >= 0) continue
      sum.add(v)
      count++
      front = Math.min(front, v.z)
    }
  })
  if (count === 0) return null
  sum.divideScalar(count)
  return sum.setZ(front - BEAM_AHEAD)
}

/** A cold-gas jet leaving one of a Roadster's nozzles: where, and which way it blows, in world space. */
export interface Jet {
  at: Vector3
  direction: Vector3
}

/**
 * An open jeep, a low sports car or a Roadster, from the glTF the Blender script builds. Wheels roll with the
 * speed the interpolator reports and the front pair steers (an eight-wheeler's first two pairs). Its riders are drawn sitting by `Game`,
 * their heads on the `seat_N` markers of the model (`VehicleDef.seatHeads`).
 */
export class CarModel implements VehicleModel {
  readonly root = new Group()
  /** The steered wheels' pivots, and each one's share of the lock. */
  private readonly frontPivots: { pivot: Node; share: number }[] = []
  private readonly spinners: Node[] = []
  /** Each seat's head point in the car's frame, from the model's `seat_N` markers. */
  readonly seatMarkers: Vector3[] = []
  private readonly paint: Material
  private readonly wheelRadius: number
  private readonly maxSteer: number
  private roll = 0
  private placed = false
  private readonly last = new Vector3()
  private readonly forward = new Vector3()
  private readonly damage: CarWear
  /** A Roadster's nozzles in the car's frame (`nozzle_rear_N` push it forward, `nozzle_lift_N` up). */
  private readonly nozzles: { at: Vector3; lift: boolean }[] = []
  /** Where the headlights shine from in the car's frame: just ahead of the middle of the front lamps. */
  private readonly lamps: Vector3 | null
  private readonly beam = { at: new Vector3(), direction: new Vector3() }

  /** `name` is which of the car bodies it is. */
  constructor(name: CarKind, id: number) {
    const asset = assets.get(name)
    if (!asset) throw new Error('loadCarModels() must finish before cars are created')
    const { kind, wheelRadius, maxSteer, seats, paint: color, palette, secondSteer } = KINDS[name]
    this.wheelRadius = wheelRadius
    this.maxSteer = maxSteer
    const model = asset.clone()
    this.root.add(model)
    const paint = (this.paint = toonMaterial({ color: color ?? (palette ?? BODY_COLORS)[id % (palette ?? BODY_COLORS).length]! }))
    model.traverse((node) => {
      if (node instanceof Mesh && node.material.name === 'paint') node.material = paint
      // An eight-wheeler's middle axles are `wheel_FL2` and so on.
      if (/^wheel_[FR][LR]2?$/.test(node.name)) this.spinners.push(node)
      if (/^pivot_F[LR]$/.test(node.name)) this.frontPivots.push({ pivot: node, share: 1 })
      if (secondSteer !== undefined && /^pivot_F[LR]2$/.test(node.name)) this.frontPivots.push({ pivot: node, share: secondSteer })
      const nozzle = /^nozzle_(rear|lift)_\d+$/.exec(node.name)
      if (nozzle) this.nozzles.push({ at: new Vector3(node.position.x, node.position.y, node.position.z), lift: nozzle[1] === 'lift' })
    })

    this.damage = new CarWear(this.root, model, halfExtentsOf(kind), wheelRadius, paint)
    this.lamps = frontLamps(model)

    for (let seat = 0; seat < seats; seat++) {
      const marker = model.getObjectByName(`seat_${seat}`)!.position
      this.seatMarkers.push(new Vector3(marker.x, marker.y, marker.z))
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
    for (const { pivot, share } of this.frontPivots) pivot.rotation.y = -steer * this.maxSteer * share
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
      const direction = new Vector3(0, nozzle.lift ? -1 : 0, nozzle.lift ? 0 : 1).applyQuaternion(this.root.quaternion)
      jets.push({ at, direction })
    }
    return jets
  }

  /**
   * Its headlights' beam where the car was last placed: from the front lamps, along the nose and dipped a
   * little toward the road. Null for a model without front lamps. The vectors are reused by the next call.
   */
  headlight(): { at: Vector3; direction: Vector3 } | null {
    if (!this.lamps) return null
    const q = this.root.quaternion
    this.beam.at.copy(this.lamps).applyQuaternion(q).add(this.root.position)
    this.beam.direction.set(0, -BEAM_DIP, -1).normalize().applyQuaternion(q)
    return this.beam
  }

  /** The colour it was painted, for flying chips of it. */
  get paintColor(): number {
    return this.paint.color.getHex()
  }

  /** A crash at `world` (from the server's event): the dent it makes goes there. */
  noteImpact(world: Vector3) {
    this.damage.noteImpact(world)
  }

  /** Shows crash `damage` and `hp` as the snapshot has them; parts that come off go into `debris`. */
  wear(damage: readonly number[], hp: number, debris: Debris | null, velocity: Vector3) {
    this.damage.update(damage, hp, debris, velocity)
  }

  /** Frees what this car owns; the model's geometry and other materials are shared by every car of its kind. */
  dispose() {
    this.damage.dispose()
    this.paint.dispose()
    this.root.removeFromParent()
  }
}
