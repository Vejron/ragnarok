import type { Pickups as PickupModels } from '@/render/pickups'
import type { Presenter, Frame } from '../presenter'
import type { Sampled } from '../store'
import type { Entity } from '../units'

/** An item lying on the ground: where, and which. */
export interface Lying {
  id: number
  item: number
  x: number
  y: number
  z: number
}

/** Items on the ground, spinning and bobbing where they lie; and the list of them, for the prompt. */
export class Pickups implements Presenter<Frame> {
  /** What lies about this frame. */
  readonly lying: Lying[] = []

  constructor(private readonly models: PickupModels) {}

  wants(entity: Entity) {
    return entity.pickup !== undefined && entity.place !== undefined
  }

  begin() {
    this.lying.length = 0
  }

  update(id: number, { entity }: Sampled) {
    const { x, y, z } = entity.place!
    this.lying.push({ id, item: entity.pickup!.item, x, y, z })
  }

  end(frame: Frame) {
    this.models.set(this.lying)
    this.models.update(frame.now / 1000)
  }

  // `end` sets the drawn ones to exactly this frame's.
  remove() {}
}
