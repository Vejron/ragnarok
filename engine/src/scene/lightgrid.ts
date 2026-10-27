import { MAX_LIGHTS, PointLights } from './lights'

/** Side of a cell of the light grid, metres. */
export const LIGHT_CELL = 8
/** Cells along each side of the grid, which is centred on the camera: 512 m across, about as far as the fog. */
export const LIGHT_GRID = 64
/** The most lights a cell lists; past it the weakest are left out. */
export const LIGHT_CELL_MAX = 32
/** Floats per light record: `x, y, z, radius`, `r, g, b` (linear, times strength), `cosOuter`, `dx, dy, dz`, `cosInner`. */
export const LIGHT_FLOATS = 12

const CELLS = LIGHT_GRID * LIGHT_GRID

type Live = PointLights['live'][number]

/**
 * This frame's lights binned into a grid over the ground around the camera, for the shaders'
 * `pointLight` (lights.wgsl.ts): a surface looks up its cell by its world x and z and walks only the
 * lights listed there. Height is left to the lights' own radius test, so the grid serves every view
 * of the frame (the main one and the reflection) and every shader that knows its world position.
 *
 * `records` holds the lights, strongest first. `grid` holds per cell `offset << 8 | count`, then, from
 * index `LIGHT_GRID²`, every cell's light indices one after the other.
 */
export class LightGrid {
  readonly records = new Float32Array(MAX_LIGHTS * LIGHT_FLOATS)
  readonly grid = new Uint32Array(CELLS + CELLS * LIGHT_CELL_MAX)
  /** Lights listed this frame. */
  count = 0
  /** Light indices written after the cells (`grid` is used up to `LIGHT_GRID² + indexCount`). */
  indexCount = 0
  /** World x and z of the grid's corner: a whole number of cells, so the cells do not move under the lights. */
  originX = 0
  originZ = 0

  private readonly picked: Live[] = []
  private readonly strength = new Map<Live, number>()
  private readonly counts = new Uint32Array(CELLS)
  private readonly filled = new Uint32Array(CELLS)
  private readonly byStrength = (a: Live, b: Live) => this.strength.get(b)! - this.strength.get(a)!

  /**
   * Lists `lights` around the camera at `(camX, camY, camZ)`. Lights `visible` rejects (a sphere outside
   * every view drawn this frame) and lights wholly off the grid are left out, and past `MAX_LIGHTS` the
   * weakest as seen from the camera.
   */
  build(lights: readonly Live[], camX: number, camY: number, camZ: number, visible?: (x: number, y: number, z: number, radius: number) => boolean) {
    const half = (LIGHT_GRID / 2) * LIGHT_CELL
    this.originX = Math.floor(camX / LIGHT_CELL) * LIGHT_CELL - half
    this.originZ = Math.floor(camZ / LIGHT_CELL) * LIGHT_CELL - half
    const picked = this.picked
    picked.length = 0
    this.strength.clear()
    const size = LIGHT_GRID * LIGHT_CELL
    for (const l of lights) {
      const k = l.intensity * PointLights.decay(l)
      if (k <= 0 || l.radius <= 0) continue
      if (l.x + l.radius < this.originX || l.x - l.radius > this.originX + size) continue
      if (l.z + l.radius < this.originZ || l.z - l.radius > this.originZ + size) continue
      if (visible && !visible(l.x, l.y, l.z, l.radius)) continue
      const d2 = (l.x - camX) ** 2 + (l.y - camY) ** 2 + (l.z - camZ) ** 2
      this.strength.set(l, k / (1 + d2 * 0.002))
      picked.push(l)
    }
    picked.sort(this.byStrength)
    const n = Math.min(picked.length, MAX_LIGHTS)
    this.count = n

    const r = this.records
    for (let i = 0; i < n; i++) {
      const l = picked[i]!
      const k = l.intensity * PointLights.decay(l)
      const o = i * LIGHT_FLOATS
      r[o] = l.x; r[o + 1] = l.y; r[o + 2] = l.z; r[o + 3] = l.radius
      r[o + 4] = l.r * k; r[o + 5] = l.g * k; r[o + 6] = l.b * k; r[o + 7] = l.cosOuter
      r[o + 8] = l.dx; r[o + 9] = l.dy; r[o + 10] = l.dz; r[o + 11] = l.cosInner
    }

    // Count each cell's lights (strongest first, so a full cell keeps the strongest), lay the cells'
    // lists out one after the other, then fill them in the same order.
    this.counts.fill(0)
    for (let i = 0; i < n; i++) this.bin(i, false)
    const grid = this.grid
    let offset = 0
    for (let c = 0; c < CELLS; c++) {
      grid[c] = (offset << 8) | this.counts[c]!
      this.filled[c] = 0
      offset += this.counts[c]!
    }
    this.indexCount = offset
    for (let i = 0; i < n; i++) this.bin(i, true)
  }

  /** The light indices cell `(cx, cz)` lists, strongest first. */
  listed(cx: number, cz: number): number[] {
    const cell = this.grid[cz * LIGHT_GRID + cx]!
    const start = CELLS + (cell >>> 8)
    return Array.from(this.grid.subarray(start, start + (cell & 0xff)))
  }

  /** Counts light `i` into the cells its sphere's footprint touches, or (`fill`) writes it into their lists. */
  private bin(i: number, fill: boolean) {
    const r = this.records
    const o = i * LIGHT_FLOATS
    const x = r[o]! - this.originX
    const z = r[o + 2]! - this.originZ
    const radius = r[o + 3]!
    const x0 = Math.max(0, Math.floor((x - radius) / LIGHT_CELL))
    const x1 = Math.min(LIGHT_GRID - 1, Math.floor((x + radius) / LIGHT_CELL))
    const z0 = Math.max(0, Math.floor((z - radius) / LIGHT_CELL))
    const z1 = Math.min(LIGHT_GRID - 1, Math.floor((z + radius) / LIGHT_CELL))
    const r2 = radius * radius
    for (let cz = z0; cz <= z1; cz++) {
      // The nearest point of the cell's row to the light, along z.
      const nz = Math.min(Math.max(z, cz * LIGHT_CELL), (cz + 1) * LIGHT_CELL) - z
      for (let cx = x0; cx <= x1; cx++) {
        const nx = Math.min(Math.max(x, cx * LIGHT_CELL), (cx + 1) * LIGHT_CELL) - x
        if (nx * nx + nz * nz > r2) continue
        const c = cz * LIGHT_GRID + cx
        if (!fill) {
          if (this.counts[c]! < LIGHT_CELL_MAX) this.counts[c] = this.counts[c]! + 1
        } else if (this.filled[c]! < this.counts[c]!) {
          this.grid[CELLS + (this.grid[c]! >>> 8) + this.filled[c]!] = i
          this.filled[c] = this.filled[c]! + 1
        }
      }
    }
  }
}
