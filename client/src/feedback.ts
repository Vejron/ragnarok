import { wrapAngle } from './net/codec'

/** `Hit` roles and kinds, as in `crates/shared/src/protocol.rs`. */
export const HIT_DEALT = 0
export const HIT_TAKEN = 1
export const HIT_KILL = 2

/** What a `Hit` message says, with the bearing already in radians (null: no direction). */
export interface HitMessage {
  role: number
  kind: number
  amount: number
  weapon: number
  bearing: number | null
}

/** Kinds, as in `Hit`. */
export const HIT_VEHICLE = 1
export const HIT_ROOSTER = 3
/** The Orange Plague. */
export const HIT_PLAGUE = 4

/** A hit marker lasts this long, ms; the one that finishes someone lasts longer. */
export const MARKER_MS = 170
export const KILL_MARKER_MS = 480
/** Hits dealt this close together add up in the number shown. */
const NUMBER_JOIN_MS = 450
export const VIGNETTE_MS = 650
export const ARC_MS = 1300
const MAX_ARCS = 6

export interface MarkerView {
  /** 0 as it appears, 1 as it ends. */
  progress: number
  kill: boolean
  /** Damage dealt in the burst of hits this marker shows. */
  amount: number
}

export interface ArcView {
  /** Clockwise from straight up, radians: where the harm came from on the screen. */
  angle: number
  alpha: number
}

export interface HitView {
  marker: MarkerView | null
  /** Strength of the red screen-edge flash, 0..1. */
  vignette: number
  arcs: ArcView[]
}

/**
 * What the player sees of the hits they deal and take: a marker at the crosshair, a flash at the
 * screen edges and arcs towards where harm came from. Fed `Hit` messages, read each frame.
 */
export class HitFeedback {
  private dealtAt = -Infinity
  private dealtKill = false
  private dealtAmount = 0
  private hurtAt = -Infinity
  private hurtStrength = 0
  private arcs: { at: number; bearing: number; strength: number }[] = []

  receive(hit: HitMessage, now: number) {
    if (hit.role === HIT_DEALT) {
      const joined = now - this.dealtAt < NUMBER_JOIN_MS
      this.dealtAmount = (joined ? this.dealtAmount : 0) + hit.amount
      this.dealtKill = (joined && this.dealtKill) || hit.kind === HIT_KILL
      this.dealtAt = now
    } else if (hit.role === HIT_TAKEN) {
      const strength = Math.min(1, 0.3 + hit.amount / 60)
      // A new hit never weakens a flash still fading.
      this.hurtStrength = Math.max(strength, this.vignette(now))
      this.hurtAt = now
      if (hit.bearing !== null) {
        this.arcs.push({ at: now, bearing: hit.bearing, strength })
        if (this.arcs.length > MAX_ARCS) this.arcs.shift()
      }
    }
  }

  private vignette(now: number) {
    const left = 1 - (now - this.hurtAt) / VIGNETTE_MS
    return left > 0 ? this.hurtStrength * left * left : 0
  }

  /** Forgets everything, e.g. on respawning or disconnecting. */
  clear() {
    this.dealtAt = this.hurtAt = -Infinity
    this.arcs = []
  }

  /** What to draw at `now`, with the camera looking along `yaw` (0 faces -Z, + turns left). */
  view(now: number, yaw: number): HitView {
    const length = this.dealtKill ? KILL_MARKER_MS : MARKER_MS
    const progress = (now - this.dealtAt) / length
    this.arcs = this.arcs.filter((arc) => now - arc.at < ARC_MS)
    return {
      marker: progress >= 0 && progress < 1 ? { progress, kill: this.dealtKill, amount: this.dealtAmount } : null,
      vignette: this.vignette(now),
      arcs: this.arcs.map((arc) => ({ angle: wrapAngle(yaw - arc.bearing), alpha: arc.strength * (1 - (now - arc.at) / ARC_MS) })),
    }
  }
}
