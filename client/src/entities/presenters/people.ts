import type { Character } from '@/render/character'
import { FLAG_CLAWING, FLAG_CROUCHING, FLAG_DEAD, FLAG_GROUNDED, FLAG_HELD_MASK, FLAG_HELD_SHIFT, KIND_SOLDIER, KIND_ZOMBIE } from '@/net/codec'
import { CARRY_BACKPACK, CARRY_LAMP, CARRY_POLING, CARRY_SKIS, SLIDE_MIN_SPEED, STANCE_CHUTE, STANCE_DIVE, STANCE_DOWN, STANCE_PRONE, STANCE_RISE, STANCE_SURRENDER, STANCE_SWIM } from '@/sim/consts'
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
  /** The light of `avatar`'s headlamp, if lit. */
  headlamp(avatar: Character): void
  /** A zombie at `(x, y, z)`, `clawing` or not, for its groans and the swish of its claws. */
  zombie(id: number, x: number, y: number, z: number, clawing: boolean, dt: number): void
  /** The grooves and spray of a skier at `(x, z)` going `(vx, vz)`. */
  skiRun(id: number, x: number, z: number, vx: number, vz: number, grounded: boolean, dt: number): void
}

/**
 * Everyone else on foot: their soldier, running, crouching, lying down or hanging from a parachute as
 * their flags say, holding what they hold. A zombie is a soldier too, drawn in rags and clawing when
 * its flags say so. The local player is drawn from their prediction instead,
 * and riders by `Riders`, in their seats.
 */
export class Soldiers implements Presenter<Frame> {
  /** Who stands about this frame; the vehicles add their riders. */
  readonly nearby: Nearby[] = []
  /** The ids drawn on foot this frame. */
  readonly inView: number[] = []

  constructor(private readonly world: PeopleWorld) {}

  wants(entity: Entity, id: number, frame: Frame) {
    return id !== frame.localId && (entity.kind?.kind === KIND_SOLDIER || entity.kind?.kind === KIND_ZOMBIE) && entity.orientation === undefined && entity.place !== undefined && entity.flags !== undefined
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
    const zombie = entity.kind!.kind === KIND_ZOMBIE
    avatar.zombie = zombie
    const clawing = zombie && (flags & FLAG_CLAWING) !== 0
    avatar.clawing = clawing
    const buried = zombie && !dead && stance === STANCE_RISE
    avatar.graveRise = buried
    if (zombie && !dead) this.world.zombie(id, x, y, z, clawing, frame.dt)
    const swim = stance === STANCE_DIVE ? 2 : stance === STANCE_SWIM ? 1 : 0
    avatar.swim = dead ? 0 : swim
    avatar.dead = dead
    avatar.held = (flags & FLAG_HELD_MASK) >> FLAG_HELD_SHIFT
    const worn = entity.carry?.what ?? 0
    avatar.pack = (worn & CARRY_BACKPACK) !== 0
    avatar.skis = (worn & CARRY_SKIS) !== 0 && swim === 0
    avatar.lamp = (worn & CARRY_LAMP) !== 0 && !dead
    avatar.skiPoling = (worn & CARRY_POLING) !== 0
    avatar.surrendering = stance === STANCE_SURRENDER
    if (!dead && swim === 0 && !buried) this.world.blobFoot(x, y, z)
    avatar.update([x, y, z], entity.heading?.yaw ?? 0, velocity[0], velocity[1], grounded, frame.dt, 0, crouching,
      crouching && grounded && knock > SLIDE_MIN_SPEED, stance === STANCE_PRONE || stance === STANCE_DOWN, stance === STANCE_CHUTE, entity.motion?.vy ?? 0)
    if (!dead && !buried) this.world.wade(id, x, y, z, Math.hypot(velocity[0], velocity[1]), frame.dt)
    this.world.headlamp(avatar)
    if (avatar.skis && !dead) this.world.skiRun(id, x, z, velocity[0], velocity[1], grounded, frame.dt)
  }

  // A soldier not drawn in a frame is let go by `Avatars.sweep`: it may have taken a seat instead.
  remove() {}
}
