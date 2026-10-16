import { Ramp } from '@ragnarok/engine'
import { describe, expect, it } from 'vitest'
import { buildRails } from '@/render/rails'
import { RAIL_TRAIN, RAIL_TRAM, type Rail } from '@/world'

const ramp = new Ramp(new Uint8Array([128, 255]))
const flat = () => 2
const border = [0, 0, 100, 100] as const

/** Highest vertex of a mesh above the 2 m ground. */
const top = (positions: Float32Array) => Math.max(...positions.filter((_, i) => i % 3 === 1)) - 2

describe('rails', () => {
  it('lays a train track as ballast, sleepers and two raised rails', () => {
    const rails: Rail[] = [{ gauge: 1.435, kind: RAIL_TRAIN, points: [[10, 10], [10, 50], [30, 80]] }]
    const meshes = buildRails(rails, flat, 1, border, ramp)
    expect(meshes).toHaveLength(3)
    const [ballast, sleepers, steel] = meshes.map((mesh) => mesh.geometry.positions)
    expect(top(ballast!)).toBeCloseTo(0.04)
    // About one sleeper every 0.65 m along the 76 m of track, four corners each.
    expect(sleepers!.length / 12).toBeGreaterThan(100)
    expect(top(steel!)).toBeCloseTo(0.21)
    // Both rails, at half the gauge either side of the centre line on the straight.
    const xs = new Set<number>()
    for (let i = 0; i < steel!.length; i += 3) if (steel![i + 2]! > 15 && steel![i + 2]! < 45) xs.add(Math.round(steel![i]! * 1000) / 1000)
    expect(Math.min(...xs)).toBeCloseTo(10 - 0.7175 - 0.08)
    expect(Math.max(...xs)).toBeCloseTo(10 + 0.7175 + 0.08)
  })

  it('sets a tram flush in the street, with no ballast or sleepers', () => {
    const rails: Rail[] = [{ gauge: 1.435, kind: RAIL_TRAM, points: [[10, 10], [60, 10]] }]
    const meshes = buildRails(rails, flat, 1, border, ramp)
    expect(meshes).toHaveLength(1)
    expect(top(meshes[0]!.geometry.positions)).toBeCloseTo(0.09)
  })

  it('draws nothing for a tile without tracks', () => {
    expect(buildRails([], flat, 1, border, ramp)).toEqual([])
  })
})
