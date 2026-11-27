import { Geometry } from '@ragnarok/engine'
import type { Shell } from '@/sim/carve'
import { lookChunk, lookFracture, type Surfaces } from './lookfracture'
import { rubbleSoup, type RubbleMemo } from './shards'
import { reachOf } from './reach'
import type { GroundGrid, HeapAsk, LookAsk, LookBuffers, LookSource, PieceBuffers, SectionAsk, SectionBuffers } from './lookworker'
import type { RGB } from './geom'

/*
 * What the look worker does for a hurt building with a look, as pure functions: redraw it with its
 * holes (`lookFracture`), cut out the sections that fell (`lookChunk`), and heap its rubble
 * (`rubbleSoup`). The main thread runs the same when there is no worker, or it failed.
 */

/** `positions` and `colors` as a mesh to hand over: flat normals (each face its own, as `shardGeometry` gives) and bounds. */
function piece(positions: Float32Array, colors: Float32Array): PieceBuffers {
  const geometry = new Geometry({ positions })
  geometry.computeNormals()
  return { positions, colors, normals: geometry.normals!, bounds: { ...geometry.bounds } }
}

/** Section `section` of the building cut out to fall whole, about its pivot, with what its fall is measured by. */
export function cutSection(source: LookSource, shell: Shell, section: SectionAsk, wall: RGB, roof: RGB, trim: Surfaces): SectionBuffers {
  const soup = lookChunk(source.look, source.range, shell, new Set(section.cells), section.pivot, wall, roof, trim)
  return { ...piece(soup.positions, soup.colors), ...reachOf(soup.positions) }
}

/** The building redrawn as `ask` has it, and the sections it asks for cut out, in order. */
export function drawLook(source: LookSource, shell: Shell, ask: LookAsk): LookBuffers {
  const dents = new Map<number, number>()
  for (let i = 0; i < ask.dents.length; i += 2) dents.set(ask.dents[i]!, ask.dents[i + 1]!)
  const soup = lookFracture(source.look, source.range, shell, new Set(ask.removed), dents, ask.wall, ask.roof, ask.surfaces)
  return { ...piece(soup.positions, soup.colors), custom: soup.custom, uvs: soup.uvs, sections: ask.sections.map((section) => cutSection(source, shell, section, ask.wall, ask.roof, ask.surfaces)) }
}

/** Height of `grid` at `(x, z)`, metres: between its four nearest samples, its edge beyond them. */
export function groundIn(grid: GroundGrid, x: number, z: number): number {
  const fx = Math.min(Math.max((x - grid.x0) / grid.step, 0), grid.nx - 1)
  const fz = Math.min(Math.max((z - grid.z0) / grid.step, 0), grid.nz - 1)
  const [i, j] = [Math.min(Math.floor(fx), grid.nx - 2), Math.min(Math.floor(fz), grid.nz - 2)]
  if (i < 0 || j < 0) return grid.heights[0] ?? 0
  const [u, v] = [fx - i, fz - j]
  const h = (a: number, b: number) => grid.heights[b * grid.nx + a]!
  return (h(i, j) * (1 - u) + h(i + 1, j) * u) * (1 - v) + (h(i, j + 1) * (1 - u) + h(i + 1, j + 1) * u) * v
}

/** The rubble of the cells gone from the building heaped on the ground round it; `memo` keeps the mounds of squares that did not change. */
export function heapRubble(shell: Shell, ask: HeapAsk, memo: RubbleMemo): PieceBuffers {
  const ids = [...ask.removed].filter((id) => shell.isSolid(id)).sort((a, b) => a - b)
  const centres = ids.map((id) => shell.centre(id).map((v) => v / 100) as unknown as [number, number, number])
  const soup = rubbleSoup(centres, ids.map((id) => shell.isRoof(id)), (x, z) => groundIn(ask.ground, x, z), ask.wall, ask.roof, memo)
  return piece(soup.positions, soup.colors)
}
