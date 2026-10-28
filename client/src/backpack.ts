/** The backpack mode (BACKPACK.md), as the client hears of it: the circle it is somewhere inside, and who has it. */

/** One `backpack` message, in metres. */
export interface BackpackCircle {
  x: number
  z: number
  radius: number
  /** Who carries it, 0 when it lies on the ground. */
  carrier: number
  /** The carrier's seconds with it, in all. */
  held: number
}

/** What the overlay shows of it. */
export interface BackpackView {
  x: number
  z: number
  radius: number
  /** Who carries it, null when nobody does. */
  carrier: number | null
  /** The local player carries it. */
  yours: boolean
}

export const BACKPACK_NAME = 'Intinor Compact Backpack'

export class BackpackTracker {
  private circle: BackpackCircle | null = null

  clear() {
    this.circle = null
  }

  /** Whether the world has a backpack: once one message has come. */
  get active(): boolean {
    return this.circle !== null
  }

  /**
   * A `backpack` message. Returns what to say when it has changed hands (`nameOf` names a player),
   * or null; the first one heard says nothing, being how things already stood.
   */
  receive(circle: BackpackCircle, localId: number | null, nameOf: (id: number) => string): string | null {
    const before = this.circle
    this.circle = circle
    if (!before || before.carrier === circle.carrier) return null
    if (circle.carrier === 0) return before.carrier === localId ? `You dropped the ${BACKPACK_NAME}` : `${nameOf(before.carrier)} dropped the ${BACKPACK_NAME}`
    return circle.carrier === localId ? `You have the ${BACKPACK_NAME}!` : `${nameOf(circle.carrier)} has the ${BACKPACK_NAME}`
  }

  view(localId: number | null): BackpackView | null {
    const circle = this.circle
    if (!circle) return null
    const carrier = circle.carrier === 0 ? null : circle.carrier
    return { x: circle.x, z: circle.z, radius: circle.radius, carrier, yours: carrier !== null && carrier === localId }
  }
}
