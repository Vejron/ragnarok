import { Quaternion, Vector3 } from '@ragnarok/engine'
import type { VehicleModel } from '@/render/car'
import { TowedGun } from '@/render/towedgun'
import { FLAG_HELD_MASK, FLAG_HELD_SHIFT, KIND_MEDIC, KIND_OFFICER } from '@/net/codec'
import { colorFor, knownPlayer, setPlayer } from '@/profile'
import { seatView } from '@/seats'
import { doorOf, seatHead } from '@/sim/vehicles'
import type { DoorStep } from '@/render/character'
import type { Avatars } from '../avatars'
import type { Frame, Presenter } from '../presenter'
import type { Sampled } from '../store'
import type { Entity } from '../units'
import type { Nearby } from './people'
import type { Vehicles } from './vehicles'

/** The team the emergency services are on (`TEAM_SERVICES` on the server): nobody's enemy. */
const SERVICES_TEAM = 254

/**
 * Where the rider of seat `index` of vehicle `model` (a `kind`) sits: their head point in the world
 * into `head` and the vehicle's turn (a towed gun's mount's) into `rotation`. False for a crew that
 * does not show, who are not drawn.
 */
export function seatFrame(model: VehicleModel, kind: number, index: number, head: Vector3, rotation: Quaternion, gunYaw = 0): boolean {
  // A gun's crew are on its turning mount: a mortar's, a Pak's or a ship's gun's.
  if (model instanceof TowedGun) {
    if (index !== 0) return false
    model.seatFrame(head, rotation)
    return true
  }
  const local = seatHead(kind, index, gunYaw)
  if (!local) return false
  const { position, quaternion } = model.root
  rotation.set(quaternion.x, quaternion.y, quaternion.z, quaternion.w)
  head.set(...local).applyQuaternion(rotation)
  head.x += position.x
  head.y += position.y
  head.z += position.z
  return true
}

/**
 * Whoever sits in seat `index` of vehicle `model` (a `kind`) and is `step` of the way through getting
 * in or out by its door (`Seated.step`): which way the door opens and where they stand once out, in
 * the world, into `into`. Null for someone sitting still, or a seat with no door.
 */
export function doorStep(model: VehicleModel, kind: number, index: number, step: number, into: DoorStep): DoorStep | null {
  const door = doorOf(kind, index)
  if (!door || step <= 0) return null
  const { position, quaternion } = model.root
  into.spot.set(...door.spot).applyQuaternion(quaternion).add(position)
  into.step = step
  into.side = door.hinge[0] < 0 ? -1 : 1
  return into
}

/**
 * Everyone else sitting in a vehicle whose crew show (`seated`, with no place of their own): their
 * soldier in the seat, turned where they look (`heading`, `look`), holding what they hold (`flags`).
 * The local player is drawn from their own camera. Runs after the vehicles are placed, since a rider
 * sits where the model is drawn.
 */
export class Riders implements Presenter<Frame> {
  /** Who sits where they can be heard this frame. */
  readonly seated: Nearby[] = []
  private readonly head = new Vector3()
  private readonly turn = new Quaternion()
  private readonly door: DoorStep = { step: 0, side: 0, spot: new Vector3() }

  constructor(private readonly avatars: Avatars, private readonly vehicles: Vehicles) {}

  wants(entity: Entity, id: number, frame: Frame) {
    return id !== frame.localId && entity.seated !== undefined && entity.place === undefined
  }

  begin() {
    this.seated.length = 0
  }

  update(id: number, { entity }: Sampled, frame: Frame) {
    const { vehicle, seat, step } = entity.seated!
    const model = this.vehicles.model(vehicle)
    const car = this.vehicles.placed.get(vehicle)
    if (!model || !car || !seatFrame(model, car.kind, seat, this.head, this.turn, car.turret?.yaw)) return
    this.seated.push({ id, x: this.head.x, y: this.head.y, z: this.head.z })
    const avatar = this.avatars.draw(id)
    // The crew of a police car or an ambulance at the wheel has no profile until they get out, when
    // the server gives them the services' colour, 0: they wear it in the car too, the same people.
    const kind = entity.kind?.kind
    if ((kind === KIND_OFFICER || kind === KIND_MEDIC) && !knownPlayer(id)) {
      setPlayer(id, kind === KIND_OFFICER ? 'Police' : 'Medic', 0, SERVICES_TEAM, true)
      avatar.setColor(colorFor(id))
    }
    avatar.dead = false
    avatar.held = ((entity.flags?.flags ?? 0) & FLAG_HELD_MASK) >> FLAG_HELD_SHIFT
    avatar.seat(this.head, this.turn, entity.heading?.yaw ?? car.yaw, entity.look?.pitch ?? 0, frame.dt, seatView(car.kind, seat).stand === true, doorStep(model, car.kind, seat, step, this.door))
  }

  // Their soldier is let go by `Avatars.sweep` once nobody draws it.
  remove() {}
}
