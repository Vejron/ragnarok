import type { Mesh, Ramp } from '@ragnarok/engine'
import { BALLAST_EXTRA, RAIL_TRAIN, type Point2, type Rail } from '@/world'
import { Layer, layerMaterial } from './roads'
import { GroundShade, type Height } from './shade'

/** Sleeper spacing along the track, its width along it, and how far it reaches past each rail, metres. */
const SLEEPER_PITCH = 0.65
const SLEEPER_WIDTH = 0.24
const SLEEPER_OVERHANG = 0.45
/** Along-track spacing of a rail's cross-sections, so it follows the ground's bumps, metres. */
const SPACING = 1

const BALLAST_COLOR = 0x8a8478
const SLEEPER_COLOR = 0x5a4632
const STEEL_COLOR = 0x9aa0a6

/**
 * A rail's cross-section as `[offset from its centre, lift]` pairs, left to right: a head on a
 * splayed foot. The flanks slope rather than stand, so every face has some area seen from above.
 */
const TRAIN_RAIL: readonly [number, number][] = [
  [-0.08, 0.12],
  [-0.035, 0.21],
  [0.035, 0.21],
  [0.08, 0.12],
]
/** A tram's rail is set into the street, its head flush with the road. */
const TRAM_RAIL: readonly [number, number][] = [
  [-0.035, 0.09],
  [0.035, 0.09],
]

/** Unit normal to the left of `a -> b`, or `null` for a zero-length step. */
function across(a: Point2, b: Point2): Point2 | null {
  const length = Math.hypot(b[0] - a[0], b[1] - a[1])
  return length < 1e-4 ? null : [-(b[1] - a[1]) / length, (b[0] - a[0]) / length]
}

/**
 * The direction to offset each vertex of `points` in, scaled so a line offset along it stays
 * parallel to both segments at a joint (a mitre, capped at sharp bends).
 */
function mitres(points: readonly Point2[]): Point2[] {
  const sides = points.slice(1).map((p, i) => across(points[i]!, p))
  return points.map((_, i) => {
    const before = sides[i - 1] ?? sides[i] ?? null
    const after = sides[i] ?? before
    if (!before || !after) return [0, 0]
    const sum: Point2 = [before[0] + after[0], before[1] + after[1]]
    const length = Math.hypot(sum[0], sum[1])
    if (length < 1e-4) return before
    const unit: Point2 = [sum[0] / length, sum[1] / length]
    const scale = 1 / Math.max(0.5, unit[0] * before[0] + unit[1] * before[1])
    return [unit[0] * scale, unit[1] * scale]
  })
}

/** Sweeps `profile` (centred `centre` metres to the left of the line) along `points` into `layer`. */
function sweep(layer: Layer, points: readonly Point2[], normals: readonly Point2[], centre: number, profile: readonly [number, number][]) {
  let previous: number[] | null = null
  const section = (x: number, z: number, nx: number, nz: number) => {
    const ids = profile.map(([offset, lift]) => layer.vertex(x + nx * (centre + offset), z + nz * (centre + offset), lift))
    if (previous) {
      for (let j = 0; j + 1 < ids.length; j++) {
        layer.triangle(previous[j]!, previous[j + 1]!, ids[j]!)
        layer.triangle(previous[j + 1]!, ids[j + 1]!, ids[j]!)
      }
    }
    previous = ids
  }
  for (let i = 0; i + 1 < points.length; i++) {
    const [ax, az] = points[i]!
    const [bx, bz] = points[i + 1]!
    const [anx, anz] = normals[i]!
    const [bnx, bnz] = normals[i + 1]!
    const parts = Math.max(1, Math.ceil(Math.hypot(bx - ax, bz - az) / SPACING))
    // A joint's section is shared by the segments either side of it.
    for (let k = i === 0 ? 0 : 1; k <= parts; k++) {
      const t = k / parts
      section(ax + (bx - ax) * t, az + (bz - az) * t, anx + (bnx - anx) * t, anz + (bnz - anz) * t)
    }
  }
}

/** Sleepers every `SLEEPER_PITCH` along `points`, each square across the track. */
function sleepers(layer: Layer, points: readonly Point2[], length: number) {
  let ahead = SLEEPER_PITCH / 2
  for (let i = 0; i + 1 < points.length; i++) {
    const [ax, az] = points[i]!
    const [bx, bz] = points[i + 1]!
    const side = across(points[i]!, points[i + 1]!)
    if (!side) continue
    const span = Math.hypot(bx - ax, bz - az)
    const [dx, dz] = [(bx - ax) / span, (bz - az) / span]
    for (; ahead < span; ahead += SLEEPER_PITCH) {
      const [x, z] = [ax + dx * ahead, az + dz * ahead]
      const corner = (a: number, b: number) => layer.vertex(x + side[0] * a + dx * b, z + side[1] * a + dz * b)
      const w = SLEEPER_WIDTH / 2
      const [p, q, r, s] = [corner(-length, -w), corner(length, -w), corner(length, w), corner(-length, w)]
      layer.triangle(p, q, r)
      layer.triangle(p, r, s)
    }
    ahead -= span
  }
}

/**
 * Railway tracks laid on the terrain like roads: a ballast bed under the roads (so a level
 * crossing is paved), and sleepers and rails over them. Trams are rails alone, flush with the
 * street. `border` is the tile's `[x0, z0, x1, z1]`; a track ending on it gets no round cap, since
 * its neighbour tile carries on from there.
 */
export function buildRails(rails: readonly Rail[], height: Height, step: number, border: readonly [number, number, number, number], ramp: Ramp): Mesh[] {
  if (rails.length === 0) return []
  const shade = new GroundShade(height, step, border)
  const ballast = new Layer(0.04, shade)
  const ties = new Layer(0.14, shade)
  const steel = new Layer(0, shade)
  for (const { gauge, kind, points } of rails) {
    const train = kind === RAIL_TRAIN
    if (train) {
      const half = (gauge + BALLAST_EXTRA) / 2
      for (let i = 0; i + 1 < points.length; i++) ballast.segment(points[i]![0], points[i]![1], points[i + 1]![0], points[i + 1]![1], half)
      for (const [x, z] of points) {
        const onBorder = Math.abs(x - border[0]) < 0.01 || Math.abs(x - border[2]) < 0.01 || Math.abs(z - border[1]) < 0.01 || Math.abs(z - border[3]) < 0.01
        if (!onBorder) ballast.disc(x, z, half)
      }
      sleepers(ties, points, gauge / 2 + SLEEPER_OVERHANG)
    }
    const normals = mitres(points)
    for (const centre of [-gauge / 2, gauge / 2]) sweep(steel, points, normals, centre, train ? TRAIN_RAIL : TRAM_RAIL)
  }
  // The ballast sits under the roads' casings; sleepers and rails pull in front of their fills.
  return [ballast.mesh(layerMaterial(BALLAST_COLOR, 6, ramp)), ties.mesh(layerMaterial(SLEEPER_COLOR, 26, ramp)), steel.mesh(layerMaterial(STEEL_COLOR, 28, ramp))].filter((mesh): mesh is Mesh => mesh !== null)
}
