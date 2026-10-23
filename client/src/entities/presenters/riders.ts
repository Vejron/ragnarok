import * as THREE from 'three'
import type { VehicleModel } from '@/render/car'
import { MortarModel } from '@/render/mortar'
import { FLAG_HELD_MASK, FLAG_HELD_SHIFT } from '@/net/codec'
import { seatHead } from '@/sim/vehicles'
import type { Avatars } from '../avatars'
import type { Frame, Presenter } from '../presenter'
import type { Sampled } from '../store'
import type { Entity } from '../units'
import type { Nearby } from './people'
import type { Vehicles } from './vehicles'

/**
 * Where the rider of seat `index` of vehicle `model` (a `kind`) sits: their head point in the world
 * into `head` and the vehicle's turn (the mortar's mount's) into `rotation`. False for a crew that
 * does not show, who are not drawn.
 */
export function seatFrame(model: VehicleModel, kind: number, index: number, head: THREE.Vector3, rotation: THREE.Quaternion): boolean {
  if (model instanceof MortarModel) {
    if (index !== 0) return false
    model.seatFrame(head, rotation)
    return true
  }
  const local = seatHead(kind, index)
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
 * Everyone else sitting in a vehicle whose crew show (`seated`, with no place of their own): their
 * stickman in the seat, turned where they look (`heading`, `look`), holding what they hold (`flags`).
 * The local player is drawn from their own camera. Runs after the vehicles are placed, since a rider
 * sits where the model is drawn.
 */
export class Riders implements Presenter<Frame> {
  /** Who sits where they can be heard this frame. */
  readonly seated: Nearby[] = []
  private readonly head = new THREE.Vector3()
  private readonly turn = new THREE.Quaternion()

  constructor(private readonly avatars: Avatars, private readonly vehicles: Vehicles) {}

  wants(entity: Entity, id: number, frame: Frame) {
    return id !== frame.localId && entity.seated !== undefined && entity.place === undefined
  }

  begin() {
    this.seated.length = 0
  }

  update(id: number, { entity }: Sampled, frame: Frame) {
    const { vehicle, seat } = entity.seated!
    const model = this.vehicles.model(vehicle)
    const car = this.vehicles.placed.get(vehicle)
    if (!model || !car || !seatFrame(model, car.kind, seat, this.head, this.turn)) return
    this.seated.push({ id, x: this.head.x, y: this.head.y, z: this.head.z })
    const avatar = this.avatars.draw(id)
    avatar.dead = false
    avatar.held = ((entity.flags?.flags ?? 0) & FLAG_HELD_MASK) >> FLAG_HELD_SHIFT
    avatar.seat(this.head, this.turn, entity.heading?.yaw ?? car.yaw, entity.look?.pitch ?? 0, frame.dt)
  }

  // Their stickman is let go by `Avatars.sweep` once nobody draws it.
  remove() {}
}
