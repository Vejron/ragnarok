/**
 * A Titan's fists under mouse and keyboard: what its pilot's look and buttons ask of them each tick,
 * as the `ClientHands` the server takes (VEHICLES-XIV.md). A VR pilot's hands send the same message (`xr/hands.ts`).
 *
 * Both fists are held up in a guard towards what the middle of the screen is on. A click throws that
 * side's fist at it, closed: a punch. Shift reaches both hands out to it, open, one either side of it,
 * and a click then closes that hand on what is there; let go of Shift and it is brought back in, let
 * go of the button and it opens: what it held is put down, or thrown if the arm was swinging.
 */

import { Quaternion, Vector3 } from '@ragnarok/engine'
import { HANDS_GRIP_LEFT, HANDS_GRIP_RIGHT, HANDS_REACH_LEFT, HANDS_REACH_RIGHT, toCm, type ClientHands } from '@/net/codec'
import { BUTTON_ALT, BUTTON_FIRE, BUTTON_RUN, TITAN_FIST_RADIUS, TITAN_REACH, TITAN_SHOULDER } from '@/sim/consts'

/** How far from its shoulder a fist is held in the guard, metres. */
const GUARD = 11
/** How far either side of the point they reach for the open hands are held, metres: clear of whoever stands there. */
const SPREAD = TITAN_FIST_RADIUS + 0.7
/** How far over the point it reaches for an open hand's middle is, metres: resting on the ground, not in it. */
const HOVER = 1.2
/** The least of an arm's direction that is forwards: a fist asked for behind the Titan stays at its side. */
const FORWARD = 0.25
/** The furthest a fist is asked for from its shoulder, metres: a little past its reach, which the server keeps it to. */
const FAR = TITAN_REACH + 2

type Mode = 'guard' | 'punch' | 'grip'

/** The buttons that close the left and the right fist. */
const TRIGGERS = [BUTTON_FIRE, BUTTON_ALT] as const

export class TitanHands {
  private readonly modes: [Mode, Mode] = ['guard', 'guard']
  private held = 0

  /** Back to the guard, for a pilot who has just climbed in. */
  reset() {
    this.modes[0] = this.modes[1] = 'guard'
    this.held = 0
  }

  /**
   * One tick. `aim` is the point in the world the middle of the screen is on, `position` and `rotation`
   * the Titan's hull as drawn, and `buttons` the pilot's.
   */
  step(buttons: number, aim: Vector3, position: readonly [number, number, number], rotation: readonly [number, number, number, number]): ClientHands {
    const reaching = (buttons & BUTTON_RUN) !== 0
    const local = aim.clone().sub(new Vector3(...position)).applyQuaternion(new Quaternion(...rotation).invert())
    const out: [Vector3, Vector3] = [new Vector3(), new Vector3()]
    let flags = HANDS_REACH_LEFT | HANDS_REACH_RIGHT
    for (let hand = 0; hand < 2; hand++) {
      const side = hand === 0 ? -1 : 1
      const down = (buttons & TRIGGERS[hand]!) !== 0
      const was = (this.held & TRIGGERS[hand]!) !== 0
      // A click with the hands reached out closes on what is there; without, it throws a punch.
      if (down && !was) this.modes[hand] = reaching ? 'grip' : 'punch'
      else if (!down) this.modes[hand] = 'guard'
      const mode = this.modes[hand]!
      const shoulder = new Vector3(side * TITAN_SHOULDER[0], TITAN_SHOULDER[1], TITAN_SHOULDER[2])
      // A punch goes to the point itself; an open or a gripping hand to its own side of it.
      const point = mode === 'punch' ? local.clone() : new Vector3(local.x + side * SPREAD, local.y + HOVER, local.z)
      const arm = point.sub(shoulder)
      const length = arm.length() || 1
      arm.divideScalar(length)
      if (arm.z > -FORWARD) arm.setZ(-FORWARD).normalize()
      const extended = mode === 'punch' || reaching
      out[hand]!.copy(shoulder).addScaledVector(arm, extended ? Math.min(length, FAR) : Math.min(length, GUARD))
      if (mode !== 'guard') flags |= hand === 0 ? HANDS_GRIP_LEFT : HANDS_GRIP_RIGHT
    }
    this.held = buttons
    const [left, right] = out
    return { lx: toCm(left.x), ly: toCm(left.y), lz: toCm(left.z), rx: toCm(right.x), ry: toCm(right.y), rz: toCm(right.z), flags }
  }
}
