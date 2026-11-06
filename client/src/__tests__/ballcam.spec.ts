import { describe, expect, it } from 'vitest'
import { BallCam } from '../render/ballcam'

const flat = () => 0
const DT = 1 / 60

/** Steps `cam` `seconds` with the ball where `ball(t)` says, the player's view at the origin. */
function run(cam: BallCam, seconds: number, ball: (t: number) => { x: number; y: number; z: number } | null, from = 0) {
  let view = null
  for (let t = from; t < from + seconds; t += DT) view = cam.update(ball(t), [0, 2, 3], [0, 2, -7], DT, flat)
  return view
}

describe('the golf ball camera', () => {
  it('takes over after the follow-through and trails the ball from behind and above', () => {
    const cam = new BallCam()
    cam.start(0)
    const flying = (t: number) => ({ x: 0, y: 1 + 5 * t, z: -50 * t })
    expect(run(cam, 0.2, flying)).toBeNull()
    const view = run(cam, 2, flying, 0.2)!
    const ball = flying(2.2)
    expect(view.at[2]).toBeCloseTo(ball.z, 0)
    expect(view.eye[2]).toBeGreaterThan(ball.z + 5)
    expect(view.eye[1]).toBeGreaterThan(ball.y)
  })

  it('hands the view back once the ball has lain still, or the player moves on', () => {
    const cam = new BallCam()
    cam.start(0)
    run(cam, 1, (t) => ({ x: 0, y: 0, z: -40 * Math.min(t, 0.6) }))
    expect(cam.active).toBe(true)
    expect(run(cam, 5, () => ({ x: 0, y: 0, z: -24 }), 1)).toBeNull()
    cam.start(0)
    run(cam, 1, (t) => ({ x: 0, y: 3, z: -40 * t }))
    cam.stop()
    expect(run(cam, 3, (t) => ({ x: 0, y: 3, z: -40 * t }), 1)).toBeNull()
  })
})
