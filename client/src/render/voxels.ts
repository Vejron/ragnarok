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

/** Set on the key of a cell that only stands in the way of a brick's faces (`voxelBrick`), so its own are dropped. */
const BORDER = 0x1000000

/**
 * The faces of the cells of `shell` in the box `lo` to `hi` (grid positions, `hi` not included) for
 * which `standing` holds, as `voxelFaces` makes them. The cells just outside the box are looked at
 * too, so a face between the box and its neighbour is left out as it is in the whole building, but
 * none of theirs are made. A big building is then redrawn a brick at a time, those a blast touched.
 */
export function voxelBrick(shell: Shell, lo: readonly number[], hi: readonly number[], standing: (id: number) => boolean, color: (id: number) => number, out: Soup) {
  const from = [0, 1, 2].map((a) => Math.max(0, lo[a]! - 1))
  const to = [0, 1, 2].map((a) => Math.min(shell.dims[a]!, hi[a]! + 1))
  const dims = [to[0]! - from[0]!, to[1]! - from[1]!, to[2]! - from[2]!]
  if (dims.some((d) => d <= 0)) return
  const keys = new Int32Array(dims[0]! * dims[1]! * dims[2]!).fill(-1)
  let any = false
  for (let y = from[1]!; y < to[1]!; y++) {
    for (let z = from[2]!; z < to[2]!; z++) {
      let at = ((y - from[1]!) * dims[2]! + (z - from[2]!)) * dims[0]!
      for (let x = from[0]!; x < to[0]!; x++, at++) {
        const id = shell.index(x, y, z)
        if (!shell.isSolid(id) || !standing(id)) continue
        const own = x >= lo[0]! && x < hi[0]! && y >= lo[1]! && y < hi[1]! && z >= lo[2]! && z < hi[2]!
        keys[at] = own ? color(id) : BORDER
        any ||= own
      }
    }
  }
  if (!any) return
  const origin = [0, 1, 2].map((a) => shell.origin[a]! + from[a]! * shell.cell)
  const faces = keyedFaces(origin, dims, keys)
  for (let t = 0; t < faces.length; t += 10) {
    const key = faces[t + 9]!
    if (key >= BORDER) continue
    const [r, g, b] = [((key >> 16) & 255) / 255, ((key >> 8) & 255) / 255, (key & 255) / 255]
    for (let k = 0; k < 9; k += 3) {
      out.positions.push(faces[t + k]! / 100, faces[t + k + 1]! / 100, faces[t + k + 2]! / 100)
      out.colors.push(r, g, b)
    }
  }
}
