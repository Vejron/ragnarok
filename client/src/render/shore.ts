import type { TileGround } from '@/sim/terrain'
import { NO_WATER } from '@/world'

/*
 * Where a tile's ground meets its water, for what gets wet there: the ground and the painted stones in
 * the terrain's shader, and the rocks' feet (`rocks.ts`, `stones.ts`). The level of the water near each
 * cell of a grid over the tile, spread out from the water itself.
 */

/** Metres from the water that still know its level. */
export const SHORE_REACH = 12

/** The level of the water within `SHORE_REACH` of each of `n × n` cells over the tile, NaN where there is none. */
export interface Shore {
  x0: number
  z0: number
  size: number
  n: number
  levels: Float32Array
}

/** `ground`'s shore, from `water` (the tile's, on the ground's samples), or null when the tile has no water. */
export function shoreOf(ground: TileGround, water: Float32Array, n = 128): Shore | null {
  const { x0, z0, cell, side, heights } = ground
  if (water.length !== side * side) return null
  const size = cell * (side - 1)
  let levels = new Float32Array(n * n).fill(NaN)
  let any = false
  for (let j = 0; j < side; j++) {
    for (let i = 0; i < side; i++) {
      const k = j * side + i
      const level = water[k]!
      // Water stands here only over ground lower than it.
      if (level === NO_WATER || !(level > heights[k]!)) continue
      const c = Math.min(n - 1, Math.floor(((j * cell) / size) * n)) * n + Math.min(n - 1, Math.floor(((i * cell) / size) * n))
      levels[c] = Number.isNaN(levels[c]!) ? level : Math.max(levels[c]!, level)
      any = true
    }
  }
  if (!any) return null
  // Spread a cell at a time, each empty cell taking the highest level beside it.
  for (let pass = Math.ceil(SHORE_REACH / (size / n)); pass > 0; pass--) {
    const next = levels.slice()
    for (let j = 0; j < n; j++) {
      for (let i = 0; i < n; i++) {
        if (!Number.isNaN(levels[j * n + i]!)) continue
        let best = NaN
        for (let dj = -1; dj <= 1; dj++) {
          for (let di = -1; di <= 1; di++) {
            const jj = j + dj, ii = i + di
            if (jj < 0 || ii < 0 || jj >= n || ii >= n) continue
            const level = levels[jj * n + ii]!
            if (!Number.isNaN(level) && !(level <= best)) best = level
          }
        }
        next[j * n + i] = best
      }
    }
    levels = next
  }
  return { x0, z0, size, n, levels }
}

/** The water's level near `(x, z)`, or NaN where there is none within reach. */
export function shoreLevelAt(shore: Shore, x: number, z: number): number {
  const i = Math.floor(((x - shore.x0) / shore.size) * shore.n), j = Math.floor(((z - shore.z0) / shore.size) * shore.n)
  if (i < 0 || j < 0 || i >= shore.n || j >= shore.n) return NaN
  return shore.levels[j * shore.n + i]!
}

/** How wet a rock or the ground is, metres above the water, from the line the water reaches and the spray over it. */
export const WET_HEIGHT = 0.3
