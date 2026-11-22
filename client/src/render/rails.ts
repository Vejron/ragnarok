import { BALLAST_EXTRA, RAIL_TRAIN, type Point2, type Rail, type RailDeck } from '@/world'
import { Layer, type Ribbon } from './roads'
import { winter } from './season'
import type { GroundShade } from './shade'

/** Sleeper spacing along the track, its width along it, and how far it reaches past each rail, metres. */
const SLEEPER_PITCH = 0.65
const SLEEPER_WIDTH = 0.24
const SLEEPER_OVERHANG = 0.45
/** Along-track spacing of a rail's cross-sections, so it follows the ground's bumps, metres. */
const SPACING = 1

const BALLAST_COLOR = 0x8a8478
const WINTER_BALLAST_COLOR = 0xdfe3e8
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
export function mitres(points: readonly Point2[]): Point2[] {
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
export function sweep(layer: Layer, points: readonly Point2[], normals: readonly Point2[], centre: number, profile: readonly [number, number][]) {
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

/** Sleepers every `SLEEPER_PITCH` along `points`, each square across the track, where the track at that distance along it `stands`. */
function sleepers(layer: Layer, points: readonly Point2[], length: number, stands?: (along: number) => boolean) {
  let ahead = SLEEPER_PITCH / 2
  let travelled = 0
  for (let i = 0; i + 1 < points.length; i++) {
    const [ax, az] = points[i]!
    const [bx, bz] = points[i + 1]!
    const side = across(points[i]!, points[i + 1]!)
    if (!side) continue
    const span = Math.hypot(bx - ax, bz - az)
    const [dx, dz] = [(bx - ax) / span, (bz - az) / span]
    for (; ahead < span; ahead += SLEEPER_PITCH) {
      if (stands && !stands(travelled + ahead)) continue
      const [x, z] = [ax + dx * ahead, az + dz * ahead]
      const corner = (a: number, b: number) => layer.vertex(x + side[0] * a + dx * b, z + side[1] * a + dz * b)
      const w = SLEEPER_WIDTH / 2
      const [p, q, r, s] = [corner(-length, -w), corner(length, -w), corner(length, w), corner(-length, w)]
      layer.triangle(p, q, r)
      layer.triangle(p, r, s)
    }
    ahead -= span
    travelled += span
  }
}

/** A track on a deck as the ground a `Layer` lies on: the deck's height under the nearest of the track, and level across. */
class DeckShade {
  constructor(
    private readonly points: readonly Point2[],
    private readonly tops: readonly number[],
  ) {}

  height(x: number, z: number): number {
    let [best, nearest] = [this.tops[0]!, Infinity]
    for (let i = 0; i + 1 < this.points.length; i++) {
      const [ax, az] = this.points[i]!
      const [bx, bz] = this.points[i + 1]!
      const [dx, dz] = [bx - ax, bz - az]
      const t = Math.max(0, Math.min(1, ((x - ax) * dx + (z - az) * dz) / (dx * dx + dz * dz || 1)))
      const off = (x - ax - t * dx) ** 2 + (z - az - t * dz) ** 2
      if (off >= nearest) continue
      nearest = off
      best = this.tops[i]! + t * (this.tops[i + 1]! - this.tops[i]!)
    }
    return best
  }

  normal(): [number, number, number] {
    return [0, 1, 0]
  }

  /** Its own height, so the deck's track is toned as flat ground is. */
  mean(x: number, z: number): number {
    return this.height(x, z)
  }
}

/** The stretches of a track that stand, as lines, and whether it stands a distance along it. */
function standing(points: readonly Point2[], tops: readonly number[], gauge: number, stands: (x: number, y: number, z: number) => boolean): { lines: Point2[][]; at: (along: number) => boolean } {
  const along = [0]
  for (let i = 1; i < points.length; i++) along.push(along[i - 1]! + Math.hypot(points[i]![0] - points[i - 1]![0], points[i]![1] - points[i - 1]![1]))
  const length = along[along.length - 1]!
  const held: boolean[] = []
  let i = 0
  /** The point `s` metres along, and the segment it is on. */
  const pointAt = (s: number) => {
    while (i + 2 < points.length && along[i + 1]! < s) i++
    const t = Math.max(0, Math.min(1, (s - along[i]!) / (along[i + 1]! - along[i]! || 1)))
    const [a, b] = [points[i]!, points[i + 1]!]
    return { x: a[0] + (b[0] - a[0]) * t, z: a[1] + (b[1] - a[1]) * t, y: tops[i]! + (tops[i + 1]! - tops[i]!) * t, side: across(a, b) ?? [0, 0] }
  }
  // Each sleeper's length, by the deck under its middle and under both rails there.
  for (let k = 0; k * SLEEPER_PITCH < length; k++) {
    const { x, y, z, side } = pointAt(Math.min(length, (k + 0.5) * SLEEPER_PITCH))
    held.push([0, -gauge / 2, gauge / 2].every((off) => stands(x + side[0] * off, y, z + side[1] * off)))
  }
  const at = (s: number) => held[Math.min(held.length - 1, Math.floor(s / SLEEPER_PITCH))] ?? true
  if (held.every((ok) => ok)) return { lines: [[...points]], at }
  const lines: Point2[][] = []
  for (let from = 0; from < held.length; from++) {
    if (!held[from]) continue
    let to = from
    while (to < held.length && held[to]) to++
    const [start, end] = [from * SLEEPER_PITCH, Math.min(length, to * SLEEPER_PITCH)]
    i = 0
    const first = pointAt(start)
    const line: Point2[] = [[first.x, first.z], ...points.filter((_, k) => along[k]! > start && along[k]! < end)]
    const last = pointAt(end)
    line.push([last.x, last.z])
    lines.push(line)
    from = to
  }
  return { lines, at }
}

/**
 * Railway tracks laid on the terrain like roads: a ballast bed under the roads (so a level
 * crossing is paved), and sleepers and rails over them. Trams are rails alone, flush with the
 * street. `border` is the tile's `[x0, z0, x1, z1]`; a track ending on it gets no round cap, since
 * its neighbour tile carries on from there.
 */
export function railRibbons(rails: readonly Rail[], shade: GroundShade, border: readonly [number, number, number, number]): Ribbon[] {
  const ballast = new Layer(0.04, shade)
  const ties = new Layer(0.14, shade)
  const steel = new Layer(0, shade)
  for (const { gauge, kind, points, deck } of rails) {
    // A bridge's track is drawn on its deck (`deckRibbons`), not on what is under it.
    if (deck) continue
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
  return [ballast.ribbon(winter ? WINTER_BALLAST_COLOR : BALLAST_COLOR, 6), ties.ribbon(SLEEPER_COLOR, 26), steel.ribbon(STEEL_COLOR, 28)].filter((ribbon): ribbon is Ribbon => ribbon !== null)
}

/**
 * The sleepers and rails of the tracks on bridges, at the decks' heights: no ballast, which is the
 * deck's own top. Where the deck under it is gone (`stands`: of the section, at a point of the
 * track) so is the track, a sleeper's length at a time.
 */
export function deckRibbons(rails: readonly (Rail & { deck: RailDeck })[], stands: (section: number, x: number, y: number, z: number) => boolean): Ribbon[] {
  const ribbons: Ribbon[] = []
  for (const { gauge, kind, points, deck } of rails) {
    if (points.length < 2) continue
    const shade = new DeckShade(points, deck.tops)
    const ties = new Layer(0.14, shade)
    const steel = new Layer(0, shade)
    const { lines, at } = standing(points, deck.tops, gauge, (x, y, z) => stands(deck.building, x, y, z))
    const train = kind === RAIL_TRAIN
    if (train) sleepers(ties, points, gauge / 2 + SLEEPER_OVERHANG, at)
    for (const line of lines) {
      const normals = mitres(line)
      for (const centre of [-gauge / 2, gauge / 2]) sweep(steel, line, normals, centre, train ? TRAIN_RAIL : TRAM_RAIL)
    }
    for (const ribbon of [ties.ribbon(SLEEPER_COLOR, 26), steel.ribbon(STEEL_COLOR, 28)]) if (ribbon) ribbons.push(ribbon)
  }
  return joined(ribbons)
}

/** The ribbons of a colour as one: a draw a layer, however many runs of track a tile's decks have. */
function joined(ribbons: readonly Ribbon[]): Ribbon[] {
  const out: Ribbon[] = []
  for (const color of new Set(ribbons.map((ribbon) => ribbon.color))) {
    const same = ribbons.filter((ribbon) => ribbon.color === color)
    const total = (key: 'positions' | 'indices') => same.reduce((sum, { buffers }) => sum + buffers[key].length, 0)
    const [positions, normals, colors] = [new Float32Array(total('positions')), new Float32Array(total('positions')), new Float32Array(total('positions'))]
    const indices = new Uint32Array(total('indices'))
    let [vertex, index] = [0, 0]
    for (const { buffers } of same) {
      positions.set(buffers.positions, vertex)
      normals.set(buffers.normals, vertex)
      colors.set(buffers.colors, vertex)
      for (const at of buffers.indices) indices[index++] = at + vertex / 3
      vertex += buffers.positions.length
    }
    out.push({ ...same[0]!, buffers: { positions, normals, colors, indices } })
  }
  return out
}
