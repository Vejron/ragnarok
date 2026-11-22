/**
 * A helicopter pilot's collective lever. Jump raises it and run lowers it, each at its kind's
 * `leverRate` of its travel a second, and it stays where it is let go, so a pilot sets a climb, a hover or a descent
 * and flies the stick and the look from there. The input frame carries it to the server as `collective`,
 * 0..=255, which the prediction reads back the same way.
 */

import { BUTTON_JUMP, BUTTON_RUN, TICK_DT } from './consts'
import type { Rotor } from './vehicles'

/** Rotor speed (share of governed) above which a helicopter whose pilot's seat is taken counts as flying. */
const FLYING_ROTOR = 0.5
/** Height over the ground, metres, above which a helicopter counts as flying. */
const FLYING_HEIGHT = 2

export class CollectiveLever {
  /** Where the lever is, 0 (down) ..= 1 (full up). */
  value = 0
  /** How fast it moves, share of its travel per second: that of the helicopter it was last taken in. */
  private rate = 0

  /**
   * The pilot's seat of a helicopter that flies as `rotor` is taken: on the ground the lever starts down; in the air (its rotor turning and
   * `height` metres up) it starts where it hovers, so the helicopter does not drop out from under them.
   */
  take(rotor: Rotor, speed: number, height: number) {
    this.rate = rotor.leverRate
    this.value = speed > FLYING_ROTOR && height > FLYING_HEIGHT ? rotor.hoverLever : 0
  }

  /** One tick of the pilot's `buttons`; returns the lever as the input frame carries it. */
  step(buttons: number): number {
    const raise = (buttons & BUTTON_JUMP) !== 0 ? 1 : 0
    const lower = (buttons & BUTTON_RUN) !== 0 ? 1 : 0
    this.value = Math.min(1, Math.max(0, this.value + (raise - lower) * this.rate * TICK_DT))
    return this.byte
  }

  /** The lever on the wire, 0..=255. */
  get byte(): number {
    return Math.round(this.value * 255)
  }
}
