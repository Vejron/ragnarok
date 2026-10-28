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

/** How far in from the small map's edge the pip pointing at a circle out of view sits, pixels. */
export const PIP_INSET = 9

/**
 * A circle of `radius` metres about world `(cx, cz)` on a `view`-px small map showing `metres` across,
 * centred on `(x, z)`: its middle and radius in pixels, and, when none of it is in view, where on the
 * map's edge a pip points the way to it (else null).
 */
export function circleOnSmall(cx: number, cz: number, radius: number, x: number, z: number, metres: number, view: number) {
  const scale = view / metres
  const left = view / 2 + (cx - x) * scale
  const top = view / 2 + (cz - z) * scale
  const r = radius * scale
  // The nearest point of the map's square to the circle's middle: if that is outside the circle, none of it shows.
  const nearX = Math.min(view, Math.max(0, left))
  const nearY = Math.min(view, Math.max(0, top))
  const seen = Math.hypot(nearX - left, nearY - top) <= r
  let pip: { left: number; top: number } | null = null
  if (!seen) {
    const [dx, dy] = [left - view / 2, top - view / 2]
    const reach = view / 2 - PIP_INSET
    const stretch = reach / Math.max(Math.abs(dx), Math.abs(dy))
    pip = { left: view / 2 + dx * stretch, top: view / 2 + dy * stretch }
  }
  return { left, top, r, pip }
}
