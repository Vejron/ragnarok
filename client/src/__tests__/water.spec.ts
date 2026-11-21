import { Mesh } from '@ragnarok/engine'
import { describe, expect, it } from 'vitest'
import { buildWater, disposeWater } from '@/render/water'
import type { TileGround } from '@/sim/terrain'
import { FLOW_STEP, REACH_STEP } from '@/sim/waves'
import { FLOW_BIAS, LANE_STEPS, LANE_WRAP, STIR_RANGE, STIR_STEP, streamBuffers } from '@/render/watermesh'
import { NO_WATER, type TileData } from '@/world'
import { tileParts } from '@/render/tileprep'

/** 5 × 5 samples 5 m apart, flat at 2 m, with a pond of 3 × 3 samples bedded 1 m down. */
function pond() {
  const side = 5
  const heights = new Float32Array(side * side).fill(2)
  const water = new Float32Array(side * side).fill(NO_WATER)
  for (let row = 1; row <= 3; row++) {
    for (let column = 1; column <= 3; column++) {
      heights[row * side + column] = 1
      water[row * side + column] = 2
    }
  }
  const ground: TileGround = { x0: 0, z0: 0, cell: 5, side, heights }
  return { ground, water }
}

describe('water', () => {
  it('draws only the cells that touch the pond, with depth at the vertices', () => {
    const { ground, water } = pond()
    const group = buildWater({ water, streams: [] }, ground)!
    const mesh = group.children[0] as Mesh
    // The geometry's custom attribute carries depth in x.
    const custom = mesh.geometry.custom!
    const positions = mesh.geometry.positions
    const depthAt = (x: number, z: number) => {
      for (let i = 0; i < positions.length / 3; i++) if (positions[i * 3] === x && positions[i * 3 + 2] === z) return custom[i * 4]!
      return undefined
    }
    expect(depthAt(10, 10)).toBeCloseTo(1)
    expect(depthAt(0, 0)).toBeLessThan(0)
    // 3 × 3 wet samples touch 4 × 4 = 16 cells, each 5 m cell cut into 4 × 4 for the waves, two triangles each.
    expect(mesh.geometry.indexCount).toBe(16 * 16 * 6)
    disposeWater(group)
  })

  it('leans a river’s surface as its level falls, for the light, and leaves a lake’s facing straight up', () => {
    const { ground, water } = pond()
    const still = buildWater({ water, streams: [] }, { ...ground, cell: 1 })!
    const flat = (still.children[0] as Mesh).geometry.normals!
    for (let i = 0; i < flat.length / 3; i++) expect([flat[i * 3], flat[i * 3 + 1], flat[i * 3 + 2]]).toEqual([0, 1, 0])
    disposeWater(still)
    // The same water falling 20 cm a metre towards +x, and running that way.
    const falling = water.map((level, at) => (level === NO_WATER ? level : level - 0.2 * (at % 5)))
    const flow = new Int8Array(water.length * 2)
    for (let at = 0; at < water.length; at++) flow[at * 2] = 40
    const river = buildWater({ water: falling, flow, streams: [] }, { ...ground, cell: 1, heights: ground.heights.map((h) => h - 1) })!
    const geometry = (river.children[0] as Mesh).geometry
    const middle = 2 * 5 + 2
    const [nx, ny, nz] = [geometry.normals![middle * 3]!, geometry.normals![middle * 3 + 1]!, geometry.normals![middle * 3 + 2]!]
    expect(-nx / ny).toBeCloseTo(-0.2)
    expect(nz).toBeCloseTo(0)
    disposeWater(river)
  })

  it('keeps a fine grid as it is', () => {
    const { ground, water } = pond()
    const group = buildWater({ water, streams: [] }, { ...ground, cell: 1 })!
    expect((group.children[0] as Mesh).geometry.indexCount).toBe(16 * 6)
    disposeWater(group)
  })

  it('carries the open water’s reach, metres, to the waves in y', () => {
    const { ground, water } = pond()
    const reach = new Uint8Array(water.length).fill(10)
    const group = buildWater({ water, reach, streams: [] }, ground)!
    const mesh = group.children[0] as Mesh
    const custom = mesh.geometry.custom!
    for (let i = 0; i < custom.length / 4; i++) expect(custom[i * 4 + 1]).toBeCloseTo(10 * REACH_STEP)
    disposeWater(group)
  })

  it('carries a river’s current in zw, biased below a stream’s flag, and a lake’s as still', () => {
    const { ground, water } = pond()
    const flow = new Int8Array(water.length * 2)
    for (let i = 0; i < water.length; i++) flow.set([40, -20], i * 2)
    const custom = (buildWater({ water, flow, streams: [] }, ground)!.children[0] as Mesh).geometry.custom!
    for (let i = 0; i < custom.length / 4; i++) {
      expect(custom[i * 4 + 2]).toBeCloseTo(40 * FLOW_STEP)
      expect(custom[i * 4 + 3]).toBeCloseTo(-20 * FLOW_STEP - FLOW_BIAS)
    }
    const still = (buildWater({ water, streams: [] }, ground)!.children[0] as Mesh).geometry.custom!
    expect(still[2]).toBe(0)
    expect(still[3]).toBe(-FLOW_BIAS)
  })

  it('carries a river’s foam in its uv’s y, leaving x to tell the refined mesh apart, and gives still water no uvs', () => {
    const { ground, water } = pond()
    const foam = new Uint8Array(water.length).fill(102)
    const uvs = (buildWater({ water, foam, streams: [] }, ground)!.children[0] as Mesh).geometry.uvs!
    expect(uvs.length).toBeGreaterThan(0)
    for (let i = 0; i < uvs.length / 2; i++) {
      expect(uvs[i * 2]).toBe(0)
      expect(uvs[i * 2 + 1]).toBeCloseTo(0.4)
    }
    expect((buildWater({ water, streams: [] }, ground)!.children[0] as Mesh).geometry.uvs).toBeUndefined()
  })

  it('carries how a river’s current stirs its surface in its colours, and runs it on to the shore', () => {
    const { ground, water } = pond()
    const flow = new Int8Array(water.length * 2)
    const stir = new Int8Array(water.length * 2)
    const lane = new Int16Array(water.length)
    for (let at = 0; at < water.length; at++) {
      flow[at * 2] = 20
      // Welling up at 0.1 a second, spinning the other way at 0.05, and a streak and a half along.
      stir.set([0.1 / STIR_STEP, -0.05 / STIR_STEP], at * 2)
      lane[at] = 1.5 * LANE_STEPS
    }
    const geometry = (buildWater({ water, flow, stir, lane, streams: [] }, { ...ground, cell: 1 })!.children[0] as Mesh).geometry
    const colors = geometry.colors!
    expect(colors.length).toBe((geometry.positions.length / 3) * 4)
    // The pond's dry corner (0, 0) takes its wet neighbour's, as its current does.
    for (const at of [2 * 5 + 2, 0]) {
      expect((colors[at * 4]! - 0.5) * 2 * STIR_RANGE).toBeCloseTo(0.1, 2)
      expect((colors[at * 4 + 1]! - 0.5) * 2 * STIR_RANGE).toBeCloseTo(-0.05, 2)
      const turn = Math.atan2(colors[at * 4 + 3]! - 0.5, colors[at * 4 + 2]! - 0.5)
      expect((turn / (Math.PI * 2)) * LANE_WRAP).toBeCloseTo(1.5)
    }
    // A lake has none.
    expect((buildWater({ water, streams: [] }, ground)!.children[0] as Mesh).geometry.colors).toBeUndefined()
  })

  it('hands the tile worker the open water’s reach and current, so the surface it builds has the waves the sim floats things on', () => {
    const reach = new Uint8Array([1, 2, 3])
    const flow = new Int8Array([0, 0, 4, 5, 0, 0])
    const foam = new Uint8Array([0, 9, 200])
    const stir = new Int8Array([0, 0, 3, -1, 0, 0])
    const lane = new Int16Array([0, 700, 0])
    const tile = { cover: [], roads: [], rails: [], ski: [], water: new Float32Array(3), reach, flow, foam, stir, lane, streams: [], bridges: [], collision: null, buildings: [], trees: [] } as unknown as TileData
    expect(tileParts(tile).reach).toBe(reach)
    expect(tileParts(tile).flow).toBe(flow)
    expect(tileParts(tile).foam).toBe(foam)
    expect(tileParts(tile).stir).toBe(stir)
    expect(tileParts(tile).lane).toBe(lane)
  })

  it('has no mesh for a tile without water', () => {
    const { ground } = pond()
    expect(buildWater({ water: new Float32Array(), streams: [] }, ground)).toBeNull()
  })

  it('lays a stream as a ribbon lifted over the ground, carrying its distance along', () => {
    const { ground } = pond()
    const group = buildWater({ water: new Float32Array(), streams: [{ width: 2, points: [[2, 2], [18, 2]] }] }, ground)!
    const mesh = group.children[0] as Mesh
    expect(mesh.geometry.positions[1]).toBeGreaterThan(2)
    // Flow is (along, across, is-stream) in custom's y, z and w.
    const custom = mesh.geometry.custom!
    expect(custom[3]).toBe(1)
    // The 16 m line and a metre's tip past either end.
    expect(custom[(mesh.geometry.vertexCount - 1) * 4 + 1]).toBeCloseTo(18)
  })

  it('fills a stream’s channel level across, wider than its bed, so the banks draw its edge', () => {
    // A bed 2.5 m wide at 10 m along z = 0, with banks rising 1 in 1.
    const height = (_x: number, z: number) => 10 + Math.max(0, Math.abs(z) - 1.25)
    const { positions, custom } = streamBuffers([{ width: 2.5, points: [[0, 0], [10, 0]] }], height)!
    const count = positions.length / 3
    const across = Array.from({ length: count }, (_, i) => custom[i * 4 + 2]!)
    expect(Math.max(...across)).toBeGreaterThan(1.25)
    const along = Array.from({ length: count }, (_, i) => custom[i * 4 + 1]!)
    const total = Math.max(...along)
    for (let i = 0; i < count; i++) {
      const level = positions[i * 3 + 1]!
      expect(level).toBeCloseTo(10.35)
      // Depth is the water over the ground there: deepest over the bed, below zero past the banks...
      const over = level - height(positions[i * 3]!, positions[i * 3 + 2]!)
      if (along[i]! > 2 && along[i]! < total - 2) expect(custom[i * 4]).toBeCloseTo(over)
      else expect(custom[i * 4]).toBeLessThanOrEqual(over + 1e-6)
      // ...and below zero at the tips, past either end of the line, where the stream fades out.
      if (along[i] === 0 || along[i] === total) expect(custom[i * 4]).toBeLessThan(0)
    }
    expect(Math.max(...Array.from({ length: count }, (_, i) => custom[i * 4]!))).toBeCloseTo(0.35)
  })
})
