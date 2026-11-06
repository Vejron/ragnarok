import { BUTTON_ALT, BUTTON_AMMO, BUTTON_CHUTE, BUTTON_CROUCH, BUTTON_FIRE, BUTTON_JUMP, BUTTON_PRONE, BUTTON_RELOAD, BUTTON_RUN, BUTTON_SKIS, BUTTON_USE } from './sim/consts'
import type { HudStats } from './hud'

type Seat = HudStats['seat']

/**
 * One of the touch screen's round buttons. `hold` is held like a key while the finger stays; `tap` is
 * pressed once; `toggle` is pressed once and lit while `on` says so. `col` and `row` count from the
 * bottom-right corner of the grid the buttons sit on.
 */
export interface TouchButton {
  id: string
  label: string
  mode: 'hold' | 'tap' | 'toggle'
  /** The `BUTTON_*` it presses; 0 for one the component handles itself (holstering). */
  mask: number
  col: number
  row: number
  tone?: 'fire' | 'reload'
  big?: boolean
}

export interface TouchContext {
  canSwitch: boolean
  winter: boolean
  /** A gun with two kinds of round is in hand. */
  ammo: boolean
}

/** Seats that aim down the middle of the screen with a sight or over the shoulder, as right mouse does. */
const AIMS = new Set<Seat>(['gunner', 'roofgunner', 'strv', 'walker', 'tupp', 'passenger'])

const jumpLabel = (seat: Seat) =>
  seat === 'driver' || seat === 'sled' ? 'BRAKE' : seat === 'helm' ? 'ANCHOR' : seat === 'roadster' ? 'HOP' : seat === 'hen' || seat === 'tupp' ? 'FLAP' : seat === 'aviator' ? 'FASTER' : seat === 'pilot' || seat === 'drone' ? 'UP' : 'JUMP'
const fireLabel = (seat: Seat) => (seat === 'drone' || seat === 'loiter' ? 'BOOM' : seat === 'hen' ? 'EGG' : seat === 'tupp' ? 'LASER' : 'FIRE')

/** The buttons for `seat` (null on foot), each in a cell of its own. */
export function touchButtons(seat: Seat, { canSwitch, winter, ammo }: TouchContext): TouchButton[] {
  const buttons: TouchButton[] = [
    { id: 'fire', label: fireLabel(seat), mode: 'hold', mask: BUTTON_FIRE, col: 0, row: 0, tone: 'fire', big: true },
    { id: 'jump', label: jumpLabel(seat), mode: 'hold', mask: BUTTON_JUMP, col: 0, row: 1 },
    { id: 'use', label: seat === 'drone' || seat === 'loiter' ? 'RECALL' : 'USE', mode: 'tap', mask: BUTTON_USE, col: 0, row: 2 },
  ]
  if (seat === null) {
    buttons.push(
      { id: 'aim', label: 'AIM', mode: 'hold', mask: BUTTON_ALT, col: 1, row: 0 },
      { id: 'reload', label: 'RELOAD', mode: 'tap', mask: BUTTON_RELOAD, col: 2, row: 0, tone: 'reload' },
      { id: 'crouch', label: 'CROUCH', mode: 'hold', mask: BUTTON_CROUCH, col: 1, row: 1 },
      // Lying down is a toggle, as Z is.
      { id: 'prone', label: 'PRONE', mode: 'toggle', mask: BUTTON_PRONE, col: 2, row: 1 },
      { id: 'holster', label: 'HANDS', mode: 'toggle', mask: 0, col: 1, row: 2 },
      { id: 'chute', label: 'CHUTE', mode: 'tap', mask: BUTTON_CHUTE, col: 2, row: 2 },
    )
    if (winter) buttons.push({ id: 'skis', label: 'SKIS', mode: 'tap', mask: BUTTON_SKIS, col: 3, row: 2 })
    return buttons
  }
  if (AIMS.has(seat)) buttons.push({ id: 'aim', label: 'AIM', mode: 'hold', mask: BUTTON_ALT, col: 1, row: 0 })
  // A loitering drone's camera swap is aim, as right-click is.
  if (seat === 'loiter') buttons.push({ id: 'cam', label: 'CAM', mode: 'hold', mask: BUTTON_ALT, col: 1, row: 0 })
  if (seat === 'passenger') buttons.push({ id: 'reload', label: canSwitch ? 'RELOAD / SEAT' : 'RELOAD', mode: 'tap', mask: BUTTON_RELOAD, col: 2, row: 0, tone: 'reload' })
  else if (canSwitch) buttons.push({ id: 'seat', label: 'SEAT', mode: 'tap', mask: BUTTON_RELOAD, col: 2, row: 0, tone: 'reload' })
  if (seat === 'pilot' || seat === 'drone' || seat === 'aviator' || seat === 'roadster' || seat === 'sled') {
    buttons.push({ id: 'sink', label: seat === 'aviator' ? 'SLOWER' : seat === 'roadster' || seat === 'sled' ? 'BOOST' : 'DOWN', mode: 'hold', mask: BUTTON_RUN, col: 1, row: 1 })
  }
  if (seat === 'aviator' || seat === 'roadster') buttons.push({ id: 'crouch', label: seat === 'roadster' ? 'BRAKE' : 'BOMB', mode: 'hold', mask: BUTTON_CROUCH, col: 2, row: 1 })
  if (ammo) buttons.push({ id: 'ammo', label: 'AMMO', mode: 'tap', mask: BUTTON_AMMO, col: 1, row: 2 })
  return buttons
}
