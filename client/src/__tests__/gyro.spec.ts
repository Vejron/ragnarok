import { afterEach, describe, expect, it } from 'vitest'
import { followGravity, GYRO_MAX_DT, gyroTurn } from '@/sim/gyro'
import { InputController } from '@/sim/input'

const rate = (alpha: number, beta: number, gamma: number) => ({ alpha, beta, gamma })
const DEG = Math.PI / 180

describe('gyroTurn', () => {
  it('turning the phone left in landscape turns the view left, either way round', () => {
    // Top of the phone to the left (90): the device's x axis points up the screen.
    expect(gyroTurn(rate(30, 0, 0), 90, 0.1).yaw).toBeCloseTo(3 * DEG)
    // Top to the right (270): x points down the screen, so the same turn left reads the other sign.
    expect(gyroTurn(rate(-30, 0, 0), 270, 0.1).yaw).toBeCloseTo(3 * DEG)
  })

  it('tipping the top of the screen away raises the view', () => {
    // At 90 the screen's right edge is the device's −y, so tipping back is a negative beta.
    expect(gyroTurn(rate(0, -20, 0), 90, 0.1).pitch).toBeCloseTo(2 * DEG)
    expect(gyroTurn(rate(0, 20, 0), 270, 0.1).pitch).toBeCloseTo(2 * DEG)
  })

  it('in portrait the axes are the device own', () => {
    const turn = gyroTurn(rate(10, 20, 0), 0, 0.1)
    expect(turn.yaw).toBeCloseTo(2 * DEG)
    expect(turn.pitch).toBeCloseTo(1 * DEG)
  })

  it('a hand holding still and a steering-wheel roll of an upright phone do nothing', () => {
    expect(gyroTurn(rate(0.3, -0.3, 0), 90, 0.1)).toEqual({ yaw: 0, pitch: 0 })
    expect(gyroTurn(rate(0, 0, 45), 90, 0.1)).toEqual({ yaw: 0, pitch: 0 })
  })

  it('a phone held flat turns the view by its spin about the screen', () => {
    // Lying face up, the world's up is out of the screen: turning on the spot spins it about z.
    expect(gyroTurn(rate(0, 0, 30), 90, 0.1, [0, 0, 9.8]).yaw).toBeCloseTo(3 * DEG)
  })

  it('reads gravity the same whichever sign the browser gives it', () => {
    const tilted = rate(20, 0, 20)
    const spec = gyroTurn(tilted, 90, 0.1, [6.9, 0, 6.9])
    const flipped = gyroTurn(tilted, 90, 0.1, [-6.9, 0, -6.9])
    expect(spec.yaw).toBeCloseTo(flipped.yaw)
    expect(spec.yaw).toBeCloseTo(Math.SQRT2 * 2 * DEG)
  })

  it('a long gap between readings counts as no more than the cap', () => {
    expect(gyroTurn(rate(30, 0, 0), 90, 5).yaw).toBeCloseTo(30 * DEG * GYRO_MAX_DT)
    expect(gyroTurn(rate(30, 0, 0), 90, -1).yaw).toBe(0)
  })

  it('gravity is smoothed towards each reading', () => {
    expect(followGravity(null, [0, 9.8, 0])).toEqual([0, 9.8, 0])
    const next = followGravity([0, 9.8, 0], [9.8, 0, 0])
    expect(next[0]).toBeGreaterThan(0)
    expect(next[0]).toBeLessThan(9.8)
  })
})

describe('InputController.turnBy', () => {
  let input: InputController | null = null
  afterEach(() => input?.dispose())

  it('turns by radians, scaled down while aiming, and keeps the pitch in range', () => {
    input = new InputController(document.createElement('div'))
    const yaw = input.yaw
    input.turnBy(0.2, 0)
    expect(input.yaw).toBeCloseTo(yaw + 0.2)
    input.lookScale = 0.5
    input.turnBy(0.2, 0)
    expect(input.yaw).toBeCloseTo(yaw + 0.3)
    input.turnBy(0, 10)
    expect(input.pitch).toBeCloseTo(0.6)
    input.turnBy(0, -10)
    expect(input.pitch).toBeCloseTo(-1.2)
  })

  it('gyro aim is idle while off or before the player starts', () => {
    input = new InputController(document.createElement('div'))
    expect(input.gyroActive).toBe(false)
    input.gyroMode = 'always'
    expect(input.gyroActive).toBe(false)
  })
})
