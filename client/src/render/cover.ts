import * as THREE from 'three'
import {
  COVER_FARM,
  COVER_INDUSTRIAL,
  COVER_PARK,
  COVER_SAND,
  COVER_URBAN,
  COVER_WATER,
  COVER_WOOD,
  ROAD_MAJOR,
  ROAD_PATH,
  type TileData,
} from '@/world'

export const GRASS_COLOR = 0x8fd16a

const COVER_COLORS: Record<number, number> = {
  [COVER_WATER]: 0x5cc0f0,
  [COVER_WOOD]: 0x5fae5a,
  [COVER_PARK]: 0xa8e07a,
  [COVER_FARM]: 0xe3e79a,
  [COVER_SAND]: 0xf3e3a6,
  [COVER_URBAN]: 0xd9d3c7,
  [COVER_INDUSTRIAL]: 0xc6c6d0,
}

/** `[casing, fill]` per road kind; paths have no casing. */
const ROAD_COLORS: Record<number, [number | null, number]> = {
  [ROAD_MAJOR]: [0xc9a86a, 0xfff1c9],
  1: [0xbdbdbd, 0xffffff],
  [ROAD_PATH]: [null, 0xd8b98a],
}
/** How much wider than the road its casing is, metres. */
const CASING = 0.8
/** Texels per metre of terrain. */
const TEXELS_PER_METRE = 2

const css = (color: number) => `#${color.toString(16).padStart(6, '0')}`

/**
 * Paints a tile's ground cover and roads, seen from above, into a texture. Canvas `(0, 0)` is the
 * tile's `(x0, z0)` corner, which is UV `(0, 0)` with `flipY` off. Null when there is no canvas
 * (tests), in which case the terrain stays plain grass.
 */
export function bakeTerrainTexture(tile: Pick<TileData, 'cover' | 'roads'>, x0: number, z0: number, size: number): THREE.CanvasTexture | null {
  if (typeof document === 'undefined') return null
  const pixels = Math.min(1024, Math.max(128, 2 ** Math.round(Math.log2(size * TEXELS_PER_METRE))))
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = pixels
  const ctx = canvas.getContext('2d')
  if (!ctx) return null
  const scale = pixels / size
  const at = ([x, z]: [number, number]) => [(x - x0) * scale, (z - z0) * scale] as const

  ctx.fillStyle = css(GRASS_COLOR)
  ctx.fillRect(0, 0, pixels, pixels)

  for (const { kind, rings } of tile.cover) {
    ctx.fillStyle = css(COVER_COLORS[kind] ?? GRASS_COLOR)
    ctx.beginPath()
    for (const ring of rings) {
      ring.forEach((p, i) => (i === 0 ? ctx.moveTo(...at(p)) : ctx.lineTo(...at(p))))
      ctx.closePath()
    }
    ctx.fill('evenodd')
  }

  ctx.lineCap = 'round'
  ctx.lineJoin = 'round'
  // Casings first, then fills, so junctions read as one surface instead of a stack of outlines.
  for (const pass of [0, 1] as const) {
    for (const road of [...tile.roads].sort((a, b) => b.kind - a.kind)) {
      const [casing, fill] = ROAD_COLORS[road.kind] ?? ROAD_COLORS[1]!
      const color = pass === 0 ? casing : fill
      if (color === null) continue
      ctx.strokeStyle = css(color)
      ctx.lineWidth = (road.width + (pass === 0 ? CASING : 0)) * scale
      ctx.beginPath()
      road.points.forEach((p, i) => (i === 0 ? ctx.moveTo(...at(p)) : ctx.lineTo(...at(p))))
      ctx.stroke()
    }
  }

  const texture = new THREE.CanvasTexture(canvas)
  texture.flipY = false
  texture.colorSpace = THREE.SRGBColorSpace
  texture.wrapS = texture.wrapT = THREE.ClampToEdgeWrapping
  texture.anisotropy = 4
  return texture
}
