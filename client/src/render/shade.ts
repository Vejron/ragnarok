import { heightIn, type TileGround } from '@/sim/terrain'

/** Where to look for the ground beyond a tile's own edge, so shading carries across tile seams. */
export interface HeightSource {
  sample(x: number, z: number): number | null
}

/** Ground height under `(x, z)`, metres. */
export type Height = (x: number, z: number) => number

/**
 * The height of `ground`, continuing into the neighbouring tiles from `around` past its edge. Where
 * no neighbour is loaded it clamps to the edge, so the shading there leans one-sidedly.
 */
export function heightSampler(ground: TileGround, around?: HeightSource): Height {
  const x1 = ground.x0 + (ground.side - 1) * ground.cell
  const z1 = ground.z0 + (ground.side - 1) * ground.cell
  return (x, z) => {
    if (x >= ground.x0 && x <= x1 && z >= ground.z0 && z <= z1) return heightIn(ground, x, z)
    return around?.sample(x, z) ?? heightIn(ground, x, z)
  }
}

/** Unit surface normal at `(x, z)` from central differences `step` metres apart. */
export function normalAt(height: Height, x: number, z: number, step: number): [number, number, number] {
  return unitNormal(height(x + step, z), height(x - step, z), height(x, z + step), height(x, z - step), step)
}

/** `normalAt` from its four heights. */
function unitNormal(east: number, west: number, south: number, north: number, step: number): [number, number, number] {
  const dx = (east - west) / (2 * step)
  const dz = (south - north) / (2 * step)
  const inv = 1 / Math.sqrt(dx * dx + 1 + dz * dz)
  return [-dx * inv, inv, -dz * inv]
}

/** Direction the baked light comes from: the sun's azimuth, lower than the sun so slopes read. */
const LIGHT = (() => {
  const [x, , z] = [70, 80, 55]
  const along = Math.hypot(x, z)
  const lift = (25 * Math.PI) / 180
  return [(x / along) * Math.cos(lift), Math.sin(lift), (z / along) * Math.cos(lift)] as const
})()
const FLAT_LIGHT = LIGHT[1]
/** Distance to the ring of samples the cavity term compares against, metres. */
export const CAVITY_RADIUS = 6

const smooth = (low: number, high: number, v: number) => {
  const t = Math.min(Math.max((v - low) / (high - low), 0), 1)
  return t * t * (3 - 2 * t)
}

/**
 * A brightness multiplier baked into the terrain: hillshade from a low light, darker on steep
 * ground and in hollows, brighter on rises, and a faint low-frequency wobble so a big lawn is not
 * one flat tone. The live sun still lights it; this is the form the sun alone misses.
 */
export function toneAt(height: Height, x: number, z: number, normal: readonly [number, number, number]): number {
  return toneOf(normal, height(x, z), ringMean(height, x, z), x, z)
}

/** The mean height on a ring `CAVITY_RADIUS` round `(x, z)`, which `toneOf` compares the ground with. */
export function ringMean(height: Height, x: number, z: number): number {
  const r = CAVITY_RADIUS
  const d = r * Math.SQRT1_2
  return (height(x + r, z) + height(x - r, z) + height(x, z + r) + height(x, z - r) + height(x + d, z + d) + height(x - d, z + d) + height(x + d, z - d) + height(x - d, z - d)) / 8
}

/** How far past a tile's border `GroundShade` keeps its grids, metres: wider than any road and its round ends. */
const SHADE_MARGIN = 16
/** Spacing of `GroundShade`'s lattice of ring means, metres: they change over `CAVITY_RADIUS`, not over a cell. */
const MEAN_STEP = 3

/**
 * `normalAt` and `ringMean` for the many points of one tile that `Layer` shades (every vertex of
 * its roads and rails): worked out where first needed, the normal at the ground's nodes and the
 * ring's mean on a coarser lattice, and read bilinearly between them. A ribbon's casing and fill
 * lie on the same ground, and its vertices are a cell apart, so most are shared: about a dozen
 * height samples a vertex become one. Points beyond the margin are worked out directly.
 */
export class GroundShade {
  private readonly x0: number
  private readonly z0: number
  private readonly nodes: number
  private readonly lattice: number
  /** The ground's height at each node, NaN until worked out: the normals are differences of them. */
  private readonly heights: Float32Array
  /** `nx, ny, nz` a node, NaN until worked out. */
  private readonly normals: Float32Array
  /** A ring mean a lattice point, NaN until worked out. */
  private readonly means: Float32Array

  constructor(
    /** The ground's height anywhere, beyond the tile's edge too. */
    private readonly sample: Height,
    private readonly step: number,
    border: readonly [number, number, number, number],
  ) {
    // A whole number of steps, so the nodes are the ground's own and `normalAt`'s samples are nodes too.
    const margin = Math.ceil(SHADE_MARGIN / step) * step
    this.x0 = border[0] - margin
    this.z0 = border[1] - margin
    const span = Math.max(border[2] - border[0], border[3] - border[1]) + 2 * margin
    this.nodes = Math.ceil(span / step) + 1
    this.heights = new Float32Array(this.nodes * this.nodes).fill(NaN)
    this.lattice = Math.ceil(span / MEAN_STEP) + 1
    this.normals = new Float32Array(this.nodes * this.nodes * 3).fill(NaN)
    this.means = new Float32Array(this.lattice * this.lattice).fill(NaN)
  }

  /**
   * The ground's height at `(x, z)`, on the collider's triangles (`heightIn`: each cell split from its
   * `(x0, z1)` corner to its `(x1, z0)` one), from the nodes' heights. With `step` the ground's cell
   * the nodes are the ground's own, so this is `heightIn` but for rounding.
   */
  height(x: number, z: number): number {
    const gx = (x - this.x0) / this.step
    const gz = (z - this.z0) / this.step
    const i = Math.floor(gx)
    const j = Math.floor(gz)
    if (i < 0 || j < 0 || i >= this.nodes - 1 || j >= this.nodes - 1) return this.sample(x, z)
    const [fx, fz] = [gx - i, gz - j]
    if (fx + fz <= 1) {
      const h00 = this.heightAt(i, j)
      return h00 + fx * (this.heightAt(i + 1, j) - h00) + fz * (this.heightAt(i, j + 1) - h00)
    }
    const h11 = this.heightAt(i + 1, j + 1)
    return h11 + (1 - fx) * (this.heightAt(i, j + 1) - h11) + (1 - fz) * (this.heightAt(i + 1, j) - h11)
  }

  /** The unit surface normal at `(x, z)`, as `normalAt` with this shade's step. */
  normal(x: number, z: number): [number, number, number] {
    const gx = (x - this.x0) / this.step
    const gz = (z - this.z0) / this.step
    const i = Math.floor(gx)
    const j = Math.floor(gz)
    if (i < 0 || j < 0 || i >= this.nodes - 1 || j >= this.nodes - 1) return normalAt(this.sample, x, z, this.step)
    const [u, v] = [gx - i, gz - j]
    const [a, b, c, d] = [this.node(i, j), this.node(i + 1, j), this.node(i, j + 1), this.node(i + 1, j + 1)]
    const n = this.normals
    const mix = (k: number) => (n[a + k]! * (1 - u) + n[b + k]! * u) * (1 - v) + (n[c + k]! * (1 - u) + n[d + k]! * u) * v
    const [nx, ny, nz] = [mix(0), mix(1), mix(2)]
    const inv = 1 / Math.sqrt(nx * nx + ny * ny + nz * nz)
    return [nx * inv, ny * inv, nz * inv]
  }

  /** The mean height on the ring round `(x, z)`, as `ringMean`. */
  mean(x: number, z: number): number {
    const gx = (x - this.x0) / MEAN_STEP
    const gz = (z - this.z0) / MEAN_STEP
    const i = Math.floor(gx)
    const j = Math.floor(gz)
    if (i < 0 || j < 0 || i >= this.lattice - 1 || j >= this.lattice - 1) return ringMean(this.sample, x, z)
    const [u, v] = [gx - i, gz - j]
    return (this.meanAt(i, j) * (1 - u) + this.meanAt(i + 1, j) * u) * (1 - v) + (this.meanAt(i, j + 1) * (1 - u) + this.meanAt(i + 1, j + 1) * u) * v
  }

  /** Where node `(i, j)`'s normal is in `normals`, working it out first if need be. */
  private node(i: number, j: number): number {
    const at = (j * this.nodes + i) * 3
    if (Number.isNaN(this.normals[at]!)) {
      const last = this.nodes - 1
      const [nx, ny, nz] =
        i > 0 && j > 0 && i < last && j < last
          ? unitNormal(this.heightAt(i + 1, j), this.heightAt(i - 1, j), this.heightAt(i, j + 1), this.heightAt(i, j - 1), this.step)
          : normalAt(this.sample, this.x0 + i * this.step, this.z0 + j * this.step, this.step)
      this.normals[at] = nx
      this.normals[at + 1] = ny
      this.normals[at + 2] = nz
    }
    return at
  }

  private heightAt(i: number, j: number): number {
    const at = j * this.nodes + i
    let h = this.heights[at]!
    if (Number.isNaN(h)) this.heights[at] = h = this.sample(this.x0 + i * this.step, this.z0 + j * this.step)
    return h
  }

  private meanAt(i: number, j: number): number {
    const at = j * this.lattice + i
    let mean = this.means[at]!
    if (Number.isNaN(mean)) this.means[at] = mean = ringMean(this.sample, this.x0 + i * MEAN_STEP, this.z0 + j * MEAN_STEP)
    return mean
  }
}

/** `toneAt` from its parts: the normal and height at `(x, z)` and the ring's mean height there (`ringMean`). */
export function toneOf(normal: readonly [number, number, number], height: number, mean: number, x: number, z: number): number {
  const [nx, ny, nz] = normal
  const shade = 1 + 0.45 * (nx * LIGHT[0] + ny * LIGHT[1] + nz * LIGHT[2] - FLAT_LIGHT)
  const steep = 1 - 0.3 * smooth(0.02, 0.25, 1 - ny)
  const cavity = 1 + Math.min(Math.max((height - mean) * 0.05, -0.18), 0.08)
  const wobble = 1 + 0.03 * (Math.sin(x * 0.043 + 1.3) * Math.sin(z * 0.057) + 0.5 * Math.sin((x - z) * 0.09 + 0.7))
  return Math.min(Math.max(shade * steep * cavity * wobble, 0.55), 1.2)
}
