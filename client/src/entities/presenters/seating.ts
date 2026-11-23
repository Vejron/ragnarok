import { KIND_CIVILIAN } from '@/net/codec'
import { vehicleDef } from '@/sim/vehicles'
import type { Frame, Presenter } from '../presenter'
import type { Sampled } from '../store'
import type { Entity } from '../units'

/**
 * Who sits where, from every rider's `seated`: run before the vehicles, which draw their seats as
 * taken or free (a rotor turning, a canopy shut) and tell the prompts which seat is free.
 */
export class Seating implements Presenter<Frame> {
  private readonly riders = new Map<number, number[]>()
  /** The vehicles with a driver of the traffic at the wheel. */
  private readonly traffic = new Set<number>()

  wants(entity: Entity) {
    return entity.seated !== undefined
  }

  begin() {
    this.riders.clear()
    this.traffic.clear()
  }

  update(id: number, { entity }: Sampled) {
    const { vehicle, seat } = entity.seated!
    let seats = this.riders.get(vehicle)
    if (!seats) this.riders.set(vehicle, (seats = []))
    seats[seat] = id
    if (seat === 0 && entity.kind?.kind === KIND_CIVILIAN) this.traffic.add(vehicle)
  }

  /** Whether vehicle `id` is a car of the traffic with its driver in it, who is dragged out by whoever boards it. */
  manned(id: number): boolean {
    return this.traffic.has(id)
  }

  /** The player in each seat of vehicle `id` (a `kind`), 0 for an empty one; seat 0 drives. */
  of(id: number, kind: number): readonly number[] {
    const count = vehicleDef(kind).seats
    const seats = this.riders.get(id)
    if (!seats) return Array.from({ length: count }, () => 0)
    for (let index = 0; index < count; index++) seats[index] ??= 0
    return seats
  }

  remove() {}
}
