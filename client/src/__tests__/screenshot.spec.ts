import { describe, expect, it } from 'vitest'
import { SHOT_KEYWORD, buildingAt, crc32, textOf, withText, type TiledBuilding } from '@/screenshot'
import type { Building, Point2 } from '@/world'

/** A 1 × 1 grey PNG: signature, IHDR, IDAT, IEND. */
const PIXEL = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAAAAAA6fptVAAAACklEQVR4nGNoAAAAggCBd81ytgAAAABJRU5ErkJggg=='), (c) => c.charCodeAt(0))

/** The chunk types of `png`, in order. */
function chunks(png: Uint8Array): string[] {
  const view = new DataView(png.buffer, png.byteOffset, png.byteLength)
  const out: string[] = []
  for (let at = 8; at + 12 <= png.length; at += 12 + view.getUint32(at)) out.push(new TextDecoder().decode(png.subarray(at + 4, at + 8)))
  return out
}

describe('a screenshot', () => {
  it('carries its JSON in a text chunk that reads back, after the header, the picture untouched', () => {
    const info = JSON.stringify({ camera: { position: [2038.5, 30.2, -395.1] }, world: '/worlds/home/', note: 'Kläppudden' })
    const shot = withText(PIXEL, SHOT_KEYWORD, info)
    expect(chunks(PIXEL)).toEqual(['IHDR', 'IDAT', 'IEND'])
    expect(chunks(shot)).toEqual(['IHDR', 'iTXt', 'IDAT', 'IEND'])
    expect(textOf(shot, SHOT_KEYWORD)).toBe(info)
    expect(textOf(shot, 'other')).toBeNull()
    expect(textOf(PIXEL, SHOT_KEYWORD)).toBeNull()
    // Every chunk's CRC is right, the new one included.
    const view = new DataView(shot.buffer)
    for (let at = 8; at + 12 <= shot.length; ) {
      const length = view.getUint32(at)
      expect(view.getUint32(at + 8 + length)).toBe(crc32(shot.subarray(at + 4, at + 8 + length)))
      at += 12 + length
    }
  })

  it('names the building the crosshair hits, or one just beside the hit', () => {
    const house = (footprint: Point2[]): Building => ({ footprint, holes: [], base: 0, height: 6 }) as unknown as Building
    const ref = { col: 3, row: 4, hash: 0 }
    const buildings: TiledBuilding[] = [
      { ref, index: 0, building: house([[0, 0], [10, 0], [10, 8], [0, 8]]) },
      { ref, index: 1, building: house([[20, 0], [30, 0], [30, 8], [20, 8]]) },
    ]
    expect(buildingAt([25, 4], buildings)?.index).toBe(1)
    expect(buildingAt([11.5, 4], buildings)?.index).toBe(0)
    expect(buildingAt([15, 4], buildings)).toBeNull()
  })
})
