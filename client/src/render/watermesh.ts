import type { TileGround } from '@/sim/terrain'
import { NO_WATER, type Stream } from '@/world'
import { FLOW_STEP, REACH_STEP } from '@/sim/waves'
import type { Height } from './shade'
import { splitsAlongMainDiagonal } from './terrainmesh'

/*
 * A tile's water as plain arrays: built in the tile worker (`tile.worker.ts`) or, without
 * one, on the main thread, and made meshes with the water's material in `water.ts`. Nothing here
 * touches the scene, so it loads in a worker.
 */

/** Ribbon cross-sections along a stream are this far apart, metres. */
const STREAM_STEP = 2
/** Vertices across a stream ribbon (an odd number, so there is a middle one). */
const STREAM_ACROSS = 7
/**
 * How high a stream's water stands over the middle of its bed, metres. The importer cuts the bed half a
 * metre under the banks (`STREAM_DEPTH` in `crates/osm-import/src/condition.rs`), so the water lies a
 * hand's breadth under them.
 */
export const STREAM_FILL = 0.35
/**
 * How far a stream's ribbon reaches past its bed on either side, metres: further than the 1-in-1 banks
 * take to rise through the water, so the ground draws its edge and the ribbon's own edge is never seen.
 */
const STREAM_OVER = 0.6
/**
 * How far a stream's ribbon runs on past each end of its line, metres, its water thinning out to nothing
 * over that stretch (`STREAM_TAPER`): it reaches into the pond it feeds, or up to the culvert it leaves
 * by, and ends in a soft edge rather than a cut.
 */
const STREAM_TIP = 1
/** How fast a stream's depth falls away towards the tip of its ribbon, metres a metre. */
const STREAM_TAPER = 0.4
/** Water cells wider than this, metres, are cut into smaller ones, fine enough for the waves to bend. */
const WAVE_CELL = 1.5
/**
 * A lake's or river's current goes in `custom.zw` as `(x, z - FLOW_BIAS)` m/s, so its `w` is always well
 * under a stream's 1 and the shader tells the two apart as before.
 */
export const FLOW_BIAS = 10
/** The step `TileData.stir` counts in, 1/s (`STIR_STEP` in `crates/shared`). */
export const STIR_STEP = 0.005
/** The most `TileData.stir` holds either way, 1/s: the vertex colour's 0 and 1 (`WaterBuffers.colors`). */
export const STIR_RANGE = 127 * STIR_STEP
/** The steps of `TileData.lane` to a streak (`LANE_STEPS` in `crates/shared`); it wraps round every 64 streaks. */
export const LANE_STEPS = 1024
/** The streaks `TileData.lane` wraps round after. */
export const LANE_WRAP = 65536 / LANE_STEPS
/**
 * A water mesh: positions, the shader's `custom` attribute (depth, then a stream's flow, or on a lake or
 * river the open water's reach in metres in y and its current in zw, `FLOW_BIAS` off in w), and indices.
 */
export interface WaterBuffers {
  positions: Float32Array
  custom: Float32Array
  /**
   * On a lake or river with foam on it (`TileData.foam`), each vertex's uv: 0, then how much of the
   * surface is under foam there, 0 to 1. The shader reads uv's x as which mesh it is (`WaterRefine`'s is 1).
   */
  uvs?: Float32Array
  /**
   * On a lake or river, which way the still surface faces at each vertex: straight up on a lake, and on
   * a river leaning as the surface falls along it and stands in waves where it runs fast (`TileData.water`
   * is shaped by the current). The shader lights the surface by it; the mesh's own triangles are too
   * coarse to.
   */
  normals?: Float32Array
  /**
   * On a river (`TileData.stir`, `TileData.lane`), each vertex's colour: how the surface wells up or
   * gathers and how fast it spins, each as `0.5 + v / (2 STIR_RANGE)`, then where it is among the
   * streaks as a turn round a circle (one turn to `LANE_WRAP` streaks), its cosine and sine as
   * `0.5 + 0.5 c`. A turn blends between vertices where the count it was made from wraps round.
   */
  colors?: Float32Array
  indices: Uint32Array
  /** A lake's or river's surface rather than a stream's ribbon: the wave field moves it (`WaterRefine`). */
  surface?: boolean
}

/**
 * The lake and river surface of one tile. Every ground sample is a vertex; only the cells that
 * touch water are triangulated. Depth is `water - ground` at the vertices and interpolates across
 * a cell, so the shoreline (depth 0) falls between a wet and a dry sample, and islands stay dry
 * because their ground is above the level. `reach` (`TileData.reach`) is how wide the water is at each
 * sample, which the waves grow with, `flow` (`TileData.flow`) which way it runs, `foam`
 * (`TileData.foam`) how much foam it carries, and `stir` and `lane` how the current stirs it and where its
 * streaks run (`WaterBuffers.colors`). Null when the tile has no water.
 */
export function surfaceBuffers(water: Float32Array, ground: TileGround, reach?: Uint8Array, flow?: Int8Array, foam?: Uint8Array, stir?: Int8Array, lane?: Int16Array): WaterBuffers | null {
  const { x0, z0, cell, side, heights } = ground
  if (water.length !== side * side) return null
  const positions = new Float32Array(side * side * 3)
  const depths = new Float32Array(side * side)
  const flows = new Float32Array(side * side * 3)
  const foamy = foam && foam.length === water.length
  const foams = foamy ? new Float32Array(side * side) : undefined
  const stirred = stir && lane && stir.length === 2 * water.length && lane.length === water.length
  const colors = stirred ? new Float32Array(side * side * 4) : undefined
  for (let row = 0; row < side; row++) {
    for (let column = 0; column < side; column++) {
      const at = row * side + column
      const level = water[at]!
      // A sample with no water is only ever a corner of a cell with some; it takes a neighbour's level below.
      const dry = level === NO_WATER
      positions.set([x0 + column * cell, dry ? heights[at]! : level, z0 + row * cell], at * 3)
      depths[at] = dry ? -1 : level - heights[at]!
      // A lake's or river's reach and current go where a stream's flow would.
      flows[at * 3] = reach && reach.length === water.length ? reach[at]! * REACH_STEP : 0
      const flowing = flow && flow.length === 2 * water.length
      flows[at * 3 + 1] = flowing ? flow[at * 2]! * FLOW_STEP : 0
      flows[at * 3 + 2] = (flowing ? flow[at * 2 + 1]! * FLOW_STEP : 0) - FLOW_BIAS
      if (foams) foams[at] = foam![at]! / 255
      if (colors) {
        const turn = (lane![at]! / 65536) * Math.PI * 2
        colors.set([0.5 + (stir![at * 2]! * STIR_STEP) / (2 * STIR_RANGE), 0.5 + (stir![at * 2 + 1]! * STIR_STEP) / (2 * STIR_RANGE), 0.5 + 0.5 * Math.cos(turn), 0.5 + 0.5 * Math.sin(turn)], at * 4)
      }
    }
  }
  const indices: number[] = []
  const wet = (at: number) => depths[at]! > 0
  // The still surface's slope at each wet sample, from the wet samples either side of it. Only running
  // water has one: a lake is level.
  const normals = new Float32Array(side * side * 3)
  const running = flow && flow.length === 2 * water.length
  const speedAt = (at: number) => Math.hypot(flows[at * 3 + 1]!, flows[at * 3 + 2]! + FLOW_BIAS)
  for (let row = 0; row < side; row++) {
    for (let column = 0; column < side; column++) {
      const at = row * side + column
      let sx = 0
      let sz = 0
      if (running && wet(at)) {
        const fall = (back: number | null, ahead: number | null) => {
          const from = back !== null && wet(back) ? back : at
          const to = ahead !== null && wet(ahead) ? ahead : at
          return from === to ? 0 : (water[to]! - water[from]!) / ((to === at || from === at ? 1 : 2) * cell)
        }
        sx = fall(column > 0 ? at - 1 : null, column < side - 1 ? at + 1 : null)
        sz = fall(row > 0 ? at - side : null, row < side - 1 ? at + side : null)
      }
      const length = Math.hypot(sx, 1, sz)
      // `|| 0`: a level surface faces (0, 1, 0), not (-0, 1, -0).
      normals.set([-sx / length || 0, 1 / length, -sz / length || 0], at * 3)
    }
  }
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
      // And they run as the fastest wet corner does (their own samples have no current), so a river is
      // a river up to its shoreline and does not fade to still water across its last cell.
      if (running) {
        let fastest = -1
        for (const p of [p00, p01, p10, p11]) if (wet(p) && (fastest < 0 || speedAt(p) > speedAt(fastest))) fastest = p
        for (const p of [p00, p01, p10, p11]) {
          if (fastest >= 0 && !wet(p) && speedAt(p) < speedAt(fastest)) {
            flows[p * 3 + 1] = flows[fastest * 3 + 1]!
            flows[p * 3 + 2] = flows[fastest * 3 + 2]!
            // And are stirred as it is, so the streaks run on to the shore and do not crowd into it.
            if (colors) colors.copyWithin(p * 4, fastest * 4, fastest * 4 + 4)
          }
        }
      }
      // Split as the terrain splits the cell, so the line where the ground rises out of the water
      // and the line where the water fades out at zero depth are the same line.
      if (splitsAlongMainDiagonal(column, row)) indices.push(p00, p10, p11, p00, p11, p01)
      else indices.push(p00, p10, p01, p10, p11, p01)
    }
  }
  if (indices.length === 0) return null
  const split = Math.ceil(cell / WAVE_CELL - 1e-6)
  if (split > 1) return subdivided(positions, depths, flows, foams, normals, colors, Uint32Array.from(indices), side, cell, split)
  return { positions, custom: surfaceCustom(depths, flows), ...(foams ? { uvs: surfaceUvs(foams) } : {}), normals, ...(colors ? { colors } : {}), indices: Uint32Array.from(indices), surface: true }
}

/**
 * The cells of a surface mesh (two triangles a cell, as `surfaceBuffers` lists them, over a grid
 * `side` samples across) each cut into `split` × `split`, their heights and depths blended from the
 * cell's corners, and so are their flows (a lake's reach), `foams`, `normals` and `colors`. Neighbouring cells repeat their shared edge's
 * vertices, which the waves move alike.
 */
function subdivided(positions: Float32Array, depths: Float32Array, flows: Float32Array, foams: Float32Array | undefined, normals: Float32Array, colors: Float32Array | undefined, cells: Uint32Array, side: number, cell: number, split: number): WaterBuffers {
  const n = split + 1
  const count = cells.length / 6
  const out = new Float32Array(count * n * n * 3)
  const deep = new Float32Array(count * n * n)
  const flow = new Float32Array(count * n * n * 3)
  const foam = foams ? new Float32Array(count * n * n) : undefined
  const facing = new Float32Array(count * n * n * 3)
  const stirs = colors ? new Float32Array(count * n * n * 4) : undefined
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
        for (let k = 0; k < 3; k++) flow[vertex * 3 + k] = blend((p) => flows[p * 3 + k]!, u, v)
        // Not made unit length again: the shader does that.
        for (let k = 0; k < 3; k++) facing[vertex * 3 + k] = blend((p) => normals[p * 3 + k]!, u, v)
        if (foam) foam[vertex] = blend((p) => foams![p]!, u, v)
        if (stirs) for (let k = 0; k < 4; k++) stirs[vertex * 4 + k] = blend((p) => colors![p * 4 + k]!, u, v)
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
  return { positions: out, custom: surfaceCustom(deep, flow), ...(foam ? { uvs: surfaceUvs(foam) } : {}), normals: facing, ...(stirs ? { colors: stirs } : {}), indices, surface: true }
}

/**
 * The vertex attribute the shader reads: depth in x, and in yzw a stream's flow (along, across, is-stream)
 * or a lake or river's reach (metres) and current (x, z - `FLOW_BIAS`, m/s).
 */
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

/** The uv attribute of a surface with foam on it: 0, then the foam (see `WaterBuffers.uvs`). */
function surfaceUvs(foams: Float32Array): Float32Array {
  const out = new Float32Array(foams.length * 2)
  for (let i = 0; i < foams.length; i++) out[i * 2 + 1] = foams[i]!
  return out
}

/**
 * Streams as water standing in their channels: cross-sections every `STREAM_STEP` metres, `STREAM_ACROSS`
 * vertices wide, level across at `STREAM_FILL` over the ground under the middle and wider than the bed,
 * so the banks rise through it. Depth is the water over the ground at each vertex, below zero past the
 * banks, where the shader fades it out; `flow` carries the distance along the stream and the offset
 * across it.
 */
export function streamBuffers(streams: readonly Stream[], height: Height): WaterBuffers | null {
  const positions: number[] = []
  const depths: number[] = []
  const flows: number[] = []
  const indices: number[] = []
  for (const { width, points: line } of streams) {
    const half = width / 2 + STREAM_OVER
    const points = withTips(line)
    let total = 0
    for (let i = 0; i + 1 < points.length; i++) total += Math.hypot(points[i + 1]![0] - points[i]![0], points[i + 1]![1] - points[i]![1])
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
        const surface = height(x, z) + STREAM_FILL
        const at = along + (length * k) / parts
        // The water thins out to nothing over the tips, half a tip past the line's own ends.
        const tip = (Math.min(at, total - at) - STREAM_TIP / 2) * STREAM_TAPER
        const section: number[] = []
        for (let j = 0; j < STREAM_ACROSS; j++) {
          const across = (2 * j) / (STREAM_ACROSS - 1) - 1
          const px = x + nx * across * half
          const pz = z + nz * across * half
          section.push(positions.length / 3)
          positions.push(px, surface, pz)
          depths.push(Math.min(surface - height(px, pz), tip))
          flows.push(at, across * half, 1)
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

/** A stream's line carried on `STREAM_TIP` past both ends, along its first and last pieces. */
function withTips(points: readonly (readonly [number, number])[]): [number, number][] {
  const out = points.map(([x, z]) => [x, z] as [number, number])
  if (out.length < 2) return out
  const reach = (from: [number, number], to: [number, number]): [number, number] => {
    const length = Math.hypot(to[0] - from[0], to[1] - from[1])
    return length < 1e-4 ? to : [to[0] + ((to[0] - from[0]) / length) * STREAM_TIP, to[1] + ((to[1] - from[1]) / length) * STREAM_TIP]
  }
  out[0] = reach(out[1]!, out[0]!)
  out[out.length - 1] = reach(out[out.length - 2]!, out[out.length - 1]!)
  return out
}
