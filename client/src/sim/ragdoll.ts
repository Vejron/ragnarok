/**
 * Which soldiers are ragdolls this frame (KNOCKDOWN.md). A soldier knocked down is one, thrown as they
 * were and following where the server has them, until they are up again. Every death is one too: the
 * body flies where the killing blow and whatever threw them sent it, tethered to nothing, loses the limbs
 * that come off on the way, and once it lies still it is frozen as it lies (`freezeRagdoll`), costing
 * nothing until the soldier respawns. The physics is `ClientSim`'s (`crates/sim/src/ragdoll.rs`).
 */

import type { SimMotor, Vec3 } from './motor'

/** The most ragdolls moving at once. A knockdown past it just lies down; a death past it freezes the
 * body that has been dead longest, as it lies, to make room. */
export const RAGDOLL_CAP = 8
/** How fast a thrown soldier turns head over heels, rad/s per m/s it was thrown at. */
const SPIN_PER_SPEED = 0.45
/** The head's part (`crates/sim/src/ragdoll.rs`, `PARTS`). */
export const RAGDOLL_HEAD = 2

export type RagdollPhysics = Pick<
  SimMotor,
  'spawnRagdoll' | 'setRagdollTarget' | 'clearRagdollTarget' | 'pushRagdoll' | 'severRagdoll' | 'ragdollAsleep' | 'stepRagdolls' | 'ragdollPose' | 'removeRagdoll'
>

/** What a ragdoll poses: a soldier's `Character`. */
export interface RagdollBody {
  /** Drawn and in one piece. */
  readonly intact: boolean
  /** Dead and lying as its ragdoll left it. */
  readonly frozen: boolean
  ragdollSegments(): { segments: Float32Array; side: Vec3 }
  startRagdoll(pose: ArrayLike<number>): void
  driveRagdoll(pose: ArrayLike<number>): void
  endRagdoll(rise?: boolean): void
  freezeRagdoll(): void
  /** The killing blow's throw, if one is waiting: the whole body's velocity and the head's on top, m/s. */
  takeThrow(): { velocity: Vec3; head: Vec3 } | null
}

/** A soldier drawn this frame, as far as ragdolls go. */
export interface Knockable {
  key: number
  body: RagdollBody
  /** Knocked down (`STANCE_DOWN`). */
  down: boolean
  dead: boolean
  /** Where the server (or the prediction) has them. */
  position: Vec3
  /** How they are moving: the throw, as the knockdown or the death starts. */
  velocity: Vec3
  /** Limbs lost (`LIMB_*`): no part for them, and one lost while a ragdoll comes off it. */
  lost: number
}

interface Doll {
  dead: boolean
  /** The limbs it was made or left without. */
  lost: number
  /** When it died, in `Ragdolls` frames: the longest dead is frozen first. */
  since: number
}

export class Ragdolls {
  private readonly dolls = new Map<number, Doll>()
  private frame = 0

  constructor(
    private readonly physics: RagdollPhysics,
    readonly cap = RAGDOLL_CAP,
  ) {}

  /** How many ragdolls there are. */
  get count() {
    return this.dolls.size
  }

  has(key: number) {
    return this.dolls.has(key)
  }

  /** Brings the ragdolls into line with `soldiers` (all those drawn), runs them on by `dt` and poses them. */
  update(soldiers: readonly Knockable[], camera: Vec3, dt: number) {
    this.frame++
    const seen = new Set<number>()
    const knocked: Knockable[] = []
    const dying: Knockable[] = []
    for (const soldier of soldiers) {
      const { key, body, dead } = soldier
      seen.add(key)
      const doll = this.dolls.get(key)
      const wants = body.intact && !body.frozen && (soldier.down || dead)
      if (doll && !wants) this.drop(key, body, !dead)
      else if (doll) this.carry(key, doll, soldier)
      else if (wants) (dead ? dying : knocked).push(soldier)
    }
    for (const key of [...this.dolls.keys()]) if (!seen.has(key)) this.drop(key, null, false)
    // Every death gets a ragdoll, freezing the longest dead to make room; a knockdown only if there is room.
    for (const soldier of dying) {
      while (this.dolls.size >= this.cap && this.freezeOldest(soldiers)) {
        // Freed one.
      }
      this.spawn(soldier)
    }
    const away = (at: Vec3) => (at[0] - camera[0]) ** 2 + (at[2] - camera[2]) ** 2
    knocked.sort((a, b) => away(a.position) - away(b.position))
    for (const soldier of knocked) {
      if (this.dolls.size >= this.cap) break
      this.spawn(soldier)
    }
    if (this.dolls.size === 0) return
    this.physics.stepRagdolls(dt)
    for (const { key, body } of soldiers) {
      if (!this.dolls.has(key)) continue
      const pose = this.physics.ragdollPose(key)
      if (pose) body.driveRagdoll(pose)
    }
  }

  /** Lets every ragdoll go: the world is going away. */
  clear() {
    for (const key of this.dolls.keys()) this.physics.removeRagdoll(key)
    this.dolls.clear()
  }

  /** One frame of a ragdoll that stays one: tethered while alive, free once dead, losing limbs and taking blows. */
  private carry(key: number, doll: Doll, { body, dead, position, lost }: Knockable) {
    if (dead && !doll.dead) {
      // Died down: let go, to fall as it will.
      doll.dead = true
      doll.since = this.frame
      this.physics.clearRagdollTarget(key)
    }
    if (!dead) this.physics.setRagdollTarget(key, position)
    const severed = lost & ~doll.lost
    if (severed) {
      this.physics.severRagdoll(key, severed)
      doll.lost |= severed
    }
    const thrown = body.takeThrow()
    if (thrown) this.throw(key, thrown)
    // Lying still a while (the sim's `REST_AFTER`): frozen as it lies.
    if (dead && this.physics.ragdollAsleep(key)) this.freeze(key, body)
  }

  private spawn({ key, body, position, velocity, lost, dead }: Knockable) {
    const { segments, side } = body.ragdollSegments()
    const thrown = body.takeThrow()
    const v: Vec3 = thrown ? [velocity[0] + thrown.velocity[0], velocity[1] + thrown.velocity[1], velocity[2] + thrown.velocity[2]] : velocity
    // Head over heels the way it was thrown: about the level axis across the throw.
    const [vx, , vz] = v
    const speed = Math.hypot(vx, vz)
    const spin: Vec3 = speed > 1e-3 ? [vz * SPIN_PER_SPEED, 0, -vx * SPIN_PER_SPEED] : [0, 0, 0]
    if (!this.physics.spawnRagdoll(key, segments, side, v, spin, lost)) return
    const pose = this.physics.ragdollPose(key)
    if (!pose) return
    if (dead) this.physics.clearRagdollTarget(key)
    else this.physics.setRagdollTarget(key, position)
    if (thrown) this.physics.pushRagdoll(key, thrown.head, RAGDOLL_HEAD)
    body.startRagdoll(pose)
    this.dolls.set(key, { dead, lost, since: this.frame })
  }

  /** Carries a blow that came after the ragdoll was made: the whole body, and the head on top. */
  private throw(key: number, { velocity, head }: { velocity: Vec3; head: Vec3 }) {
    this.physics.pushRagdoll(key, velocity)
    if (head[0] || head[1] || head[2]) this.physics.pushRagdoll(key, head, RAGDOLL_HEAD)
  }

  /** Freezes the body that has been dead longest, of those drawn. Whether there was one. */
  private freezeOldest(soldiers: readonly Knockable[]) {
    let oldest: { key: number; since: number } | null = null
    for (const [key, doll] of this.dolls) if (doll.dead && (!oldest || doll.since < oldest.since)) oldest = { key, since: doll.since }
    const body = oldest && soldiers.find((s) => s.key === oldest.key)?.body
    if (!oldest || !body) return false
    this.freeze(oldest.key, body)
    return true
  }

  private freeze(key: number, body: RagdollBody) {
    this.physics.removeRagdoll(key)
    this.dolls.delete(key)
    body.freezeRagdoll()
  }

  private drop(key: number, body: RagdollBody | null, rise: boolean) {
    this.physics.removeRagdoll(key)
    this.dolls.delete(key)
    body?.endRagdoll(rise)
  }
}
