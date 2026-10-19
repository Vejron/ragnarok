import { CELL, type Shell } from '@/sim/carve'

/**
 * A building's cells (`sim/carve.ts`) drawn as cubes: only the faces that look onto a cell with
 * nothing standing in it, merged into as few rectangles as there are runs of one colour (greedy
 * meshing), so a plain wall is two triangles whatever its size.
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
  const size = CELL / 100
  const base = [0, 1, 2].map((a) => shell.origin[a]! / 100 - offset[a]!)
  // Each cell's colour, or -1 where nothing stands: asked once a cell, not once a face.
  const keys = new Int32Array(dims[0] * dims[1] * dims[2]).fill(-1)
  for (let id = 0; id < keys.length; id++) if (standing(id)) keys[id] = color(id)
  // Step in the cell number for one cell along x, y and z (`Shell::index`).
  const stride = [1, dims[0] * dims[2], dims[0]]
  for (let d = 0; d < 3; d++) {
    const u = (d + 1) % 3
    const v = (d + 2) % 3
    const [nu, nv, nd] = [dims[u]!, dims[v]!, dims[d]!]
    const [su, sv, sd] = [stride[u]!, stride[v]!, stride[d]!]
    // The colour of the face at each (u, v) of the slice, or -1 for none.
    const mask = new Int32Array(nu * nv)
    for (const side of [1, -1]) {
      for (let i = 0; i < nd; i++) {
        const j = i + side
        const beyond = j >= 0 && j < nd
        let any = false
        for (let b = 0; b < nv; b++) {
          for (let a = 0; a < nu; a++) {
            const id = i * sd + a * su + b * sv
            const key = keys[id]!
            mask[b * nu + a] = key >= 0 && !(beyond && keys[id + side * sd]! >= 0) ? key : -1
            if (mask[b * nu + a]! >= 0) any = true
          }
        }
        if (!any) continue
        // Greedy: grow each unclaimed face along u, then along v while the whole row matches.
        for (let b = 0; b < nv; b++) {
          for (let a = 0; a < nu; ) {
            const key = mask[b * nu + a]!
            if (key < 0) {
              a++
              continue
            }
            let w = 1
            while (a + w < nu && mask[b * nu + a + w] === key) w++
            let h = 1
            grow: while (b + h < nv) {
              for (let k = 0; k < w; k++) if (mask[(b + h) * nu + a + k] !== key) break grow
              h++
            }
            for (let r = 0; r < h; r++) mask.fill(-1, (b + r) * nu + a, (b + r) * nu + a + w)
            quad(out, d, u, v, side, base, size, i, a, b, w, h, key)
            a += w
          }
        }
      }
    }
  }
}

/** One merged face: `w × h` cells from `(a, b)` of slice `i` along axis `d`, facing `side`. */
function quad(out: Soup, d: number, u: number, v: number, side: number, base: number[], size: number, i: number, a: number, b: number, w: number, h: number, key: number) {
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

/**
 * Appends one cell on its own as a cube, all six faces, shrunk by `inset` metres on each side:
 * for a cell drawn apart from the rest (one that has been shot and is pushed in).
 */
export function voxelCube(shell: Shell, id: number, key: number, inset: number, out: Soup, offset: readonly [number, number, number] = [0, 0, 0]) {
  const x = id % shell.dims[0]
  const z = Math.floor(id / shell.dims[0]) % shell.dims[2]
  const y = Math.floor(id / (shell.dims[0] * shell.dims[2]))
  const size = CELL / 100
  const lo = [x, y, z].map((c, a) => shell.origin[a]! / 100 + c * size + inset - offset[a]!)
  const span = size - inset * 2
  const [r, g, b] = [((key >> 16) & 255) / 255, ((key >> 8) & 255) / 255, (key & 255) / 255]
  for (let d = 0; d < 3; d++) {
    const u = (d + 1) % 3
    const v = (d + 2) % 3
    for (const side of [1, -1]) {
      const corner = (du: number, dv: number) => {
        const p = [0, 0, 0]
        p[d] = lo[d]! + (side > 0 ? span : 0)
        p[u] = lo[u]! + du * span
        p[v] = lo[v]! + dv * span
        return p
      }
      const [p0, p1, p2, p3] = [corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1)]
      for (const p of side > 0 ? [p0, p1, p2, p0, p2, p3] : [p0, p2, p1, p0, p3, p2]) {
        out.positions.push(p[0]!, p[1]!, p[2]!)
        out.colors.push(r, g, b)
      }
    }
  }
}
