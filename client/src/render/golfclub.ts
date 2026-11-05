import { Group, Mesh, boxGeometry, cylinderGeometry } from '@ragnarok/engine'
import { toonMaterial } from './toon'
import { ITEM_PUTTER } from '@/sim/items'

/**
 * Where the club lies in the right hand bone's frame, as `golf_swing` in
 * `tools/character/build_character.py` lays the hand along it: the shaft runs +Y from the butt above
 * the hand to the head, the toe is +X and the face looks +Z.
 */
const BUTT = -0.08
const HEAD = 0.8

let parts: ReturnType<typeof make> | null = null

function make() {
  return {
    shaft: cylinderGeometry(0.011, 0.008, HEAD - BUTT, 6),
    grip: cylinderGeometry(0.016, 0.014, 0.25, 6),
    driver: boxGeometry(0.11, 0.06, 0.09),
    putter: boxGeometry(0.11, 0.03, 0.03),
    steel: toonMaterial({ color: 0xc8ccd0 }),
    black: toonMaterial({ color: 0x1c1c1e }),
  }
}

/** A golf club (`ITEM_DRIVER`, `ITEM_PUTTER`), to hang on the right hand bone as it is. */
export function buildClub(item: number): Group {
  parts ??= make()
  const group = new Group()
  const shaft = new Mesh(parts.shaft, parts.steel)
  shaft.position.y = (BUTT + HEAD) / 2
  group.add(shaft)
  const grip = new Mesh(parts.grip, parts.black)
  grip.position.y = BUTT + 0.12
  group.add(grip)
  const head = item === ITEM_PUTTER ? new Mesh(parts.putter, parts.steel) : new Mesh(parts.driver, parts.black)
  head.position.set(0.04, HEAD, item === ITEM_PUTTER ? 0 : -0.02)
  group.add(head)
  return group
}

/** Phases of the `golf_swing` clip, 0..1: the top of the backswing, impact, the finish, and the finish held until. Mirrors `GOLF_*` in build_character.py. */
const TOP = 0.4
const IMPACT = 0.56
const FINISH = 0.75
const HOLD = 0.86
/** Seconds: a backswing not seen being charged (another player's), the downswing to the finish, the finish held, and back to the address. */
const BACK_TIME = 0.3
const DOWN_TIME = 0.4
const HOLD_TIME = 0.6
const RETURN_TIME = 0.8

/** The clip's phase with the backswing drawn `charge` (0..1) of the way to the top. */
export const backswingPhase = (charge: number) => TOP * Math.min(1, Math.max(0, charge))

/** How far down from the top the club starts while the meter's marker runs back, of the way to impact. */
const TRANSITION = 0.3

/** The clip's phase with the meter's marker `run` (0..1) of the way back to the line: the club starts down from the top. */
export const downswingPhase = (run: number) => TOP + (IMPACT - TOP) * TRANSITION * Math.min(1, Math.max(0, run))

/** Seconds of the downswing from phase `from` to the finish: all of it from the top. */
const downTime = (from: number) => (DOWN_TIME * (FINISH - from)) / (FINISH - TOP)

/** How long a swing takes, in seconds, from being let go to back at the address. */
export const swingLength = (from: number) => (from > 0 ? downTime(from) : BACK_TIME + DOWN_TIME) + HOLD_TIME + RETURN_TIME

const smooth = (x: number) => {
  const t = Math.min(1, Math.max(0, x))
  return t * t * (3 - 2 * t)
}

/** The clip's phase `time` seconds into a swing let go at phase `from`; none drawn means it is drawn first, quickly. */
export function swingPhase(time: number, from: number): number {
  let t = time
  let top = from
  if (from <= 0) {
    if (t < BACK_TIME) return TOP * smooth(t / BACK_TIME)
    t -= BACK_TIME
    top = TOP
  }
  const down = downTime(top)
  if (t < down) return top + (FINISH - top) * smooth(t / down)
  t -= down
  if (t < HOLD_TIME) return FINISH + (HOLD - FINISH) * (t / HOLD_TIME)
  t -= HOLD_TIME
  return HOLD + (1 - HOLD) * smooth(t / RETURN_TIME)
}
