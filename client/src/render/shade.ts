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
  const dx = (height(x + step, z) - height(x - step, z)) / (2 * step)
  const dz = (height(x, z + step) - height(x, z - step)) / (2 * step)
  const inv = 1 / Math.hypot(dx, 1, dz)
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

/** `toneAt` from its parts: the normal and height at `(x, z)` and the ring's mean height there (`ringMean`). */
export function toneOf(normal: readonly [number, number, number], height: number, mean: number, x: number, z: number): number {
  const [nx, ny, nz] = normal
  const shade = 1 + 0.45 * (nx * LIGHT[0] + ny * LIGHT[1] + nz * LIGHT[2] - FLAT_LIGHT)
  const steep = 1 - 0.3 * smooth(0.02, 0.25, 1 - ny)
  const cavity = 1 + Math.min(Math.max((height - mean) * 0.05, -0.18), 0.08)
  const wobble = 1 + 0.03 * (Math.sin(x * 0.043 + 1.3) * Math.sin(z * 0.057) + 0.5 * Math.sin((x - z) * 0.09 + 0.7))
  return Math.min(Math.max(shade * steep * cavity * wobble, 0.55), 1.2)
}
