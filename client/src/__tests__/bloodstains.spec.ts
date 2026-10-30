import { describe, expect, it } from 'vitest'
import { Vector3 } from '@ragnarok/engine'
import { Bloodstains } from '@/render/bloodstains'
import { Debris } from '@/render/debris'
import { Effects } from '@/render/effects'
import { Gore } from '@/render/gore'
import { Particles, type ParticleSpec } from '@/render/particles'
import type { Character } from '@/render/character'
import { DEATH_BODY, LIMB_LEG_L, PART_TORSO } from '@/net/codec'

describe('Bloodstains', () => {
  it('lays a blot at once and joins a smear into a strip the way it was dragged', () => {
    const stains = new Bloodstains(10)
    stains.blot(0, 0, 0, 0.2)
    expect(stains.laid).toBe(1)
    stains.smear('a', new Vector3(0, 0, 0), 0.4)
    stains.smear('a', new Vector3(0, 0, -0.1), 0.4)
    expect(stains.laid).toBe(1)
    stains.smear('a', new Vector3(0, 0, -1), 0.4)
    expect(stains.laid).toBe(2)
    // A jump starts over rather than smearing across the gap.
    stains.smear('a', new Vector3(0, 0, -20), 0.4)
    stains.smear('a', new Vector3(0, 0, -21), 0.4)
    expect(stains.laid).toBe(2)
    stains.lift('a')
    stains.smear('a', new Vector3(0, 0, -22), 0.4)
    expect(stains.laid).toBe(2)
  })

  it('never holds more than its capacity: the oldest are laid over', () => {
    const stains = new Bloodstains(3)
    for (let i = 0; i < 5; i++) stains.blot(i * 10, 0, 0, 0.2)
    expect(stains.laid).toBe(5)
    const xs = (stains as unknown as { positions: Float32Array }).positions
    expect(xs.length).toBe(3 * 4 * 3)
    // Quad 0 now holds the fourth blot, quad 1 the fifth, quad 2 still the third.
    expect([xs[0]! + xs[3]!, xs[12]! + xs[15]!, xs[24]! + xs[27]!].map((x) => Math.round(x / 2))).toEqual([30, 40, 20])
  })
})

describe('Gore on the ground', () => {
  const setup = () => {
    const stains = new Bloodstains(500)
    const gore = new Gore(new Debris(), new Particles(2000), new Effects(64), stains, () => 0)
    const position = new Vector3(0, 0.9, 0)
    const pinned: Vector3[] = []
    const pin = (point: Vector3) => {
      pinned.push(point.clone())
      return (out = new Vector3()) => out.copy(point)
    }
    const avatar = { position, stump: (_: number, out = new Vector3()) => out.copy(position), launch: () => {}, pin } as unknown as Character
    return { stains, gore, avatar, pinned }
  }

  it('stains the ground where the blood from a hit comes down, not before', () => {
    const { stains, gore, avatar } = setup()
    for (let i = 0; i < 10; i++) gore.hit(avatar, PART_TORSO, 0, 40)
    expect(stains.laid).toBe(0)
    for (let t = 0; t < 2; t += 1 / 60) gore.update(1 / 60)
    expect(stains.laid).toBeGreaterThan(10)
  })

  it('leaves a pool under a body once it has come to rest', () => {
    const { stains, gore, avatar } = setup()
    gore.die(avatar, DEATH_BODY, 0, 5, () => 0)
    for (let t = 0; t < 3; t += 1 / 60) gore.update(1 / 60)
    expect(stains.laid).toBeGreaterThan(1)
  })

  it('smears the ground behind a crawler', () => {
    const { stains, gore, avatar } = setup()
    for (let z = 0; z > -3; z -= 0.5) {
      avatar.position.z = z
      gore.drag('crawl1', avatar)
    }
    expect(stains.laid).toBe(5)
  })

  it('bleeds from the spot the round struck, and out of the far side from the shooter', () => {
    const { stains, gore, avatar, pinned } = setup()
    // Shot in the left shin by a shooter to the +z (bearing pi, as 0 faces -z): it exits towards -z.
    gore.hit(avatar, LIMB_LEG_L, Math.PI, 40, { x: 0.12, y: -0.6, z: -0.1 })
    expect(pinned).toHaveLength(1)
    expect(pinned[0]!.toArray()).toEqual([0.12, 0.3, -0.1].map((v) => expect.closeTo(v, 5)))
    for (let t = 0; t < 2; t += 1 / 60) gore.update(1 / 60)
    const positions = (stains as unknown as { positions: Float32Array }).positions
    const zs: number[] = []
    for (let quad = 0; quad < stains.laid; quad++) zs.push((positions[quad * 12 + 2]! + positions[quad * 12 + 11]!) / 2)
    expect(zs.length).toBeGreaterThan(5)
    const behind = zs.filter((z) => z < -0.1).length
    expect(behind / zs.length, 'most of it lands on the far side').toBeGreaterThan(0.6)
  })

  it('bleeds at the rate it is told, however short the frames', () => {
    const particles = new Particles(2000)
    const spawned: ParticleSpec[] = []
    particles.spawn = (spec) => void spawned.push(spec)
    const gore = new Gore(new Debris(), particles, new Effects(64), new Bloodstains(500), () => 0)
    const position = new Vector3(0, 0.9, 0)
    const avatar = { position, stump: (_: number, out = new Vector3()) => out.copy(position) } as unknown as Character
    // A drip is a quarter of a second at 14 drops a second: three or four, not one a frame.
    gore.drip(avatar, LIMB_LEG_L)
    for (let t = 0; t < 1; t += 1 / 144) gore.update(1 / 144)
    expect(spawned.length).toBeGreaterThanOrEqual(3)
    expect(spawned.length).toBeLessThanOrEqual(4)
  })

  it('ends each drop where the particle reaches the ground, on a slope too', () => {
    const particles = new Particles(2000)
    const spawned: ParticleSpec[] = []
    particles.spawn = (spec) => void spawned.push(spec)
    const slope = (x: number, z: number) => 0.3 * x - 0.1 * z
    const gore = new Gore(new Debris(), particles, new Effects(64), new Bloodstains(500), slope)
    const position = new Vector3(0, 1.2, 0)
    const avatar = { position, stump: (_: number, out = new Vector3()) => out.copy(position), pin: (p: Vector3) => () => p.clone() } as unknown as Character
    gore.hit(avatar, PART_TORSO, 0.7, 40, { x: 0.1, y: 0.3, z: 0 })
    const drops = spawned.filter((spec) => spec.shape === 'drop')
    expect(drops.length).toBeGreaterThan(10)
    for (const drop of drops) {
      // Moved as the GPU moves it: drag a share of the speed each step, then gravity.
      const at = 'x' in drop.position ? [drop.position.x, drop.position.y, drop.position.z] : [...drop.position]
      const v = [...drop.velocity]
      const dt = 1 / 600
      for (let t = 0; t < drop.life - 1e-9; t += dt) {
        const keep = Math.max(0, 1 - drop.drag! * dt)
        v[0]! *= keep
        v[1]! = v[1]! * keep - drop.gravity! * dt
        v[2]! *= keep
        for (let i = 0; i < 3; i++) at[i]! += v[i]! * dt
      }
      expect(Math.abs(at[1]! - slope(at[0]!, at[2]!))).toBeLessThan(0.05)
    }
  })
})

