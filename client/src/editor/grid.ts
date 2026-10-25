/** The editor's view of the importer's fixed global tile grids (`osm-import`'s `grid.rs`), one per region. */

import { forward, inverse } from './lambert93'

export type Region = 'sweden' | 'france'

/** How the grid's metres lie on the Earth (`Region::grid` in `region.rs`). */
export type GridFrame = { kind: 'equirect'; origin: [number, number]; scaleLatitude: number } | { kind: 'lambert93'; e0: number; n0: number }

export interface GridInfo {
  region: Region
  frame: GridFrame
  /** Where to open the map, `[lat, lon]`. */
  origin: [number, number]
  tile: number
  /** The ground resolutions a world can be built at, metres between height samples. */
  cells: number[]
  blockTiles: number
  metresPerDegLat: number
  metresPerDegLon: number
}

export interface TileId {
  gx: number
  gy: number
}

export interface BlockStatus {
  bx: number
  by: number
  overpass: boolean
  hydro: boolean
  trees: boolean
}

export const tileKey = (gx: number, gy: number): string => `${gx},${gy}`

export const blockKey = (bx: number, by: number): string => `${bx},${by}`

export const blockOf = (grid: GridInfo, gx: number, gy: number): [number, number] => [
  Math.floor(gx / grid.blockTiles),
  Math.floor(gy / grid.blockTiles),
]

/** `[x, z]` grid metres of a point (+X east, +Z south), by the same projection the importer uses. */
export function project(grid: GridInfo, lat: number, lon: number): [number, number] {
  const f = grid.frame
  if (f.kind === 'lambert93') {
    const [e, n] = forward(lat, lon)
    return [e - f.e0, f.n0 - n]
  }
  const [lat0, lon0] = f.origin
  return [(lon - lon0) * grid.metresPerDegLon * Math.cos((f.scaleLatitude * Math.PI) / 180), -(lat - lat0) * grid.metresPerDegLat]
}

/** `[lat, lon]` of grid metres `(x, z)`. */
export function unproject(grid: GridInfo, x: number, z: number): [number, number] {
  const f = grid.frame
  if (f.kind === 'lambert93') return inverse(f.e0 + x, f.n0 - z)
  const [lat0, lon0] = f.origin
  return [lat0 - z / grid.metresPerDegLat, lon0 + x / (grid.metresPerDegLon * Math.cos((f.scaleLatitude * Math.PI) / 180))]
}

/** The tile containing a point. */
export function tileAt(grid: GridInfo, lat: number, lon: number): TileId {
  const [x, z] = project(grid, lat, lon)
  return { gx: Math.floor(x / grid.tile), gy: Math.floor(z / grid.tile) }
}

/**
 * The corners `[lat, lon]` of a square of `size` x `size` tiles from tile `(gx, gy)`: north-west,
 * north-east, south-east, south-west. In Lambert-93 the square is turned from north a little.
 */
export function rectCorners(grid: GridInfo, gx: number, gy: number, size = 1): [number, number][] {
  const side = size * grid.tile
  const [x0, z0] = [gx * grid.tile, gy * grid.tile]
  return [
    unproject(grid, x0, z0),
    unproject(grid, x0 + side, z0),
    unproject(grid, x0 + side, z0 + side),
    unproject(grid, x0, z0 + side),
  ]
}

/** `[south, west, north, east]` around a square of `size` x `size` tiles from tile `(gx, gy)`. */
export function rectBounds(grid: GridInfo, gx: number, gy: number, size = 1): [number, number, number, number] {
  const corners = rectCorners(grid, gx, gy, size)
  const lats = corners.map((c) => c[0])
  const lons = corners.map((c) => c[1])
  return [Math.min(...lats), Math.min(...lons), Math.max(...lats), Math.max(...lons)]
}
