import { Quaternion, Vector3 } from '@ragnarok/engine'
import { FLAG_DEAD, FLAG_HELD_MASK, FLAG_HELD_SHIFT } from '@/net/codec'
import { CARRY_BACKPACK, CARRY_LAMP } from '@/sim/consts'
import type { LiftView } from '@/render/lifts'
import type { Avatars } from '../avatars'
import type { Frame, Presenter } from '../presenter'
import type { Sampled } from '../store'
import type { Entity } from '../units'
import type { Nearby } from './people'

/**
 * Everyone else riding a ski lift (`lifted`): their soldier sitting on the chair the lift view draws,
 * at the same clock, turned where they look (`heading`) and holding what they hold. The local player
 * is drawn from their own camera. LIFTS.md.
 */
export class LiftRiders implements Presenter<Frame> {
  /** Who rides where they can be heard this frame. */
  readonly seated: Nearby[] = []
  private readonly head = new Vector3()
  private readonly turn = new Quaternion()

  constructor(private readonly avatars: Avatars, private readonly lifts: () => LiftView | null, private readonly tick: () => number) {}

  wants(entity: Entity, id: number, frame: Frame) {
    return id !== frame.localId && entity.lifted !== undefined && entity.place !== undefined && ((entity.flags?.flags ?? 0) & FLAG_DEAD) === 0
  }

  begin() {
    this.seated.length = 0
  }

  update(id: number, { entity }: Sampled, frame: Frame) {
    const { lift, chair, seat } = entity.lifted!
    if (!this.lifts()?.rider(lift, chair, seat, this.tick(), this.head, this.turn)) return
    this.seated.push({ id, x: this.head.x, y: this.head.y, z: this.head.z })
    const avatar = this.avatars.draw(id)
    const flags = entity.flags?.flags ?? 0
    const worn = entity.carry?.what ?? 0
    avatar.dead = false
    avatar.swim = 0
    avatar.skis = false
    avatar.pack = (worn & CARRY_BACKPACK) !== 0
    avatar.lamp = (worn & CARRY_LAMP) !== 0
    avatar.held = (flags & FLAG_HELD_MASK) >> FLAG_HELD_SHIFT
    avatar.seat(this.head, this.turn, entity.heading?.yaw ?? 0, 0, frame.dt)
  }

  // Their soldier is let go by `Avatars.sweep` once nobody draws it.
  remove() {}
}
