/**
 * The key hint for what E (or G) does where the local player stands, judged as the server will judge
 * it: the parachute first while falling, then an item within reach, then the nearest vehicle with a
 * free seat, which the server boards into its lowest free seat.
 */

import type { Lying } from '@/entities/presenters/pickups'
import type { Placed } from '@/entities/presenters/vehicles'
import { seatView } from '@/seats'
import { ENTER_RADIUS, WALKER_LEG_LENGTH } from '@/sim/consts'
import { itemDef, PICKUP_RADIUS } from '@/sim/items'
import { isLegged } from '@/sim/vehicles'

/** Falling this high (metres) over the ground, a hint offers the parachute. */
const CHUTE_PROMPT_HEIGHT = 4

export interface Where {
  /** The local player's position, metres: their capsule centre. */
  position: readonly number[]
  /** The ground's height under them. */
  ground: number
  alive: boolean
  grounded: boolean
  /** Hanging under an open parachute. */
  chute: boolean
  /** The seat they are in, if any. */
  seat: { kind: number; index: number } | null
}

/** The nearest item on the ground within reach of `position`, as the server will judge it. */
export function itemInReach(lying: readonly Lying[], position: readonly number[]): Lying | null {
  let best: Lying | null = null
  let bestDistance = PICKUP_RADIUS
  for (const pickup of lying) {
    if (Math.abs(pickup.y - position[1]!) > 2.5) continue
    const distance = Math.hypot(pickup.x - position[0]!, pickup.z - position[2]!)
    if (distance <= bestDistance) {
      best = pickup
      bestDistance = distance
    }
  }
  return best
}

/** What E or G does here, or null when nothing. */
export function prompt(where: Where, lying: readonly Lying[], cars: ReadonlyMap<number, Placed>): string | null {
  const { position, seat } = where
  // A drone is flown from afar: there is no getting out of it.
  if (seat) return seatView(seat.kind, seat.index).board ? 'E: get out' : null
  if (where.alive && !where.grounded && position[1]! - where.ground > CHUTE_PROMPT_HEIGHT) return where.chute ? 'G: cut the parachute' : 'G: open the parachute'
  // The server hands over an item before it boards a car, so the hint follows the same order.
  const item = itemInReach(lying, position)
  if (item) return `E: take ${itemDef(item.item)?.name ?? 'item'}`
  let best: { distance: number; board: string } | null = null
  for (const car of cars.values()) {
    // A walker or a hen is boarded from between its feet, as the server does (`boarding_point`).
    const y = isLegged(car.kind) ? car.y - WALKER_LEG_LENGTH : car.y
    const distance = Math.hypot(car.x - position[0]!, y - position[1]!, car.z - position[2]!)
    const free = car.seats.indexOf(0)
    // A drone's seat is boarded by nobody: it is flown by whoever launched it.
    const board = free < 0 ? undefined : seatView(car.kind, free).board
    if (board && distance <= ENTER_RADIUS && (!best || distance < best.distance)) best = { distance, board }
  }
  return best?.board ?? null
}
