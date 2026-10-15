/** The editor's view of the importer's fixed global tile grid (`osm-import`'s `grid.rs`). */

export interface GridInfo {
  origin: [number, number]
  tile: number
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

/** The tile containing a point, by the same equirectangular projection the importer uses. */
export function tileAt(grid: GridInfo, lat: number, lon: number): TileId {
  const [lat0, lon0] = grid.origin
  const x = (lon - lon0) * grid.metresPerDegLon * Math.cos((lat0 * Math.PI) / 180)
  const z = -(lat - lat0) * grid.metresPerDegLat
  return { gx: Math.floor(x / grid.tile), gy: Math.floor(z / grid.tile) }
}

/** `[south, west, north, east]` of a rectangle of `size` x `size` tiles from tile `(gx, gy)`. */
export function rectBounds(grid: GridInfo, gx: number, gy: number, size = 1): [number, number, number, number] {
  const [lat0, lon0] = grid.origin
  const perLon = grid.metresPerDegLon * Math.cos((lat0 * Math.PI) / 180)
  const side = size * grid.tile
  const x0 = gx * grid.tile
  const z0 = gy * grid.tile
  return [lat0 - (z0 + side) / grid.metresPerDegLat, lon0 + x0 / perLon, lat0 - z0 / grid.metresPerDegLat, lon0 + (x0 + side) / perLon]
}
