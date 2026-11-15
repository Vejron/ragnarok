import { describe, expect, it } from 'vitest'
import { IMPOSTOR_ATLAS_PX, IMPOSTOR_FRAMES, IMPOSTOR_FRAME_PX, IMPOSTOR_SPREAD, bakeImpostor, decodeView, emptyAtlas, encodeView, frameBasis, frameExtent, frameOrigin, frameView } from '@/render/impostors'
import { treeGeometries } from '@/render/trees'
import { TREE_SPRUCE } from '@/world'

describe('impostor views', () => {
  it('finds a direction again from where it falls on the grid', () => {
    for (const [x, y, z] of [[1, 0, 0], [0, 1, 0], [0.3, 0.5, -0.8], [-0.7, 0.1, 0.7]] as const) {
      const length = Math.hypot(x, y, z)
      const [a, b] = encodeView(x, y, z)
      const back = decodeView(a, b)
      expect(back[0]).toBeCloseTo(x / length)
      expect(back[1]).toBeCloseTo(y / length)
      expect(back[2]).toBeCloseTo(z / length)
    }
  })

  it('takes the grid\'s edge from the horizon and its middle from high above', () => {
    expect(frameView(0, 3)[1]).toBeCloseTo(0)
    expect(frameView(IMPOSTOR_FRAMES - 1, 5)[1]).toBeCloseTo(0)
    expect(frameView(3, 3)[1]).toBeGreaterThan(0.8)
  })

  it('crowds the rings of views toward the horizon, where a tree is mostly seen from', () => {
    const up = (ring: number) => Math.asin(frameView(ring, 3)[1]) * 180 / Math.PI
    expect(up(1)).toBeGreaterThan(3)
    expect(up(1)).toBeLessThan(9)
    expect(up(2)).toBeGreaterThan(20)
    expect(up(2)).toBeLessThan(36)
    expect(up(3)).toBeGreaterThan(60)
  })

  it('frames each picture square to its view, right level and up toward the sky', () => {
    for (let i = 0; i < IMPOSTOR_FRAMES; i++) {
      const v = frameView(i, 2)
      const { right, up } = frameBasis(v)
      expect(right[1]).toBe(0)
      expect(right[0] * v[0] + right[2] * v[2]).toBeCloseTo(0)
      expect(up[0] * v[0] + up[1] * v[1] + up[2] * v[2]).toBeCloseTo(0)
      expect(up[1]).toBeGreaterThanOrEqual(0)
    }
  })
})

describe('impostor bake', () => {
  const atlas = emptyAtlas()
  const { full, frame } = treeGeometries(TREE_SPRUCE)
  bakeImpostor(full, frame, 4, atlas)
  /** The pixel at (u, v) of picture (i, j), each 0 to 1 across and down. */
  const at = (i: number, j: number, u: number, v: number) => {
    const [x, y] = frameOrigin(4, i, j)
    return ((y + Math.floor(v * IMPOSTOR_FRAME_PX)) * IMPOSTOR_ATLAS_PX + x + Math.floor(u * IMPOSTOR_FRAME_PX)) * 4
  }

  it('sees a spruce from the side as a wide base and a narrow top', () => {
    // A side-on picture: on the grid's edge.
    const i = 0, j = 3
    const below = frame.center[1] - 0.25
    const v = (frame.center[1] - below) / frameExtent(frameView(i, j), frame)[1] * 0.5 + 0.5
    expect(atlas.albedo[at(i, j, 0.5, v) + 3]).toBe(255)
    expect(atlas.albedo[at(i, j, 0.5, 0.5) + 3]).toBe(255)
    // The top corners are sky.
    expect(atlas.albedo[at(i, j, 0.05, 0.05) + 3]).toBe(0)
    expect(atlas.albedo[at(i, j, 0.95, 0.05) + 3]).toBe(0)
  })

  it('stores normals that face the viewer (side-on at the silhouette), and a depth in front of the middle where the tree is thickest', () => {
    for (const [i, j] of [[0, 3], [5, 7], [3, 3]] as const) {
      const view = frameView(i, j)
      for (let v = 0.02; v < 1; v += 0.06) {
        for (let u = 0.02; u < 1; u += 0.06) {
          const p = at(i, j, u, v)
          if (atlas.albedo[p + 3] === 0) continue
          const n = [0, 1, 2].map((k) => atlas.normalDepth[p + k]! / 255 * 2 - 1)
          expect(n[0]! * view[0] + n[1]! * view[1] + n[2]! * view[2], `picture ${i},${j} at ${u},${v}`).toBeGreaterThan(-0.1)
        }
      }
    }
    expect(atlas.normalDepth[at(0, 3, 0.5, 0.5) + 3]!).toBeGreaterThan(128)
  })

  it('draws the same tree leaving out the faces turned away', () => {
    const whole = emptyAtlas()
    bakeImpostor(full, frame, 4, whole, false)
    let differ = 0, covered = 0
    for (let i = 3; i < whole.albedo.length; i += 4) {
      if (whole.albedo[i]) covered++
      if (whole.albedo[i] !== atlas.albedo[i] || whole.normalDepth[i] !== atlas.normalDepth[i]) differ++
    }
    expect(covered).toBeGreaterThan(1000)
    // A pixel or two on an edge the faces share may fall either way.
    expect(differ).toBeLessThan(covered * 0.002)
  })

  it('keeps the coverage a distance to the silhouette: whole well inside, a half at the edge, none well outside', () => {
    const i = 0, j = 3
    let rising = 0
    // Along a row through the middle, from the picture's edge in: none, then rising, then whole.
    let last = -1
    for (let u = 0.005; u < 0.5; u += 1 / 64) {
      const alpha = atlas.albedo[at(i, j, u, 0.5) + 3]!
      expect(alpha).toBeGreaterThanOrEqual(last)
      if (alpha > 0 && alpha < 255) rising++
      last = alpha
    }
    expect(last).toBe(255)
    expect(rising).toBeGreaterThanOrEqual(3)
  })

  it('fills a side-on picture with the tree, top to bottom and side to side', () => {
    const i = 0, j = 3
    const whole = (u: number, v: number) => atlas.albedo[at(i, j, u, v) + 3]! >= 128
    const rows = Array.from({ length: IMPOSTOR_FRAME_PX }, (_, row) => Array.from({ length: IMPOSTOR_FRAME_PX }, (_, col) => whole((col + 0.5) / IMPOSTOR_FRAME_PX, (row + 0.5) / IMPOSTOR_FRAME_PX)))
    const tall = rows.filter((row) => row.some(Boolean)).length
    const wide = Math.max(...rows.map((row) => row.filter(Boolean).length))
    expect(tall).toBeGreaterThanOrEqual(IMPOSTOR_FRAME_PX - 6)
    expect(wide).toBeGreaterThanOrEqual(IMPOSTOR_FRAME_PX - 6)
  })

  it('measures the coverage to the outline itself, so a slanting edge is straight', () => {
    // The spruce's lowest tier, side-on: a straight slope. Where each row's coverage crosses a half,
    // found between pixels, lies on one line to a tenth of a pixel.
    const i = 0, j = 3
    const [x, y] = frameOrigin(4, i, j)
    const alpha = (col: number, row: number) => atlas.albedo[((y + row) * IMPOSTOR_ATLAS_PX + x + col) * 4 + 3]! / 255
    const crossings: [number, number][] = []
    for (let row = 40; row < 52; row++) {
      for (let col = 0; col < IMPOSTOR_FRAME_PX / 2; col++) {
        const a = alpha(col, row), b = alpha(col + 1, row)
        if (a < 0.5 && b >= 0.5) {
          crossings.push([row, col + (0.5 - a) / (b - a)])
          break
        }
      }
    }
    expect(crossings.length).toBeGreaterThan(8)
    const slopes = crossings.slice(1).map(([, at], n) => at - crossings[n]![1])
    const mean = slopes.reduce((sum, slope) => sum + slope, 0) / slopes.length
    // Runs of rows on one straight edge step by the same amount.
    const straight = slopes.filter((slope) => Math.abs(slope - mean) < 0.1).length
    expect(Math.abs(mean)).toBeGreaterThan(0.2)
    expect(straight).toBe(slopes.length)
    expect(IMPOSTOR_SPREAD).toBeGreaterThan(1)
  })

  it('leaves other kinds\' tiles alone', () => {
    const [x, y] = frameOrigin(0, 3, 3)
    expect(atlas.albedo[(y * IMPOSTOR_ATLAS_PX + x + 20) * 4 + 3]).toBe(0)
  })
})
