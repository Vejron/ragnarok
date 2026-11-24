import type { Character } from '@/render/character'
import { FLAG_CLAWING, FLAG_CROUCHING, FLAG_DEAD, FLAG_GROUNDED, FLAG_HELD_MASK, FLAG_HELD_SHIFT, KIND_CIVILIAN, KIND_SOLDIER, KIND_ZOMBIE } from '@/net/codec'
import { CARRY_BACKPACK, CARRY_PARCEL, CARRY_LAMP, CARRY_POLING, CARRY_SKIS, SLIDE_MIN_SPEED, STANCE_CHUTE, STANCE_DIVE, STANCE_DOWN, STANCE_PRONE, STANCE_RISE, STANCE_SURRENDER, STANCE_SWIM } from '@/sim/consts'
import type { Avatars } from '../avatars'
import { ghostly } from './ghosts'
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
  /**
   * How far the water at `(x, z)` has risen, metres, from server tick `tick` to the drawn water's now: a
   * swimmer is shown as of the render tick, a little behind the water.
   */
  rewave(x: number, z: number, tick: number): number
  /** The current at `(x, z)`, `[x, z]` m/s, that carries a swimmer there. */
  current(x: number, z: number): [number, number]
  /** A foot of someone whose capsule is centred at `(x, y, z)` coming down, `weight` 0 a shuffle to 1 a sprint. */
  footfall(x: number, y: number, z: number, weight: number): void
  /** The grooves and spray of a skier at `(x, z)` going `(vx, vz)`. */
  skiRun(id: number, x: number, z: number, vx: number, vz: number, grounded: boolean, dt: number): void
}

/**
 * Everyone else on foot: their soldier, running, crouching, lying down or hanging from a parachute as
 * their flags say, holding what they hold. A zombie is a soldier too, drawn in rags and clawing when
 * its flags say so, and a driver of the traffic who is out of their car is one with nothing in hand. A ghost is drawn by `Ghosts` instead. The local player is drawn from their prediction instead,
 * riders by `Riders`, in their seats, and those on a ski lift by `LiftRiders`, on its chairs.
 */
export class Soldiers implements Presenter<Frame> {
  /** Who stands about this frame; the vehicles add their riders. */
  readonly nearby: Nearby[] = []
  /** The ids drawn on foot this frame. */
  readonly inView: number[] = []
  /** Everyone drawn as a civilian so far: nobody's kill, and no one to list on the board. */
  readonly civilians = new Set<number>()

  constructor(private readonly world: PeopleWorld) {}

  wants(entity: Entity, id: number, frame: Frame) {
    return id !== frame.localId && (entity.kind?.kind === KIND_SOLDIER || entity.kind?.kind === KIND_ZOMBIE || entity.kind?.kind === KIND_CIVILIAN) && entity.orientation === undefined && entity.place !== undefined && entity.flags !== undefined && entity.lifted === undefined && !ghostly(entity.flags.stance)
  }

  begin() {
    this.nearby.length = 0
    this.inView.length = 0
  }

  update(id: number, { entity, velocity }: Sampled, frame: Frame) {
    const { x, z } = entity.place!
    const { flags, stance } = entity.flags!
    const dead = (flags & FLAG_DEAD) !== 0
    // A swimmer floats on the waves of the render tick; carried on to the water's now, they bob with it as drawn.
    const swimming = !dead && (stance === STANCE_SWIM || stance === STANCE_DIVE)
    const y = entity.place!.y + (swimming ? this.world.rewave(x, z, frame.tick) : 0)
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
    if (entity.kind!.kind === KIND_CIVILIAN) this.civilians.add(id)
    avatar.held = (flags & FLAG_HELD_MASK) >> FLAG_HELD_SHIFT
    const worn = entity.carry?.what ?? 0
    avatar.pack = (worn & CARRY_BACKPACK) !== 0
    avatar.parcel = (worn & CARRY_PARCEL) !== 0
    avatar.skis = (worn & CARRY_SKIS) !== 0 && swim === 0
    avatar.lamp = (worn & CARRY_LAMP) !== 0 && !dead
    avatar.skiPoling = (worn & CARRY_POLING) !== 0
    avatar.surrendering = stance === STANCE_SURRENDER
    if (!dead && swim === 0 && !buried) this.world.blobFoot(x, y, z)
    // A swimmer strokes, and stirs the water, as fast as they go through it: the current carrying them is not theirs.
    const [cx, cz] = swimming ? this.world.current(x, z) : [0, 0]
    const vx = velocity[0] - cx
    const vz = velocity[1] - cz
    avatar.update([x, y, z], entity.heading?.yaw ?? 0, vx, vz, grounded, frame.dt, 0, crouching,
      crouching && grounded && knock > SLIDE_MIN_SPEED, stance === STANCE_PRONE || stance === STANCE_DOWN, stance === STANCE_CHUTE, entity.motion?.vy ?? 0)
    for (const weight of avatar.takeSteps()) if (!dead && !buried) this.world.footfall(x, y, z, weight)
    if (!dead && !buried) this.world.wade(id, x, y, z, Math.hypot(vx, vz), frame.dt)
    this.world.headlamp(avatar)
    if (avatar.skis && !dead) this.world.skiRun(id, x, z, velocity[0], velocity[1], grounded, frame.dt)
  }

  // A soldier not drawn in a frame is let go by `Avatars.sweep`: it may have taken a seat instead.
  remove() {}
}
