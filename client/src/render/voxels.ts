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
 * every position. `only` names the cells that can stand, when that is a few of them (a chunk
 * thrown out of a big building): the grid meshed is then just their bounding box, not the building's.
 */
export function voxelFaces(shell: Shell, standing: (id: number) => boolean, color: (id: number) => number, out: Soup, offset: readonly [number, number, number] = [0, 0, 0], only?: Iterable<number>) {
  // Each standing cell's colour: asked once a cell, not once a face. Only the building's own cells
  // are asked, as most of its grid is empty.
  let origin: readonly number[] = shell.origin
  let dims: readonly number[] = shell.dims
  let keys: Int32Array
  if (only) {
    const lo = [Infinity, Infinity, Infinity]
    const hi = [-1, -1, -1]
    const ids: number[] = []
    for (const id of only) {
      if (!shell.isSolid(id) || !standing(id)) continue
      ids.push(id)
      const at = shell.coords(id)
      for (let a = 0; a < 3; a++) {
        lo[a] = Math.min(lo[a]!, at[a]!)
        hi[a] = Math.max(hi[a]!, at[a]!)
      }
    }
    if (ids.length === 0) return
    dims = [hi[0]! - lo[0]! + 1, hi[1]! - lo[1]! + 1, hi[2]! - lo[2]! + 1]
    origin = [shell.origin[0] + lo[0]! * shell.cell, shell.origin[1] + lo[1]! * shell.cell, shell.origin[2] + lo[2]! * shell.cell]
    keys = new Int32Array(dims[0]! * dims[1]! * dims[2]!).fill(-1)
    for (const id of ids) {
      const at = shell.coords(id)
      keys[((at[1]! - lo[1]!) * dims[2]! + (at[2]! - lo[2]!)) * dims[0]! + (at[0]! - lo[0]!)] = color(id)
    }
  } else {
    keys = new Int32Array(dims[0]! * dims[1]! * dims[2]!).fill(-1)
    for (const id of shell.cells) if (standing(id)) keys[id] = color(id)
  }
  const faces = keyedFaces(origin, dims, keys)
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
