import type { Node } from '@ragnarok/engine'
import { onGlass, splashed, type HullMarks } from './hullmarks'

/** How far blood out of the far side of a wound is thrown onto the inside of a car, metres: to an estate's back glass. */
const THROWN = 3
/** How far the spray thrown back the way the round came reaches, metres: the glass it came through. */
const BACK = 1.2
/** How far past the wound the blood out of the far side starts, metres, as `Gore` sprays it. */
const EXIT = 0.15
/** How far each splat out of the far side climbs or falls for each metre it goes. */
const RISE = [0, 0.12, -0.06]
/** How much wider the splats from a head are than a body's. */
const HEAD_SCALE = 1.3

type V = { x: number; y: number; z: number }

const spread = (n: number) => (Math.random() * 2 - 1) * n

/**
 * Someone sitting in vehicle `id` (drawn by `root`) was shot at world point `at` from bearing `from`
 * (0 faces -Z) for `amount`, in the head if `head`. The blood lands on the inside of the vehicle: two or three
 * splats out of the far side of the wound onto the windows behind them, and two smaller ones
 * thrown back onto the glass the round came through. Only glass takes it: blood on a seat or the roof's
 * lining would show through the thin body on its outside.
 */
export function spatterCar(marks: HullMarks, id: number, root: Node, at: V, from: number, amount: number, head: boolean) {
  const out = { x: Math.sin(from), y: 0, z: Math.cos(from) }
  const heavy = Math.min(1, amount / 40)
  const size = (head ? HEAD_SCALE : 1) * (0.7 + 0.6 * heavy)
  const exit = { x: at.x + out.x * EXIT, y: at.y, z: at.z + out.z * EXIT }
  const splats = head || heavy > 0.5 ? 3 : 2
  for (let i = 0; i < splats; i++) {
    // Straight on, a little up and a little down, towards the windows behind.
    const dir = unit(out.x + spread(0.3), spread(0.1) + RISE[i]!, out.z + spread(0.3))
    const struck = glassAt(root, exit, dir, THROWN)
    if (struck) marks.mark(id, root, struck, 'blood', size * (i === 0 ? 1 : 0.6))
  }
  for (let i = 0; i < 2; i++) {
    const back = glassAt(root, at, unit(-out.x + spread(0.25), spread(0.15), -out.z + spread(0.25)), BACK)
    if (back) marks.mark(id, root, back, 'blood', size * (i === 0 ? 0.7 : 0.4))
  }
}

/** The glass the blood thrown from `from` along `dir` lands on within `max` metres; null if it meets something else first, or nothing. */
function glassAt(root: Node, from: V, dir: V, max: number) {
  const struck = splashed(root, from, dir, max)
  return struck && onGlass(struck) ? struck : null
}

function unit(x: number, y: number, z: number) {
  const length = Math.hypot(x, y, z) || 1
  return { x: x / length, y: y / length, z: z / length }
}
