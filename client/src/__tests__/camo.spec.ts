import { boxGeometry } from '@ragnarok/engine'
import { describe, expect, it } from 'vitest'
import { boxUvs, camoPixels } from '@/render/camo'

const SIZE = 64
const rgb = (data: Uint8Array, i: number) => (data[i * 4]! << 16) | (data[i * 4 + 1]! << 8) | data[i * 4 + 2]!

describe('camoPixels', () => {
  const scheme = [0xc2a868, 0x4e5a38, 0x6e3b2a]
  const { data } = camoPixels(scheme, 1, SIZE)
  const distance = (a: number, b: number) =>
    Math.abs(((a >> 16) & 0xff) - ((b >> 16) & 0xff)) + Math.abs(((a >> 8) & 0xff) - ((b >> 8) & 0xff)) + Math.abs((a & 0xff) - (b & 0xff))
  /** The scheme colour a pixel is nearest. */
  const nearest = (i: number) => {
    const c = rgb(data, i)
    return scheme.reduce((best, s, k) => (distance(c, s) < distance(c, scheme[best]!) ? k : best), 0)
  }

  it('is the same for the same seed', () => {
    expect(camoPixels(scheme, 1, SIZE).data).toEqual(data)
    expect(camoPixels(scheme, 2, SIZE).data).not.toEqual(data)
  })

  it('shows every colour, the base coat most, with soft edges between them', () => {
    const counts = [0, 0, 0]
    let blended = 0
    for (let i = 0; i < SIZE * SIZE; i++) {
      counts[nearest(i)]!++
      if (!scheme.includes(rgb(data, i))) blended++
    }
    expect(counts.every((n) => n > 0)).toBe(true)
    expect(Math.max(...counts)).toBe(counts[0])
    expect(blended).toBeGreaterThan(SIZE * SIZE * 0.05)
  })

  it('tiles: the colours across the wrap are as close as those between neighbours', () => {
    const gap = (a: number, b: number) => distance(rgb(data, a), rgb(data, b))
    let wrap = 0, inner = 0
    for (let y = 0; y < SIZE; y++) {
      wrap += gap(y * SIZE + SIZE - 1, y * SIZE) + gap((SIZE - 1) * SIZE + y, y)
      inner += gap(y * SIZE + SIZE / 2 - 1, y * SIZE + SIZE / 2) + gap((SIZE / 2 - 1) * SIZE + y, (SIZE / 2) * SIZE + y)
    }
    expect(wrap).toBeLessThan(inner * 1.5 + 50)
  })
})

describe('boxUvs', () => {
  it('gives every corner a uv from the plane its face lies in', () => {
    const geometry = boxUvs(boxGeometry(3, 3, 3), 3)
    const p = geometry.positions, uv = geometry.uvs!
    expect(uv.length).toBe((p.length / 3) * 2)
    for (let v = 0; v < p.length / 3; v++) {
      const coords = [p[v * 3]! / 3, p[v * 3 + 1]! / 3, p[v * 3 + 2]! / 3]
      // Each uv is two of the corner's three coordinates in tiles.
      expect(coords.some((c) => Math.abs(c - uv[v * 2]!) < 1e-6)).toBe(true)
      expect(coords.some((c) => Math.abs(c - uv[v * 2 + 1]!) < 1e-6)).toBe(true)
    }
  })
})
