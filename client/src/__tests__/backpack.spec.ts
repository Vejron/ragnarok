import { describe, expect, it } from 'vitest'
import { BACKPACK_NAME, BackpackTracker } from '@/backpack'

const circle = (carrier: number, held = 0) => ({ x: 10, z: -20, radius: 75, carrier, held })
const name = (id: number) => `P${id}`

describe('backpack', () => {
  it('shows nothing until the world says it has one', () => {
    const tracker = new BackpackTracker()
    expect(tracker.active).toBe(false)
    expect(tracker.view(1)).toBeNull()
  })

  it('says who took it and who dropped it, but not how things stood when you came', () => {
    const tracker = new BackpackTracker()
    expect(tracker.receive(circle(4), 1, name)).toBeNull()
    expect(tracker.receive(circle(4, 1), 1, name)).toBeNull()
    expect(tracker.receive(circle(0), 1, name)).toBe(`P4 dropped the ${BACKPACK_NAME}`)
    expect(tracker.receive(circle(1), 1, name)).toBe(`You have the ${BACKPACK_NAME}!`)
    expect(tracker.receive(circle(0), 1, name)).toBe(`You dropped the ${BACKPACK_NAME}`)
    expect(tracker.receive(circle(7), 1, name)).toBe(`P7 has the ${BACKPACK_NAME}`)
  })

  it('tells the overlay the circle and whether you carry it', () => {
    const tracker = new BackpackTracker()
    tracker.receive(circle(0), 1, name)
    expect(tracker.view(1)).toEqual({ x: 10, z: -20, radius: 75, carrier: null, yours: false })
    tracker.receive(circle(1), 1, name)
    expect(tracker.view(1)?.yours).toBe(true)
    tracker.clear()
    expect(tracker.view(1)).toBeNull()
  })
})
