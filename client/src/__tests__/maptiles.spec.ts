import { describe, expect, it, vi } from 'vitest'
import { MAX_TILES, MapTiles, type MapIndex, type TileImage } from '@/hud/maptiles'

/** A world 1 km across (half 500 m) in 3 levels of 512 px tiles: 1024, 512 and 256 m. */
const INDEX: MapIndex = { levels: 3, tile_px: 512, top_metres: 1024, origin: [-500, -500], hash: 42 }

const image = (): TileImage => ({ width: 512, height: 512, close: vi.fn() }) as unknown as TileImage

function recorder() {
  const drawn: unknown[][] = []
  const g = { drawImage: (...args: unknown[]) => drawn.push(args), getTransform: () => ({ a: 1 }) } as unknown as CanvasRenderingContext2D
  return { g, drawn }
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0))

describe('MapTiles', () => {
  it('picks the coarsest level that is still sharp', async () => {
    const tiles = new MapTiles()
    await tiles.init('/', async () => INDEX)
    expect(tiles.levelFor(2)).toBe(0) // 1024 m over 512 px is 2 m a pixel
    expect(tiles.levelFor(1)).toBe(1)
    expect(tiles.levelFor(0.5)).toBe(2)
    expect(tiles.levelFor(0.1)).toBe(2) // as fine as there is
    expect(tiles.levelFor(10)).toBe(0)
  })

  it('asks for the tiles in view by the render\'s hash, and draws them once they come', async () => {
    const urls: string[] = []
    const tiles = new MapTiles(async (url) => (urls.push(url), image()))
    const arrived = vi.fn()
    tiles.onTile = arrived
    await tiles.init('/w/', async () => INDEX)
    const { g, drawn } = recorder()
    // 200 m across 400 px around the middle: half a metre a pixel, level 2, the four tiles round (0, 0).
    tiles.draw(g, 0, 0, 2, 400, 400)
    expect(urls.sort()).toEqual(['/w/maptiles/2/1_1.webp?h=42', '/w/maptiles/2/1_2.webp?h=42', '/w/maptiles/2/2_1.webp?h=42', '/w/maptiles/2/2_2.webp?h=42'])
    expect(drawn).toHaveLength(0)
    await flush()
    expect(arrived).toHaveBeenCalled()
    tiles.draw(g, 0, 0, 2, 400, 400)
    expect(drawn).toHaveLength(4)
  })

  it('draws the coarser tile over a square still loading', async () => {
    const pending: ((image: TileImage | null) => void)[] = []
    const tiles = new MapTiles((url) => (url.includes('/0/') ? Promise.resolve(image()) : new Promise((resolve) => pending.push(resolve))))
    await tiles.init('/', async () => INDEX)
    const { g, drawn } = recorder()
    tiles.draw(g, 0, 0, 0.25, 256, 256) // level 0
    await flush()
    drawn.length = 0
    tiles.draw(g, -400, -400, 2, 100, 100) // level 2, its north-west corner: not loaded yet
    expect(drawn).toHaveLength(1)
    // The part of the level-0 tile over it: a quarter across, from its top left.
    expect(drawn[0]!.slice(1, 5)).toEqual([0, 0, 128, 128])
  })

  it('leaves out a square with no tile, and lets the oldest go beyond its limit', async () => {
    let count = 0
    const tiles = new MapTiles(async () => (count++ === 0 ? null : image()))
    await tiles.init('/', async () => ({ ...INDEX, levels: 9, top_metres: 1024 * 64 }))
    const { g, drawn } = recorder()
    tiles.draw(g, -400, -400, 2, 8, 8)
    await flush()
    tiles.draw(g, -400, -400, 2, 8, 8)
    expect(drawn).toHaveLength(0)
    for (let i = 0; i < MAX_TILES + 40; i++) {
      tiles.draw(g, -500 + (i % 60) * 4 + 2, -500 + Math.floor(i / 60) * 4 + 2, 128, 4, 4)
      await flush()
    }
    expect(tiles.size).toBeLessThanOrEqual(MAX_TILES)
  })
})
