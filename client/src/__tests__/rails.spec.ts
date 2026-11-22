import { describe, expect, it } from 'vitest'
import { deckRibbons, railRibbons } from '@/render/rails'
import { deckStands } from '@/render/decktrack'
import { GroundShade } from '@/render/shade'
import { RAIL_TRAIN, RAIL_TRAM, type Rail } from '@/world'

const flat = () => 2
const border = [0, 0, 100, 100] as const
const buildRails = (rails: Rail[]) => railRibbons(rails, new GroundShade(flat, 1, border), border)

/** Highest vertex of a mesh above the 2 m ground. */
const top = (positions: Float32Array) => Math.max(...positions.filter((_, i) => i % 3 === 1)) - 2

describe('rails', () => {
  it('lays a train track as ballast, sleepers and two raised rails', () => {
    const rails: Rail[] = [{ gauge: 1.435, kind: RAIL_TRAIN, points: [[10, 10], [10, 50], [30, 80]], deck: null }]
    const ribbons = buildRails(rails)
    expect(ribbons).toHaveLength(3)
    const [ballast, sleepers, steel] = ribbons.map((ribbon) => ribbon.buffers.positions)
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
    const rails: Rail[] = [{ gauge: 1.435, kind: RAIL_TRAM, points: [[10, 10], [60, 10]], deck: null }]
    const ribbons = buildRails(rails)
    expect(ribbons).toHaveLength(1)
    expect(top(ribbons[0]!.buffers.positions)).toBeCloseTo(0.09)
  })

  const onDeck = { gauge: 1.435, kind: RAIL_TRAIN, points: [[10, 10], [30, 10]] as [number, number][], deck: { building: 4, tops: [6, 7] } }

  it('lays a bridge\'s track at the deck\'s height and not on the ground under it', () => {
    expect(buildRails([onDeck])).toEqual([])
    const ribbons = deckRibbons([onDeck], () => true)
    // Sleepers and steel: the ballast is the deck's own top.
    expect(ribbons).toHaveLength(2)
    const p = ribbons[1]!.buffers.positions
    for (let i = 0; i < p.length; i += 3) {
      const above = p[i + 1]! - (6 + (p[i]! - 10) / 20)
      expect(above).toBeGreaterThan(0.11)
      expect(above).toBeLessThan(0.22)
    }
  })

  it('leaves out a bridge\'s track where its deck is blown out, and only there', () => {
    const asked = new Set<number>()
    const ribbons = deckRibbons([onDeck], (section, x) => {
      asked.add(section)
      return x < 17 || x > 23
    })
    expect([...asked]).toEqual([4])
    for (const { buffers } of ribbons) {
      const xs = [...buffers.positions].filter((_, i) => i % 3 === 0)
      expect(xs.filter((x) => x > 17.8 && x < 22.2)).toEqual([])
      expect(Math.min(...xs)).toBeLessThan(11)
      expect(Math.max(...xs)).toBeGreaterThan(29)
      // No triangle spans the hole.
      const { indices } = buffers
      for (let i = 0; i < indices.length; i += 3) {
        const corners = [0, 1, 2].map((k) => xs[indices[i + k]!]!)
        expect(Math.max(...corners) - Math.min(...corners)).toBeLessThan(1.1)
      }
    }
  })

  it('takes a deck as gone by its top cell, and as standing in a column that is another section\'s', () => {
    // Two columns, three cells high: the slab in the top two of the first, none of the second.
    const shell = { origin: [0, 0, 0], dims: [2, 3, 1], isSolid: (id: number) => id === 2 || id === 4, index: (x: number, y: number) => y * 2 + x } as never
    expect(deckStands(shell, new Set<number>(), 0.3, 2.1, 0.3)).toBe(true)
    expect(deckStands(shell, new Set([4]), 0.3, 2.1, 0.3)).toBe(false)
    expect(deckStands(shell, new Set([2]), 0.3, 2.1, 0.3)).toBe(true)
    expect(deckStands(shell, new Set([4, 2]), 1.0, 2.1, 0.3)).toBe(true)
  })

  it('draws nothing for a tile without tracks', () => {
    expect(buildRails([])).toEqual([])
  })
})
