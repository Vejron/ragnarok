/** What the minimap marks besides the player: allies where they are, and where others just fired. */

/** One mark, in world metres (x east, z south). */
export interface Blip {
  x: number
  z: number
  /** CSS colour. */
  color: string
  /** An ally, shown while in view, or someone who fired. */
  kind: 'ally' | 'shot'
  /** 1 when new, falling to 0 as it goes. */
  strength: number
}

/** How long a shot stays on the map, ms, and how many are kept. */
export const SHOT_MS = 4000
export const MAX_SHOTS = 24
/** A shot fired by nobody known (a shell or a rocket) this near the player is taken to be theirs. */
export const OWN_SHOT_RADIUS = 10
/** Shots this close together, by the same shooter, are one mark. */
const SAME_MARK = 5

/** Who a mark is for: the colours and sides the profiles say. */
export interface Sides {
  colorOf(id: number): string
  isBot(id: number): boolean
  allies(a: number, b: number): boolean
}

export const BOT_COLOR = '#e63946'
export const UNKNOWN_COLOR = '#ff9f1c'
export const ALLY_COLOR = '#2ec4b6'

export class Blips {
  private shots: { shooter: number; x: number; z: number; at: number }[] = []

  clear() {
    this.shots = []
  }

  /** `shooter` fired from `(x, z)` at `now`; 0 when nobody is known, as for a shell or a rocket. */
  fired(shooter: number, x: number, z: number, now: number) {
    const near = this.shots.find((shot) => shot.shooter === shooter && Math.hypot(shot.x - x, shot.z - z) < SAME_MARK)
    if (near) Object.assign(near, { x, z, at: now })
    else this.shots.push({ shooter, x, z, at: now })
    if (this.shots.length > MAX_SHOTS) this.shots.shift()
  }

  /**
   * The marks at `now` for player `localId` at `(x, z)`: `allies` where they stand, and every shot
   * not by them or their side.
   */
  view(now: number, localId: number | null, x: number, z: number, allies: readonly { id: number; x: number; z: number }[], sides: Sides): Blip[] {
    this.shots = this.shots.filter((shot) => now - shot.at < SHOT_MS)
    const blips: Blip[] = allies.map((ally) => ({ x: ally.x, z: ally.z, color: ALLY_COLOR, kind: 'ally', strength: 1 }))
    for (const shot of this.shots) {
      if (shot.shooter === localId) continue
      if (shot.shooter === 0 && Math.hypot(shot.x - x, shot.z - z) < OWN_SHOT_RADIUS) continue
      if (localId !== null && shot.shooter !== 0 && sides.allies(localId, shot.shooter)) continue
      const color = shot.shooter === 0 ? UNKNOWN_COLOR : sides.isBot(shot.shooter) ? BOT_COLOR : sides.colorOf(shot.shooter)
      blips.push({ x: shot.x, z: shot.z, color, kind: 'shot', strength: 1 - (now - shot.at) / SHOT_MS })
    }
    return blips
  }
}
