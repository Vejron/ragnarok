import {
  BUTTON_CROUCH, BUTTON_JUMP, BUTTON_SLIDE, BUTTON_RUN, CROUCH_SPEED, GRAVITY, JUMP_SPEED, KNOCK_KEEP_AIR, KNOCK_KEEP_GROUND, KNOCK_MIN_SPEED, MAX_FALL_SPEED, RUN_SPEED,
  SLIDE_KEEP, SLIDE_RETRIGGER, SLIDE_SPEED, WALK_SPEED,
} from './consts'
import { fromAxis, fromMrad, type InputFrame } from '@/net/codec'

/** Mirrors `crates/shared/src/movement.rs`: the movement rules around the character controller. */

/** The server simulates in f32; rounding at the same points keeps prediction from drifting. */
export const f32 = Math.fround

export function wishVelocity(input: InputFrame): [number, number] {
  const strafe = fromAxis(input.strafe)
  const forward = fromAxis(input.forward)
  const yaw = fromMrad(input.yaw)
  const sin = Math.sin(yaw)
  const cos = Math.cos(yaw)
  // Yaw 0 faces -Z. forward = (-sin, -cos), right = (cos, -sin).
  let x = -sin * forward + cos * strafe
  let z = -cos * forward - sin * strafe
  const length = Math.hypot(x, z)
  if (length > 1) {
    x /= length
    z /= length
  }
  const speed = input.buttons & BUTTON_CROUCH ? CROUCH_SPEED : input.buttons & BUTTON_RUN ? RUN_SPEED : WALK_SPEED
  return [f32(x * speed), f32(z * speed)]
}

export function desiredTranslation(
  vy: number,
  grounded: boolean,
  knock: readonly [number, number],
  input: InputFrame,
  dt: number,
): { vy: number; desired: [number, number, number] } {
  let next = vy
  if (grounded && input.buttons & BUTTON_JUMP) next = JUMP_SPEED
  next = f32(Math.max(next - GRAVITY * dt, -MAX_FALL_SPEED))
  const [vx, vz] = wishVelocity(input)
  return { vy: next, desired: [f32(f32(vx + knock[0]) * dt), f32(next * dt), f32(f32(vz + knock[1]) * dt)] }
}

/** Launches a slide on a run-crouch press. Mirrors `begin_slide`. */
export function beginSlide(knock: readonly [number, number], grounded: boolean, input: InputFrame): readonly [number, number] {
  const wanted = BUTTON_SLIDE | BUTTON_CROUCH
  if (!grounded || (input.buttons & wanted) !== wanted || Math.hypot(knock[0], knock[1]) >= SLIDE_RETRIGGER) return knock
  const [x, z] = wishVelocity(input)
  const length = Math.hypot(x, z)
  if (length < 1e-3) return knock
  return [f32((x / length) * SLIDE_SPEED), f32((z / length) * SLIDE_SPEED)]
}

/** Knockback after one tick. Mirrors `decay_knock`. */
export function decayKnock(knock: readonly [number, number], grounded: boolean, crouching = false): [number, number] {
  const keep = f32(!grounded ? KNOCK_KEEP_AIR : crouching ? SLIDE_KEEP : KNOCK_KEEP_GROUND)
  const x = f32(knock[0] * keep)
  const z = f32(knock[1] * keep)
  return Math.hypot(x, z) < KNOCK_MIN_SPEED ? [0, 0] : [x, z]
}

export function settleVertical(vy: number, desiredY: number, movedY: number, grounded: boolean): number {
  const landed = grounded && vy <= 0
  const bumpedCeiling = desiredY > 0 && movedY < desiredY * 0.5
  return landed || bumpedCeiling ? 0 : vy
}
