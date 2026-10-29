import { FLAG_CROUCHING, FLAG_DEAD, FLAG_GROUNDED, FLAG_HELD_MASK, FLAG_HELD_SHIFT, KIND_SOLDIER } from '@/net/codec'
import { SLIDE_MIN_SPEED, STANCE_CHUTE, STANCE_DOWN, STANCE_PRONE } from '@/sim/consts'
import type { Avatars } from '../avatars'
import type { Frame, Presenter } from '../presenter'
import type { Sampled } from '../store'
import type { Entity } from '../units'

/** Someone standing about, for the grass they part and the voices that carry. */
export interface Nearby {
  id: number
  x: number
  y: number
  z: number
}

export interface PeopleWorld {
  avatars: Avatars
  /** A contact shadow under a soldier whose capsule is centred at `(x, y, z)`. */
  blobFoot(x: number, y: number, z: number): void
  /** Splashes and ripples for someone at `(x, y, z)` moving at `speed`, m/s. */
  wade(id: number, x: number, y: number, z: number, speed: number, dt: number): void
}

/**
 * Everyone else on foot: their soldier, running, crouching, lying down or hanging from a parachute as
 * their flags say, holding what they hold. The local player is drawn from their prediction instead,
 * and riders by `Riders`, in their seats.
 */
export class Soldiers implements Presenter<Frame> {
  /** Who stands about this frame; the vehicles add their riders. */
  readonly nearby: Nearby[] = []
  /** The ids drawn on foot this frame. */
  readonly inView: number[] = []

  constructor(private readonly world: PeopleWorld) {}

  wants(entity: Entity, id: number, frame: Frame) {
    return id !== frame.localId && entity.kind?.kind === KIND_SOLDIER && entity.orientation === undefined && entity.place !== undefined && entity.flags !== undefined
  }

  begin() {
    this.nearby.length = 0
    this.inView.length = 0
  }

  update(id: number, { entity, velocity }: Sampled, frame: Frame) {
    const { x, y, z } = entity.place!
    const { flags, stance } = entity.flags!
    const dead = (flags & FLAG_DEAD) !== 0
    const grounded = (flags & FLAG_GROUNDED) !== 0
    const crouching = (flags & FLAG_CROUCHING) !== 0
    const knock = entity.motion ? Math.hypot(entity.motion.knock_x, entity.motion.knock_z) : 0
    this.inView.push(id)
    this.nearby.push({ id, x, y, z })
    const avatar = this.world.avatars.draw(id)
    avatar.dead = dead
    avatar.held = (flags & FLAG_HELD_MASK) >> FLAG_HELD_SHIFT
    avatar.pack = entity.carry !== undefined
    if (!dead) this.world.blobFoot(x, y, z)
    avatar.update([x, y, z], entity.heading?.yaw ?? 0, velocity[0], velocity[1], grounded, frame.dt, 0, crouching,
      crouching && grounded && knock > SLIDE_MIN_SPEED, stance === STANCE_PRONE || stance === STANCE_DOWN, stance === STANCE_CHUTE, entity.motion?.vy ?? 0)
    if (!dead) this.world.wade(id, x, y, z, Math.hypot(velocity[0], velocity[1]), frame.dt)
  }

  // A soldier not drawn in a frame is let go by `Avatars.sweep`: it may have taken a seat instead.
  remove() {}
}
