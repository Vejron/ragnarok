/** Where things are on `minimap.png`, the importer's top-down render of the whole playable square. */

/** World metres (x east, z south) to a fraction `[0, 1]` across and down the image. */
export function worldToMap(x: number, z: number, halfExtent: number): [number, number] {
  const side = 2 * halfExtent
  return [(x + halfExtent) / side, (z + halfExtent) / side]
}

/**
 * CSS `background-size` and `background-position` (pixels) that centre a `view`-px box on the
 * world point `(x, z)`, showing `metres` of world across it.
 */
export function viewWindow(x: number, z: number, halfExtent: number, metres: number, view: number) {
  const imagePx = (2 * halfExtent * view) / metres
  const [u, v] = worldToMap(x, z, halfExtent)
  return { size: imagePx, left: view / 2 - u * imagePx, top: view / 2 - v * imagePx }
}

/** The inverse of `worldToMap`: a fraction across and down the image to world metres. */
export function mapToWorld(u: number, v: number, halfExtent: number): [number, number] {
  const side = 2 * halfExtent
  return [u * side - halfExtent, v * side - halfExtent]
}

/**
 * Whether world point `(x, z)`, metres, is in the playable square and on one of its tiles. Mirrors
 * `WorldFile::is_inside`.
 */
export function insideWorld(x: number, z: number, world: { halfExtent: number; tileSize: number; inside?: boolean[] | null }): boolean {
  const half = world.halfExtent
  if (Math.abs(x) > half || Math.abs(z) > half) return false
  if (!world.inside || world.tileSize <= 0) return true
  const grid = Math.max(1, Math.ceil((2 * half) / world.tileSize))
  const cell = (v: number) => Math.min(grid - 1, Math.max(0, Math.floor((v + half) / world.tileSize)))
  return world.inside[cell(z) * grid + cell(x)] === true
}
