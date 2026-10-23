import { keyedFaces, type Shell } from '@/sim/carve'

/**
 * A building's cells (`sim/carve.ts`) drawn as cubes: only the faces that look onto a cell with
 * nothing standing in it, merged into as few rectangles as there are runs of one colour (greedy
 * meshing, `keyedFaces`), so a plain wall is two triangles whatever its size.
 */

/** Triangle soup, metres, with a colour per vertex. */
export interface Soup {
  positions: number[]
  colors: number[]
}

/**
 * Appends the faces of the cells of `shell` for which `standing` holds to `out`, each in the colour
 * `color` gives its cell (`0xrrggbb`, so cells of one colour merge). `offset` (metres) is taken off
 * every position.
 */
export function voxelFaces(shell: Shell, standing: (id: number) => boolean, color: (id: number) => number, out: Soup, offset: readonly [number, number, number] = [0, 0, 0]) {
  // Each standing cell's colour: asked once a cell, not once a face. Only the building's own cells
  // are asked, as most of its grid is empty.
  const keys = new Int32Array(shell.dims[0] * shell.dims[1] * shell.dims[2]).fill(-1)
  for (const id of shell.cells) if (standing(id)) keys[id] = color(id)
  const faces = keyedFaces(shell.origin, shell.dims, keys)
  const [ox, oy, oz] = offset
  for (let t = 0; t < faces.length; t += 10) {
    const key = faces[t + 9]!
    const [r, g, b] = [((key >> 16) & 255) / 255, ((key >> 8) & 255) / 255, (key & 255) / 255]
    for (let k = 0; k < 9; k += 3) {
      out.positions.push(faces[t + k]! / 100 - ox, faces[t + k + 1]! / 100 - oy, faces[t + k + 2]! / 100 - oz)
      out.colors.push(r, g, b)
    }
  }
}
