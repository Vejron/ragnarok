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
