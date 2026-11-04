import type { TileGround } from '@/sim/terrain'
import { NO_WATER, type Stream } from '@/world'
import type { Height } from './shade'

/*
 * A tile's water as plain arrays: built in the tile worker (`tile.worker.ts`) or, without
 * one, on the main thread, and made meshes with the water's material in `water.ts`. Nothing here
 * touches the scene, so it loads in a worker.
 */

/** Ribbon cross-sections along a stream are this far apart, metres. */
const STREAM_STEP = 2
/** Vertices across a stream ribbon (an odd number, so there is a middle one). */
const STREAM_ACROSS = 5
/** How far a stream floats over the ground, metres. */
const STREAM_LIFT = 0.1
/** Depth drawn at the middle of a stream, metres. */
const STREAM_DEPTH = 0.45
/** Water cells wider than this, metres, are cut into smaller ones, fine enough for the waves to bend. */
const WAVE_CELL = 1.5
/** A water mesh: positions, the shader's `custom` attribute (depth, then the flow), and indices. */
export interface WaterBuffers {
  positions: Float32Array
  custom: Float32Array
  indices: Uint32Array
}

/**
 * The lake and river surface of one tile. Every ground sample is a vertex; only the cells that
 * touch water are triangulated. Depth is `water - ground` at the vertices and interpolates across
 * a cell, so the shoreline (depth 0) falls between a wet and a dry sample, and islands stay dry
 * because their ground is above the level. Null when the tile has no water.
 */
export function surfaceBuffers(water: Float32Array, ground: TileGround): WaterBuffers | null {
  const { x0, z0, cell, side, heights } = ground
  if (water.length !== side * side) return null
  const positions = new Float32Array(side * side * 3)
  const depths = new Float32Array(side * side)
  const flows = new Float32Array(side * side * 3)
  for (let row = 0; row < side; row++) {
    for (let column = 0; column < side; column++) {
      const at = row * side + column
      const level = water[at]!
      // A sample with no water is only ever a corner of a cell with some; it takes a neighbour's level below.
      const dry = level === NO_WATER
      positions.set([x0 + column * cell, dry ? heights[at]! : level, z0 + row * cell], at * 3)
      depths[at] = dry ? -1 : level - heights[at]!
    }
  }
  const indices: number[] = []
  const wet = (at: number) => depths[at]! > 0
  for (let row = 0; row < side - 1; row++) {
    for (let column = 0; column < side - 1; column++) {
      const p00 = row * side + column
      const p01 = p00 + 1
      const p10 = p00 + side
      const p11 = p10 + 1
      if (!(wet(p00) || wet(p01) || wet(p10) || wet(p11))) continue
      // Dry corners of a wet cell sit at the cell's water level so the surface is flat across it.
      let level = -Infinity
      for (const p of [p00, p01, p10, p11]) if (water[p] !== NO_WATER) level = Math.max(level, water[p]!)
      for (const p of [p00, p01, p10, p11]) positions[p * 3 + 1] = water[p] === NO_WATER ? level : water[p]!
      indices.push(p00, p10, p01, p10, p11, p01)
    }
  }
  if (indices.length === 0) return null
  const split = Math.ceil(cell / WAVE_CELL - 1e-6)
  if (split > 1) return subdivided(positions, depths, Uint32Array.from(indices), side, cell, split)
  return { positions, custom: surfaceCustom(depths, flows), indices: Uint32Array.from(indices) }
}

/**
 * The cells of a surface mesh (two triangles a cell, as `surfaceBuffers` lists them, over a grid
 * `side` samples across) each cut into `split` × `split`, their heights and depths blended from the
 * cell's corners. Neighbouring cells repeat their shared edge's vertices, which the waves move alike.
 */
function subdivided(positions: Float32Array, depths: Float32Array, cells: Uint32Array, side: number, cell: number, split: number): WaterBuffers {
  const n = split + 1
  const count = cells.length / 6
  const out = new Float32Array(count * n * n * 3)
  const deep = new Float32Array(count * n * n)
  const indices = new Uint32Array(count * split * split * 6)
  let vertex = 0
  let index = 0
  for (let c = 0; c < count; c++) {
    const p00 = cells[c * 6]!
    const [p01, p10, p11] = [p00 + 1, p00 + side, p00 + side + 1]
    const x0 = positions[p00 * 3]!
    const z0 = positions[p00 * 3 + 2]!
    const blend = (of: (p: number) => number, u: number, v: number) => {
      const near = of(p00) + (of(p01) - of(p00)) * u
      const far = of(p10) + (of(p11) - of(p10)) * u
      return near + (far - near) * v
    }
    const first = vertex
    for (let j = 0; j < n; j++) {
      for (let i = 0; i < n; i++) {
        const u = i / split
        const v = j / split
        out.set([x0 + u * cell, blend((p) => positions[p * 3 + 1]!, u, v), z0 + v * cell], vertex * 3)
        deep[vertex++] = blend((p) => depths[p]!, u, v)
      }
    }
    for (let j = 0; j < split; j++) {
      for (let i = 0; i < split; i++) {
        const q00 = first + j * n + i
        const [q01, q10, q11] = [q00 + 1, q00 + n, q00 + n + 1]
        indices.set([q00, q10, q01, q10, q11, q01], index)
        index += 6
      }
    }
  }
  return { positions: out, custom: surfaceCustom(deep, new Float32Array(deep.length * 3)), indices }
}

/** The vertex attribute the shader reads: depth in x and the stream flow (along, across, is-stream) in yzw. */
function surfaceCustom(depths: Float32Array, flows: Float32Array): Float32Array {
  const out = new Float32Array(depths.length * 4)
  for (let i = 0; i < depths.length; i++) {
    out[i * 4] = depths[i]!
    out[i * 4 + 1] = flows[i * 3]!
    out[i * 4 + 2] = flows[i * 3 + 1]!
    out[i * 4 + 3] = flows[i * 3 + 2]!
  }
  return out
}

/**
 * Streams as ribbons on the ground: cross-sections every `STREAM_STEP` metres, `STREAM_ACROSS`
 * vertices wide, each lifted a hair over the terrain. Depth is deepest in the middle and zero at
 * the banks; `flow` carries the distance along the stream and the offset across it.
 */
export function streamBuffers(streams: readonly Stream[], height: Height): WaterBuffers | null {
  const positions: number[] = []
  const depths: number[] = []
  const flows: number[] = []
  const indices: number[] = []
  for (const { width, points } of streams) {
    const half = width / 2
    let along = 0
    let previous: number[] | null = null
    for (let i = 0; i + 1 < points.length; i++) {
      const [ax, az] = points[i]!
      const [bx, bz] = points[i + 1]!
      const length = Math.hypot(bx - ax, bz - az)
      if (length < 1e-4) continue
      const nx = -(bz - az) / length
      const nz = (bx - ax) / length
      const parts = Math.max(1, Math.ceil(length / STREAM_STEP))
      // The first cross-section of a segment repeats the last of the one before; skip it there.
      for (let k = previous ? 1 : 0; k <= parts; k++) {
        const x = ax + ((bx - ax) * k) / parts
        const z = az + ((bz - az) * k) / parts
        const section: number[] = []
        for (let j = 0; j < STREAM_ACROSS; j++) {
          const across = (2 * j) / (STREAM_ACROSS - 1) - 1
          const px = x + nx * across * half
          const pz = z + nz * across * half
          section.push(positions.length / 3)
          positions.push(px, height(px, pz) + STREAM_LIFT, pz)
          depths.push(STREAM_DEPTH * (1 - across * across))
          flows.push(along + (length * k) / parts, across * half, 1)
        }
        if (previous) {
          for (let j = 0; j + 1 < STREAM_ACROSS; j++) {
            const [a, b, c, d] = [previous[j]!, previous[j + 1]!, section[j]!, section[j + 1]!]
            // Facing up whichever way the line runs.
            const up = (positions[b * 3 + 2]! - positions[a * 3 + 2]!) * (positions[c * 3]! - positions[a * 3]!) - (positions[b * 3]! - positions[a * 3]!) * (positions[c * 3 + 2]! - positions[a * 3 + 2]!)
            if (up > 0) indices.push(a, b, c, b, d, c)
            else indices.push(a, c, b, b, c, d)
          }
        }
        previous = section
      }
      along += length
    }
  }
  if (indices.length === 0) return null
  return {
    positions: Float32Array.from(positions),
    custom: surfaceCustom(Float32Array.from(depths), Float32Array.from(flows)),
    indices: Uint32Array.from(indices),
  }
}

