/**
 * A Titan's fists under a VR pilot's own hands (VR.md): each controller, measured from where the pilot's
 * shoulder is, moves that fist the same way from the Titan's shoulder, as many times further as its arm
 * is longer than theirs. It makes the same `ClientHands` the mouse and keyboard do (`titanhands.ts`).
 */

import { Quaternion, Vector3 } from '@ragnarok/engine'
import { HANDS_GRIP_LEFT, HANDS_GRIP_RIGHT, HANDS_REACH_LEFT, HANDS_REACH_RIGHT, toCm, type ClientHands } from '@/net/codec'
import { TITAN_REACH, TITAN_SHOULDER } from '@/sim/consts'

/** A pilot's arm, shoulder to the middle of the fist round a controller, metres: a little short, so that the Titan's reaches its full length before theirs does. */
export const PILOT_ARM = 0.6
/** Where a pilot's shoulders are from their eyes, metres: this far to each side and this far down. */
export const PILOT_SHOULDER_OUT = 0.19
export const PILOT_SHOULDER_DOWN = 0.22

/** One of the pilot's hands this frame, in the headset's space (+X right, +Y up, -Z ahead at the start). */
export interface PilotHand {
  /** Where the controller is, or null while it is not tracked. */
  at: readonly [number, number, number] | null
  /** Its grip is squeezed. */
  closed: boolean
}

/**
 * What the pilot's hands (left, right) ask of the Titan's fists. `head` is the pilot's eyes in the
 * headset's space, `rigYaw` the heading that space is drawn at in the world (soldier convention), and
 * `hull` the Titan's rotation: the fists are asked for in its frame, which leans as it walks while the
 * cockpit does not.
 */
export function fistsFromHands(head: readonly [number, number, number], hands: readonly [PilotHand, PilotHand], rigYaw: number, hull: readonly [number, number, number, number]): ClientHands {
  const toWorld = new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), rigYaw)
  const toHull = new Quaternion(...hull).invert()
  const scale = TITAN_REACH / PILOT_ARM
  let flags = 0
  const out = hands.map((hand, index) => {
    if (!hand.at) return new Vector3()
    const side = index === 0 ? -1 : 1
    flags |= index === 0 ? HANDS_REACH_LEFT : HANDS_REACH_RIGHT
    if (hand.closed) flags |= index === 0 ? HANDS_GRIP_LEFT : HANDS_GRIP_RIGHT
    const reach = new Vector3(hand.at[0] - (head[0] + side * PILOT_SHOULDER_OUT), hand.at[1] - (head[1] - PILOT_SHOULDER_DOWN), hand.at[2] - head[2])
    reach.multiplyScalar(scale).applyQuaternion(toWorld).applyQuaternion(toHull)
    return reach.add(new Vector3(side * TITAN_SHOULDER[0], TITAN_SHOULDER[1], TITAN_SHOULDER[2]))
  })
  const [left, right] = out as [Vector3, Vector3]
  return { lx: toCm(left.x), ly: toCm(left.y), lz: toCm(left.z), rx: toCm(right.x), ry: toCm(right.y), rz: toCm(right.z), flags }
}

/**
 * A headset's projection (depth 0 at the near plane, 1 at the far) as the engine's, which has them the
 * other way round: the depth row becomes the w row less itself.
 */
export function reversedDepth(projection: ArrayLike<number>): Float32Array {
  const out = Float32Array.from(projection)
  for (let column = 0; column < 4; column++) out[column * 4 + 2] = projection[column * 4 + 3]! - projection[column * 4 + 2]!
  return out
}
