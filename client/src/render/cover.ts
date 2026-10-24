import { Texture } from '@ragnarok/engine'
import type { TileData } from '@/world'
import { coverPixels, paintCover, type Pixels } from './terrainmesh'

export { COVER_COLORS, GRASS_COLOR } from './terrainmesh'

/**
 * Paints a tile's ground cover, seen from above, into a texture (`paintCover`). Null when there is
 * no canvas (tests), in which case the terrain stays plain grass. The terrain worker paints the
 * same on an `OffscreenCanvas` instead (`terrain.worker.ts`).
 */
export function bakeTerrainTexture(tile: Pick<TileData, 'cover'>, x0: number, z0: number, size: number): Texture | null {
  if (typeof document === 'undefined') return null
  const pixels = coverPixels(size)
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = pixels
  const ctx = canvas.getContext('2d')
  if (!ctx) return null
  paintCover(ctx, tile, x0, z0, size, pixels)
  return coverTexture(canvas)
}

/** The texture of a painted cover canvas or bitmap. */
export function coverTexture(source: HTMLCanvasElement | ImageBitmap): Texture {
  return new Texture(source, { srgb: true, wrap: 'clamp', anisotropy: 4 })
}

/** The texture of a tile's relief map (`reliefPixels`). Data, not colour: sampled as is, with mipmaps. */
export function reliefTexture(pixels: Pixels): Texture {
  return new Texture(pixels, { srgb: false, wrap: 'clamp', mipmaps: true, anisotropy: 4 })
}
