/**
 * Which soldiers are ragdolls this frame (KNOCKDOWN.md): every one knocked down that is drawn, up to
 * `RAGDOLL_CAP`, the nearest first. A ragdoll is made as the knockdown starts, thrown as the soldier
 * was, follows where the server has the soldier, and lets go when they are up, or stays as the body if
 * they died meanwhile, until they respawn or are blown apart. The physics is `ClientSim`'s
 * (`crates/sim/src/ragdoll.rs`).
 */

import type { SimMotor, Vec3 } from './motor'

/** The most ragdolls at once; any more knocked down soldiers just lie down. */
export const RAGDOLL_CAP = 8
/** How fast a thrown soldier turns head over heels, rad/s per m/s it was thrown at. */
const SPIN_PER_SPEED = 0.45

export type RagdollPhysics = Pick<SimMotor, 'spawnRagdoll' | 'setRagdollTarget' | 'stepRagdolls' | 'ragdollPose' | 'removeRagdoll'>

/** What a ragdoll poses: a soldier's `Character`. */
export interface RagdollBody {
  /** Drawn and in one piece. */
  readonly intact: boolean
  ragdollSegments(): { segments: Float32Array; side: Vec3 }
  startRagdoll(pose: ArrayLike<number>): void
  driveRagdoll(pose: ArrayLike<number>): void
  endRagdoll(rise?: boolean): void
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
  /** How they are moving: the throw, as the knockdown starts. */
  velocity: Vec3
  /** Limbs lost (`LIMB_*`): no part for them. */
  lost: number
}

export class Ragdolls {
  private readonly active = new Set<number>()

  constructor(
    private readonly physics: RagdollPhysics,
    readonly cap = RAGDOLL_CAP,
  ) {}

  /** How many ragdolls there are. */
  get count() {
    return this.active.size
  }

  has(key: number) {
    return this.active.has(key)
  }

  /** Brings the ragdolls into line with `soldiers` (all those drawn), runs them on by `dt` and poses them. */
  update(soldiers: readonly Knockable[], camera: Vec3, dt: number) {
    const seen = new Set<number>()
    const newcomers: Knockable[] = []
    for (const soldier of soldiers) {
      const { key, body } = soldier
      seen.add(key)
      const on = this.active.has(key)
      // A body stays a ragdoll if it dies in one; a soldier is only made one while alive and down.
      const wants = body.intact && (soldier.down || (soldier.dead && on))
      if (on && !wants) this.drop(key, body, !soldier.dead)
      else if (on) this.physics.setRagdollTarget(key, soldier.position)
      else if (wants && !soldier.dead) newcomers.push(soldier)
    }
    for (const key of [...this.active]) if (!seen.has(key)) this.drop(key, null, false)
    const away = (at: Vec3) => (at[0] - camera[0]) ** 2 + (at[2] - camera[2]) ** 2
    newcomers.sort((a, b) => away(a.position) - away(b.position))
    for (const soldier of newcomers) {
      if (this.active.size >= this.cap) break
      this.spawn(soldier)
    }
    if (this.active.size === 0) return
    this.physics.stepRagdolls(dt)
    for (const { key, body } of soldiers) {
      if (!this.active.has(key)) continue
      const pose = this.physics.ragdollPose(key)
      if (pose) body.driveRagdoll(pose)
    }
  }

  /** Lets every ragdoll go: the world is going away. */
  clear() {
    for (const key of this.active) this.physics.removeRagdoll(key)
    this.active.clear()
  }

  private spawn({ key, body, position, velocity, lost }: Knockable) {
    const { segments, side } = body.ragdollSegments()
    // Head over heels the way it was thrown: about the level axis across the throw.
    const [vx, , vz] = velocity
    const speed = Math.hypot(vx, vz)
    const spin: Vec3 = speed > 1e-3 ? [(vz / speed) * speed * SPIN_PER_SPEED, 0, (-vx / speed) * speed * SPIN_PER_SPEED] : [0, 0, 0]
    if (!this.physics.spawnRagdoll(key, segments, side, velocity, spin, lost)) return
    const pose = this.physics.ragdollPose(key)
    if (!pose) return
    this.physics.setRagdollTarget(key, position)
    body.startRagdoll(pose)
    this.active.add(key)
  }

  private drop(key: number, body: RagdollBody | null, rise: boolean) {
    this.physics.removeRagdoll(key)
    this.active.delete(key)
    body?.endRagdoll(rise)
  }
}
