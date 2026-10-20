import { CELL, greedyFaces, type Face, type Shell } from '@/sim/carve'

/**
 * A building's cells (`sim/carve.ts`) drawn as cubes: only the faces that look onto a cell with
 * nothing standing in it, merged into as few rectangles as there are runs of one colour (greedy
 * meshing, `greedyFaces`), so a plain wall is two triangles whatever its size.
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
  const dims = shell.dims
  const base = [0, 1, 2].map((a) => shell.origin[a]! / 100 - offset[a]!)
  // Each standing cell's colour: asked once a cell, not once a face. Only the building's own cells
  // are asked, as most of its grid is empty.
  const stands = new Uint8Array(dims[0] * dims[1] * dims[2])
  const keys = new Int32Array(stands.length)
  const ids = shell.cells.filter((id) => standing(id))
  for (const id of ids) {
    stands[id] = 1
    keys[id] = color(id)
  }
  greedyFaces(dims, stands, ids, keys, (face) => quad(out, face, base, CELL / 100))
}

/** One merged face as two triangles. */
function quad(out: Soup, { d, u, v, side, i, a, b, w, h, key }: Face, base: number[], size: number) {
  const corner = (du: number, dv: number) => {
    const p = [0, 0, 0]
    p[d] = base[d]! + (side > 0 ? i + 1 : i) * size
    p[u] = base[u]! + (a + du) * size
    p[v] = base[v]! + (b + dv) * size
    return p
  }
  const [p0, p1, p2, p3] = [corner(0, 0), corner(w, 0), corner(w, h), corner(0, h)]
  // u × v is +d: counter-clockwise seen from +d. Reversed for a face looking the other way.
  const order = side > 0 ? [p0, p1, p2, p0, p2, p3] : [p0, p2, p1, p0, p3, p2]
  const [r, g, bl] = [((key >> 16) & 255) / 255, ((key >> 8) & 255) / 255, (key & 255) / 255]
  for (const p of order) {
    out.positions.push(p[0]!, p[1]!, p[2]!)
    out.colors.push(r, g, bl)
  }
}
