import { describe, expect, it } from 'vitest'
import { ChaseCam } from '@/render/chase'

const DT = 1 / 60
const still = () => 0.5

/** Drives a car along -Z, `speed(t)` m/s at time t, and returns the last view. */
function drive(cam: ChaseCam, seconds: number, speed: (t: number) => number, start = 0) {
  let z = start
  let view = cam.update([0, 0, z], 0, DT, 9, still)
  for (let t = 0; t < seconds; t += DT) {
    z -= speed(t) * DT
    view = cam.update([0, 0, z], 0, DT, 9, still)
  }
  return { view, z }
}

describe('ChaseCam', () => {
  it('widens the view and drops back with speed', () => {
    const slow = drive(new ChaseCam(), 3, () => 5).view
    const fast = drive(new ChaseCam(), 3, () => 40).view
    expect(slow.fovBoost).toBeLessThan(2)
    expect(fast.fovBoost).toBeGreaterThan(10)
    expect(fast.distance).toBeGreaterThan(slow.distance + 1)
  })

  it('drops back when pulling away and closes in when braking', () => {
    const pulling = drive(new ChaseCam(), 1, (t) => 10 + t * 10).view
    const braking = drive(new ChaseCam(), 1, (t) => 30 - t * 20).view
    const cruising = drive(new ChaseCam(), 1, () => 20).view
    expect(pulling.distance).toBeGreaterThan(cruising.distance)
    expect(braking.distance).toBeLessThan(cruising.distance)
  })

  it('leans into a left turn', () => {
    const cam = new ChaseCam()
    let yaw = 0
    let view = cam.update([0, 0, 0], yaw, DT, 9, still)
    for (let i = 0; i < 60; i++) {
      yaw += 0.8 * DT
      view = cam.update([0, 0, -i], yaw, DT, 9, still)
    }
    expect(view.lean).toBeGreaterThan(0.05)
  })

  it('shakes when kicked and settles afterwards', () => {
    const cam = new ChaseCam()
    const { z } = drive(cam, 1, () => 30)
    cam.kick(0.7)
    const wobbly = () => 1
    const hit = cam.update([0, 0, z], 0, DT, 9, wobbly)
    expect(Math.abs(hit.shake[0])).toBeGreaterThan(0.05)
    let view = hit
    for (let i = 0; i < 120; i++) view = cam.update([0, 0, z], 0, DT, 9, wobbly)
    expect(Math.abs(view.shake[0])).toBeLessThan(1e-3)
  })

  it('does not shake under hard braking or a tight turn', () => {
    const cam = new ChaseCam()
    let z = 0
    let yaw = 0
    let worst = 0
    for (let t = 0; t < 3; t += DT) {
      z -= Math.max(0, 40 - 30 * t) * DT
      yaw += 2.5 * DT
      const view = cam.update([0, Math.sin(t * 40) * 0.05, z], yaw, DT, 9, () => 1)
      worst = Math.max(worst, ...view.shake.map(Math.abs))
    }
    expect(worst).toBe(0)
  })

  it('is quiet on a smooth cruise', () => {
    const { view } = drive(new ChaseCam(), 2, () => 20)
    expect(view.shake.every((value) => Math.abs(value) < 1e-6)).toBe(true)
  })
})
