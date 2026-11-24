import { COVER_ROCK, STONES_ANGULAR, STONES_DENSITY, type TileData } from '@/world'
import type { TileGround } from '@/sim/terrain'
import { coverMask } from './marshnoise'
import { shoreLevelAt, type Shore } from './shore'
import type { Pixels } from './terrainmesh'

/*
 * The maps of a tile's ground that its terrain is drawn with, worked out in the tile worker
 * (`prepareTile`) so the frame a tile arrives in does not: the stony ground, the shore and bare rock.
 * Nothing here touches the scene or the GPU.
 */

/**
 * The range of heights above the water near it that the stones texture's blue channel spans, metres
 * (`stonesTexture`; the terrain's shader reads it back in `engine/src/materials/standard.wgsl.ts`).
 * Wider than the wet band, so the texture's filtering interpolates the height between texels rather
 * than a value clamped at both: a bank rises a metre and more across a texel.
 */
export const SHORE_ABOVE: readonly [number, number] = [-1, 3]

/** Texels along the stones texture of a tile with bare rock and neither stones nor a shore. */
const ROCK_TEXELS = 128

/** Whether a tile has bare rock for the terrain's shader to draw (`stonesTexture`'s alpha). */
export const hasRock = (tile: Pick<TileData, 'cover'>): boolean => tile.cover.some((cover) => cover.kind === COVER_ROCK)

/**
 * The tile's stony ground, shore and bare rock as the pixels of a texture for the terrain's shader (`stonesTexture`): how stony in red,
 * a boulder field's blocks in green, in blue how far above the water near it the ground is (over
 * `SHORE_ABOVE`, all of it where it is higher or there is none), for the wet band along the shore, and
 * in alpha how much of the ground is bare rock (`COVER_ROCK`). Null when the tile has none of them.
 */
export function stonesPixels(tile: Pick<TileData, 'stones' | 'cover'>, shore: Shore | null, ground: TileGround): Pixels | null {
  const { stones } = tile
  const n = stones ? Math.round(Math.sqrt(stones.length)) : (shore?.n ?? ROCK_TEXELS)
  const { x0, z0, cell, side, heights } = ground
  const size = cell * (side - 1)
  const rock = coverMask(tile, x0, z0, size, n, (kind) => (kind === COVER_ROCK ? 1 : 0))
  if (!stones && !shore && !rock) return null
  const data = new Uint8Array(n * n * 4)
  for (let j = 0; j < n; j++) {
    for (let i = 0; i < n; i++) {
      const k = j * n + i
      const value = stones?.[k] ?? 0
      data[k * 4] = Math.round(((value & STONES_DENSITY) / STONES_DENSITY) * 255)
      data[k * 4 + 1] = value & STONES_ANGULAR ? 255 : 0
      let above = 1
      if (shore) {
        const x = x0 + ((i + 0.5) / n) * size, z = z0 + ((j + 0.5) / n) * size
        const level = shoreLevelAt(shore, x, z)
        // The ground at the texel's centre as the terrain draws it, between its samples.
        const u = Math.min(side - 1, Math.max(0, (x - x0) / cell)), v = Math.min(side - 1, Math.max(0, (z - z0) / cell))
        const c = Math.min(side - 2, Math.floor(u)), r = Math.min(side - 2, Math.floor(v))
        const [fu, fv] = [u - c, v - r]
        const at = (dc: number, dr: number) => heights[(r + dr) * side + c + dc]!
        const ground = (at(0, 0) * (1 - fu) + at(1, 0) * fu) * (1 - fv) + (at(0, 1) * (1 - fu) + at(1, 1) * fu) * fv
        if (!Number.isNaN(level)) above = Math.min(1, Math.max(0, (ground - level - SHORE_ABOVE[0]) / (SHORE_ABOVE[1] - SHORE_ABOVE[0])))
      }
      data[k * 4 + 2] = Math.round(above * 255)
      data[k * 4 + 3] = rock?.[k] ?? 0
    }
  }
  return { data, width: n, height: n }
}

