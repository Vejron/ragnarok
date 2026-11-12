import { describe, expect, it } from 'vitest'
import { IMPOSTOR_ATLAS_PX, IMPOSTOR_FRAMES, IMPOSTOR_FRAME_PX, IMPOSTOR_MARGIN, bakeImpostor, decodeView, emptyAtlas, encodeView, frameBasis, frameOrigin, frameView } from '@/render/impostors'
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
    const v = (frame.center[1] - below) / (frame.radius * IMPOSTOR_MARGIN) * 0.5 + 0.5
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

  it('leaves other kinds\' tiles alone', () => {
    const [x, y] = frameOrigin(0, 3, 3)
    expect(atlas.albedo[(y * IMPOSTOR_ATLAS_PX + x + 20) * 4 + 3]).toBe(0)
  })
})
