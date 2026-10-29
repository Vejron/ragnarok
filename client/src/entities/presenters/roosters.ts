import type { Node } from '@ragnarok/engine'
import { playCrow } from '@/audio'
import { FLAG_CROWING, FLAG_DEAD, KIND_ROOSTER } from '@/net/codec'
import { RoosterModel } from '@/render/rooster'
import { CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS } from '@/sim/consts'
import type { Frame, Presenter } from '../presenter'
import type { Sampled } from '../store'
import type { Entity } from '../units'

/** A crow can be heard this far off, metres. */
const ROOSTER_HEARING = 45

export interface RoosterWorld {
  /** Where models go. */
  root: Node
  /** Where the local player hears from. */
  listener: () => readonly number[]
  /** A contact shadow under a soldier-sized body centred at `(x, y, z)`. */
  blobFoot(x: number, y: number, z: number): void
}

/** The rooster: strutting, pecking and crowing, which is heard louder the nearer it is. */
export class Roosters implements Presenter<Frame> {
  private readonly models = new Map<number, RoosterModel>()

  constructor(private readonly world: RoosterWorld) {}

  wants(entity: Entity) {
    return entity.kind?.kind === KIND_ROOSTER && entity.orientation === undefined && entity.place !== undefined
  }

  update(id: number, { entity }: Sampled, frame: Frame) {
    const { x, y, z } = entity.place!
    const flags = entity.flags?.flags ?? 0
    let model = this.models.get(id)
    if (!model) {
      model = new RoosterModel()
      this.models.set(id, model)
      this.world.root.add(model.root)
    }
    if (model.update([x, y, z], entity.heading?.yaw ?? 0, (flags & FLAG_CROWING) !== 0, frame.dt)) {
      const listener = this.world.listener()
      const distance = Math.hypot(x - listener[0]!, z - listener[2]!)
      playCrow((1 - Math.min(1, distance / ROOSTER_HEARING)) ** 2)
    }
    if ((flags & FLAG_DEAD) === 0) this.world.blobFoot(x, y + CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS, z)
  }

  remove(id: number) {
    this.models.get(id)?.dispose()
    this.models.delete(id)
  }
}
