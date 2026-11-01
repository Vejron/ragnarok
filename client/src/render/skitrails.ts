import { SKI_GROOVE_OFFSET } from '@/sim/consts'
import type { SkiTrail } from '@/world'
import { Layer, type Ribbon } from './roads'
import { mitres, sweep } from './rails'
import type { GroundShade } from './shade'

/** The packed lane and the shadowed grooves of the two classic tracks. */
const LANE_COLOR = 0xfcfdff
const GROOVE_COLOR = 0x8ea3bd
/** Each ski's groove is one of a pair this far apart, metres. */
const GROOVE_GAP = 0.22

/**
 * Lines on the snow, not ridges: skis run over them, so they keep under a ski's height, yet over the
 * nearest terrain mesh's 4 cm of simplification, or the ground would hide them.
 */
const GROOVE: readonly [number, number][] = [
  [-0.06, 0.045],
  [0.06, 0.045],
]

/** Groomed ski tracks on the snow: a smooth lane with two classic tracks cut into it. */
export function skiRibbons(trails: readonly SkiTrail[], shade: GroundShade): Ribbon[] {
  const lane = new Layer(0.04, shade)
  const grooves = new Layer(0, shade)
  for (const { width, points } of trails) {
    for (let i = 0; i + 1 < points.length; i++) lane.segment(points[i]![0], points[i]![1], points[i + 1]![0], points[i + 1]![1], width / 2)
    for (const [x, z] of points) lane.disc(x, z, width / 2)
    const normals = mitres(points)
    for (const track of [-SKI_GROOVE_OFFSET, SKI_GROOVE_OFFSET]) {
      for (const side of [-GROOVE_GAP / 2, GROOVE_GAP / 2]) sweep(grooves, points, normals, track + side, GROOVE)
    }
  }
  return [lane.ribbon(LANE_COLOR, 8), grooves.ribbon(GROOVE_COLOR, 14)].filter((ribbon): ribbon is Ribbon => ribbon !== null)
}
