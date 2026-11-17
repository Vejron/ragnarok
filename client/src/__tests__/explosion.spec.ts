import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Vector3 } from '@ragnarok/engine'
import { BLASTS, explosion } from '@/render/explosion'
import type { Debris } from '@/render/debris'
import type { FxWorld } from '@/render/tyres'

type Spawn = { velocity: readonly [number, number, number]; bounce?: boolean; gravity?: number }

/** Sets off a tank shell's blast whose shell was leaning `lean`, and returns what it threw: the particles and the chunks' velocities. */
function blast(lean: readonly [number, number]) {
  const particles: Spawn[] = []
  const chunks: Vector3[] = []
  const pool = { alive: 0, spawn: (p: Spawn) => particles.push(p) }
  const world = { camera: new Vector3(0, 2, 20), smoke: pool, glow: pool, surfaceAt: () => 0, groundAt: () => 0 }
  const debris = { throw: (_mesh: unknown, velocity: Vector3) => chunks.push(velocity) }
  explosion(world as unknown as FxWorld, debris as unknown as Debris, new Vector3(0, 0, 0), 5.6, BLASTS.shell, lean)
  // The dirt and the falling sparks: what is thrown, not what billows or rises.
  const thrown = particles.filter((p) => p.gravity === 9.8)
  const mean = (of: (v: readonly [number, number, number]) => number) => thrown.reduce((sum, p) => sum + of(p.velocity), 0) / thrown.length
  return { thrown, chunks, x: mean((v) => v[0]), y: mean((v) => v[1]), z: mean((v) => v[2]) }
}

describe('an explosion', () => {
  beforeEach(() => {
    // The same throws every run.
    let seed = 12345
    vi.spyOn(Math, 'random').mockImplementation(() => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646)
  })
  afterEach(() => vi.restoreAllMocks())

  it('throws its earth and sparks evenly all round for a shell from straight above', () => {
    const even = blast([0, 0])
    expect(even.thrown.length).toBeGreaterThan(40)
    expect(Math.abs(even.x)).toBeLessThan(2.5)
    expect(Math.abs(even.z)).toBeLessThan(2.5)
  })

  it('throws them ahead of a shell that skimmed in, and lower', () => {
    const even = blast([0, 0])
    const east = blast([1, 0])
    expect(east.x).toBeGreaterThan(8)
    expect(Math.abs(east.z)).toBeLessThan(2.5)
    expect(east.y).toBeLessThan(even.y)
    // The chunks go the same way, taken together.
    expect(east.chunks.length).toBeGreaterThan(0)
    expect(east.chunks.reduce((sum, velocity) => sum + velocity.x, 0) / east.chunks.length).toBeGreaterThan(2)
  })
})
