import { SILENT_GRIT, type Grit } from '@/sound/gritsynth'
import { SURFACE_ASPHALT, SURFACE_DIRT, SURFACE_GRASS, SURFACE_SAND, SURFACE_WATER, surfaceWetness } from './surface'

/**
 * What the ground sounds like: the class of what is underfoot or under the wheels, and the rolling
 * roar, the crunch and the squeal each class gives at a speed. Pure numbers: `vehicleaudio.ts` turns
 * them into nodes.
 */

/** What is underfoot, as it is heard. */
export type Ground = 'tarmac' | 'gravel' | 'grass' | 'sand' | 'mud' | 'snow' | 'ice' | 'water'
/** What a foot comes down on: the ground, water over the ankles, or a floor above the terrain. */
export type Footing = Ground | 'wade' | 'floor'

/**
 * The ground a `SURFACE_*` is heard as. Dirt is gravel: the tracks and farm roads are gravel. In a
 * winter world everything off the ploughed roads is under snow, the marsh is frozen, and so is the water
 * when the lakes are (`iced`).
 */
export function groundOf(surface: number, winter: boolean, iced: boolean): Ground {
  if (surface === SURFACE_WATER) return iced ? 'ice' : 'water'
  if (surface === SURFACE_ASPHALT) return 'tarmac'
  if (winter) return surfaceWetness(surface) > 0 ? 'ice' : 'snow'
  if (surfaceWetness(surface) > 0) return 'mud'
  if (surface === SURFACE_DIRT) return 'gravel'
  if (surface === SURFACE_SAND) return 'sand'
  if (surface === SURFACE_GRASS) return 'grass'
  return 'grass'
}

/** How a vehicle's wheels or tracks are heard rolling over the ground. */
export interface Rolling {
  /** The low roar of rolling, its level and the cutoff of its low-pass, Hz. */
  roll: number
  rollHz: number
  /** The crunch: stones, snow, stems, slaps of water. */
  grit: Grit
  /** How much of a skid is heard as a squeal: all of it on tarmac, a scrub off-road, nothing on ice or in water. */
  squeal: number
  /** How a track's clatter carries: ringing on hard ground, dull on soft, nothing afloat. Its level, and its tone as a share of the kind's. */
  clank: number
  clankHz: number
}

const clamp = (value: number, low = 0, high = 1) => Math.min(high, Math.max(low, value))

/**
 * How `ground` is heard under a vehicle going `speed` m/s with its worst tyre at `slip` (0..1). A
 * `tracked` one is heavier: lower, louder crunch.
 */
export function rolling(ground: Ground, speed: number, slip: number, tracked: boolean): Rolling {
  const s = Math.abs(speed)
  const pace = clamp(s / 20)
  // Spinning wheels churn loose ground: more grains.
  const churn = 1 + 2 * clamp(slip)
  let out: Rolling
  switch (ground) {
    case 'tarmac':
      out = { roll: 0.14 * clamp(s / 30), rollHz: 320 + 8 * s, grit: SILENT_GRIT, squeal: 1, clank: 1, clankHz: 1.25 }
      break
    case 'gravel':
      out = { roll: 0.35 * pace, rollHz: 650 + 25 * s, grit: { rate: Math.min(900, (40 + 28 * s) * churn), level: 0.45 * clamp(s / 6), hz: 2600, ring: 0.6, length: 3 }, squeal: 0.3, clank: 0.85, clankHz: 1 }
      break
    case 'grass':
      out = { roll: 0.22 * pace, rollHz: 480 + 18 * s, grit: { rate: (15 + 8 * s) * churn, level: 0.16 * clamp(s / 6), hz: 1700, ring: 0.3, length: 6 }, squeal: 0.12, clank: 0.6, clankHz: 0.7 }
      break
    case 'sand':
      out = { roll: 0.28 * pace, rollHz: 900 + 20 * s, grit: { rate: Math.min(1200, (120 + 30 * s) * churn), level: 0.22 * clamp(s / 6), hz: 4200, ring: 0.4, length: 2.5 }, squeal: 0.15, clank: 0.55, clankHz: 0.75 }
      break
    case 'mud':
      out = { roll: 0.2 * pace, rollHz: 300 + 10 * s, grit: { rate: (5 + 4 * s) * churn, level: 0.55 * clamp(s / 4 + slip), hz: 420, ring: 1.2, length: 26 }, squeal: 0, clank: 0.45, clankHz: 0.55 }
      break
    case 'snow':
      out = { roll: 0.2 * pace, rollHz: 600 + 15 * s, grit: { rate: (25 + 12 * s) * churn, level: 0.38 * clamp(s / 5), hz: 1300, ring: 1.4, length: 9 }, squeal: 0.05, clank: 0.5, clankHz: 0.65 }
      break
    case 'ice':
      out = { roll: 0.09 * clamp(s / 20), rollHz: 1800, grit: SILENT_GRIT, squeal: 0, clank: 0.9, clankHz: 1.15 }
      break
    case 'water':
      // A wash along the hull, and now and then a wave slapping it: a few a second, more as it goes faster.
      out = { roll: 0.3 * clamp(s / 12), rollHz: 450 + 25 * s, grit: { rate: 1.5 + 0.25 * s, level: 0.3 * clamp(s / 6), hz: 320, ring: 0.3, length: 60 }, squeal: 0, clank: 0, clankHz: 1 }
      break
  }
  if (tracked && out.grit.level > 0) out = { ...out, grit: { ...out.grit, level: Math.min(1, out.grit.level * 1.3), hz: out.grit.hz * 0.8 } }
  return out
}
