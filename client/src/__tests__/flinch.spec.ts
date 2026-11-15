import { describe, expect, it } from 'vitest'
import { Flinch } from '@/render/flinch'

/** How far vehicle 1 is tipped from upright, radians, after `seconds` of rocking from one blow. */
function rocked(size: number, seconds: number[]) {
  const flinch = new Flinch()
  // Struck high on its front, the round going back along +Z.
  flinch.hit(1, size, { x: 0, y: 0, z: 0 }, { x: 0, y: 10 * size, z: -2 }, { x: 0, y: 0, z: 1 }, 1)
  const tilts: number[] = []
  let t = 0
  for (const at of seconds) {
    const position: [number, number, number] = [0, 2 * size, 0]
    const rotation: [number, number, number, number] = [0, 0, 0, 1]
    while (t < at) {
      position.splice(0, 3, 0, 2 * size, 0)
      rotation.splice(0, 4, 0, 0, 0, 1)
      flinch.pose(1, position, rotation, 2 * size, 1 / 60)
      t += 1 / 60
    }
    tilts.push(2 * Math.asin(Math.min(1, Math.hypot(rotation[0], rotation[1], rotation[2]))))
  }
  return tilts
}

describe('a vehicle struck by a round', () => {
  it('rocks back from the blow and comes to rest', () => {
    const [early, , late] = rocked(1, [0.08, 1, 4])
    expect(early).toBeGreaterThan(0.005)
    expect(late).toBeLessThan(0.0005)
  })

  it('rocks less and slower the bigger it is: a Titan sways where a tank jolts', () => {
    const tank = rocked(1, [0.05])[0]!
    const titan = rocked(2.6, [0.05])[0]!
    expect(titan).toBeLessThan(tank)
    expect(titan).toBeGreaterThan(0)
  })
})
