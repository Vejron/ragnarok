import type { LyingBackpacks } from '@/render/backpack'
import { KIND_BACKPACK } from '@/net/codec'
import type { Presenter, Frame } from '../presenter'
import type { Sampled } from '../store'
import type { Entity } from '../units'

/** The backpack lying on the ground (BACKPACK.md). Worn, it is drawn on its carrier's back by `Soldiers`. */
export class Backpacks implements Presenter<Frame> {
  private readonly lying: { id: number; x: number; y: number; z: number }[] = []

  constructor(private readonly models: LyingBackpacks) {}

  wants(entity: Entity) {
    return entity.kind?.kind === KIND_BACKPACK && entity.place !== undefined
  }

  begin() {
    this.lying.length = 0
  }

  update(id: number, { entity }: Sampled) {
    const { x, y, z } = entity.place!
    this.lying.push({ id, x, y, z })
  }

  end(frame: Frame) {
    this.models.set(this.lying, frame.now / 1000)
  }

  // `end` sets the drawn ones to exactly this frame's.
  remove() {}
}
