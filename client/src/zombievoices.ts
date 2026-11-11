import type { Vec3 } from '@/vehicleaudio'

/** Seconds between one zombie's groans, shortest and longest. */
const GROAN_GAP: [number, number] = [4, 11]
/** How much one zombie's voice is pitched up or down from the next, at most: a share of the note. */
const PITCH_SPREAD = 0.25

/** What the zombies' voices are played on. */
export interface ZombieSounds {
  groan(at: Vec3, pitch: number): void
  claw(at: Vec3): void
}

/**
 * The zombies' voices (ZOMBIES.md): each groans every few seconds in a voice of its own, and growls
 * as its claws swing, each time the server starts saying it claws.
 */
export class ZombieVoices {
  /** Seconds until each one groans next. */
  private readonly next = new Map<number, number>()
  private readonly clawing = new Set<number>()
  /** The ids heard from this frame, to forget the rest. */
  private readonly seen = new Set<number>()

  constructor(private readonly sounds: ZombieSounds, private readonly random: () => number = Math.random) {}

  /** Zombie `id` at `(x, y, z)` this frame, `clawing` or not, `dt` seconds after the last. */
  hear(id: number, x: number, y: number, z: number, clawing: boolean, dt: number) {
    this.seen.add(id)
    const at: Vec3 = [x, y, z]
    // The first groan comes at a random point in the gap, so a horde that rises together does not groan together.
    const left = (this.next.get(id) ?? this.gap() * this.random()) - dt
    if (left <= 0) {
      this.sounds.groan(at, this.pitch(id))
      this.next.set(id, this.gap())
    } else this.next.set(id, left)
    if (clawing && !this.clawing.has(id)) this.sounds.claw(at)
    if (clawing) this.clawing.add(id)
    else this.clawing.delete(id)
  }

  /** Ends a frame: zombies not heard from in it are forgotten. */
  sweep() {
    for (const id of this.next.keys()) {
      if (this.seen.has(id)) continue
      this.next.delete(id)
      this.clawing.delete(id)
    }
    this.seen.clear()
  }

  private gap() {
    return GROAN_GAP[0] + (GROAN_GAP[1] - GROAN_GAP[0]) * this.random()
  }

  /** Each zombie's own voice, from its id. */
  private pitch(id: number) {
    const spread = (((id * 2654435761) >>> 0) % 1000) / 1000
    return 1 - PITCH_SPREAD + 2 * PITCH_SPREAD * spread
  }
}
