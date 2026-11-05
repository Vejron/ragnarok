import { COVER_GOLF_BUNKER, COVER_GOLF_FAIRWAY, COVER_GOLF_GREEN, COVER_GOLF_TEE, COVER_INDUSTRIAL, COVER_PARK, COVER_ROCK, COVER_THICKET_FIGHT, COVER_THICKET_SLOW, COVER_THICKET_WALK, COVER_URBAN, COVER_WOOD, type Building, type Point2, type TileData } from '@/world'
import { rasterSurface, SURFACE_CELL, SURFACE_GRASS, SURFACE_MUD, SURFACE_MUD_DEEP, SURFACE_MUD_FAINT } from '@/sim/surface'
import type { TileGround } from '@/sim/terrain'

/*
 * Where a tile's grass grows, as a grid: worked out with the rest of the tile (`tileprep.ts`), off
 * the main thread, and read by `Grass` (`grass.ts`). Nothing here touches the scene.
 */

/** The `PALETTE` entry (`grass.ts`) for each cover's grass; 0 for none. A golf course's mown surfaces have no tall blades. */
const COVER_SHADE: Record<number, number> = {
  [COVER_PARK]: 2,
  [COVER_WOOD]: 3,
  [COVER_THICKET_SLOW]: 3,
  [COVER_THICKET_WALK]: 3,
  [COVER_THICKET_FIGHT]: 3,
  [COVER_ROCK]: 0,
  [COVER_URBAN]: 0,
  [COVER_INDUSTRIAL]: 0,
  [COVER_GOLF_FAIRWAY]: 0,
  [COVER_GOLF_TEE]: 0,
  [COVER_GOLF_GREEN]: 0,
  [COVER_GOLF_BUNKER]: 0,
}

/** The `PALETTE` entries of a marsh's sedge, by the surface's grade, which the surface grid already has. */
export const MARSH_SHADES: Record<number, number> = { [SURFACE_MUD_FAINT]: 4, [SURFACE_MUD]: 5, [SURFACE_MUD_DEEP]: 6 }

/** Where one tile has grass, on the surface grid (`SURFACE_CELL` metres a cell). */
export interface GrassMask {
  x0: number
  z0: number
  side: number
  cells: Uint8Array
}

/** Calls `paint(row, column)` for every cell of the grid whose centre is inside `rings` (even-odd). */
function fillRings(rings: readonly Point2[][], x0: number, z0: number, side: number, paint: (row: number, column: number) => void) {
  for (let row = 0; row < side; row++) {
    const z = z0 + (row + 0.5) * SURFACE_CELL
    const crossings: number[] = []
    for (const ring of rings) {
      for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
        const [ax, az] = ring[i]!
        const [bx, bz] = ring[j]!
        if (az > z !== bz > z) crossings.push(ax + ((z - az) / (bz - az)) * (bx - ax))
      }
    }
    crossings.sort((a, b) => a - b)
    for (let k = 0; k + 1 < crossings.length; k += 2) {
      const from = Math.max(0, Math.ceil((crossings[k]! - x0) / SURFACE_CELL - 0.5))
      const to = Math.min(side - 1, Math.floor((crossings[k + 1]! - x0) / SURFACE_CELL - 0.5))
      for (let column = from; column <= to; column++) paint(row, column)
    }
  }
}

/**
 * Where grass grows on a tile: ground the terrain paints green (plain grass, parks and woods), and
 * sedge in a marsh, that is not a road, track, water or sand, and not under a building. Built-up land is left bare: the
 * terrain paints it grey, and its yards are full of houses. `surface` is the tile's `rasterSurface`,
 * when it has been worked out already.
 */
export function grassMask(tile: Pick<TileData, 'cover' | 'roads' | 'water' | 'streams'> & Partial<Pick<TileData, 'rails'>> & { buildings: readonly Pick<Building, 'footprint'>[] }, x0: number, z0: number, size: number, ground: TileGround | null = null, surface = rasterSurface(tile, x0, z0, size, ground)): GrassMask {
  const { side } = surface
  const cells = new Uint8Array(side * side).fill(1)
  for (const cover of tile.cover) {
    const shade = COVER_SHADE[cover.kind]
    if (shade === undefined) continue
    fillRings(cover.rings, x0, z0, side, (row, column) => (cells[row * side + column] = shade))
  }
  for (let i = 0; i < cells.length; i++) {
    const under = surface.cells[i]!
    if (under !== SURFACE_GRASS) cells[i] = MARSH_SHADES[under] ?? 0
  }
  for (const building of tile.buildings) fillRings([building.footprint], x0, z0, side, (row, column) => (cells[row * side + column] = 0))
  // Streams are drawn as ribbons on the ground: keep the blades out of the water.
  for (const stream of tile.streams) {
    const half = stream.width / 2 + SURFACE_CELL / 2
    for (let i = 1; i < stream.points.length; i++) {
      const [ax, az] = stream.points[i - 1]!
      const [bx, bz] = stream.points[i]!
      const dx = bx - ax, dz = bz - az
      const length2 = dx * dx + dz * dz
      const c0 = Math.max(0, Math.floor((Math.min(ax, bx) - half - x0) / SURFACE_CELL))
      const c1 = Math.min(side - 1, Math.floor((Math.max(ax, bx) + half - x0) / SURFACE_CELL))
      const r0 = Math.max(0, Math.floor((Math.min(az, bz) - half - z0) / SURFACE_CELL))
      const r1 = Math.min(side - 1, Math.floor((Math.max(az, bz) + half - z0) / SURFACE_CELL))
      for (let row = r0; row <= r1; row++) {
        for (let column = c0; column <= c1; column++) {
          const px = x0 + (column + 0.5) * SURFACE_CELL - ax
          const pz = z0 + (row + 0.5) * SURFACE_CELL - az
          const t = length2 > 0 ? Math.min(1, Math.max(0, (px * dx + pz * dz) / length2)) : 0
          if ((px - t * dx) ** 2 + (pz - t * dz) ** 2 <= half * half) cells[row * side + column] = 0
        }
      }
    }
  }
  return { x0, z0, side, cells }
}
